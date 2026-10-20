//! Reading the binary form: as a [`Value`], borrowed as a [`ValueRef`], or
//! step by step by a schema reading itself ([`crate::Schema::decode`]).

use std::sync::Arc;

use bumpalo::Bump;

use super::{DecodeError, MAGIC, MAX_DEPTH, NEW_STRING, STREAM_LEN_BYTES, VERSION, read_varint, tag, unzigzag};
use crate::borrowed::{MapRef, ValueRef};
use crate::value::{Map, Ref, Res, Value};

/// Strings a decoder makes room for at once: a message's keys and values.
const STRINGS_START: usize = 16;

/// Bytes of a float, and of a 32-bit one.
const F64_BYTES: usize = 8;
const F32_BYTES: usize = 4;

/// Bytes a map entry takes at least: its key and its value's tag.
const MAP_ENTRY_MIN: usize = 2;

/// Reads the binary form. The step-by-step reads ([`Decoder::int`],
/// [`Decoder::map`] …) answer `None` for anything but what
/// [`crate::Schema::encode`] writes there — the schema then reads the
/// slow way, which says what's wrong.
pub struct Decoder<'b> {
    b: &'b [u8],
    i: usize,
    strings: Vec<&'b str>,
    /// Map keys made so far, by table index: each key is allocated once.
    keys: Vec<Option<Arc<str>>>,
    depth: usize,
}

impl<'b> Decoder<'b> {
    /// After the header: the root value next.
    pub fn open(bytes: &'b [u8]) -> Result<Self, DecodeError> {
        let mut d = Decoder { b: bytes, i: 0, strings: Vec::with_capacity(STRINGS_START), keys: Vec::new(), depth: 0 };
        if !bytes.starts_with(MAGIC) {
            return Err(d.error("not Yog Data (no `YOGD` header)"));
        }
        d.i = MAGIC.len();
        let version = d.byte()?;
        if version != VERSION {
            return Err(d.error(format!("Yog Data version {version}, this reads version {VERSION}")));
        }
        Ok(d)
    }

    /// A stream's body ([`crate::Stream`]): its count and values, its strings its own.
    pub(crate) fn body(bytes: &'b [u8]) -> Self {
        Decoder { b: bytes, i: 0, strings: Vec::with_capacity(STRINGS_START), keys: Vec::new(), depth: 0 }
    }

    /// A stream's body: its bytes, read past.
    fn stream_body(&mut self) -> Result<&'b [u8], DecodeError> {
        let len = u32::from_le_bytes(self.fixed::<STREAM_LEN_BYTES>()?) as usize;
        self.take(len)
    }

    /// A stream's values, decoded as a list.
    fn stream_values(&mut self) -> Result<Vec<Value>, DecodeError> {
        let mut d = Decoder::body(self.stream_body()?);
        d.depth = self.depth;
        let n = d.count(1)?;
        let mut items = Vec::with_capacity(n);
        for _ in 0..n {
            items.push(d.value()?);
        }
        d.end()?;
        Ok(items)
    }

    /// Whether every byte was read.
    pub fn at_end(&self) -> bool {
        self.i == self.b.len()
    }

    pub(super) fn end(&self) -> Result<(), DecodeError> {
        if self.at_end() { Ok(()) } else { Err(self.error("bytes after the value")) }
    }

    fn error(&self, message: impl Into<String>) -> DecodeError {
        DecodeError { message: message.into(), offset: self.i }
    }

    fn byte(&mut self) -> Result<u8, DecodeError> {
        let b = *self.b.get(self.i).ok_or_else(|| self.error("unexpected end"))?;
        self.i += 1;
        Ok(b)
    }

    fn varint(&mut self) -> Result<u64, DecodeError> {
        read_varint(self.b, &mut self.i).ok_or_else(|| self.error("a varint cut short or longer than 64 bits"))
    }

    /// A length, checked against what's left: each of the `count` things takes at least `min` bytes.
    fn count(&mut self, min: usize) -> Result<usize, DecodeError> {
        let n = self.varint()?;
        let left = self.b.len() - self.i;
        if n > (left / min.max(1)) as u64 {
            return Err(self.error(format!("length {n} past the end ({left} bytes left)")));
        }
        Ok(n as usize)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let bytes = self.b.get(self.i..self.i + N).ok_or_else(|| self.error("unexpected end"))?;
        self.i += N;
        bytes.try_into().map_err(|_| self.error("unexpected end"))
    }

    /// `len` bytes.
    fn take(&mut self, len: usize) -> Result<&'b [u8], DecodeError> {
        let end = self.i.checked_add(len).filter(|&e| e <= self.b.len()).ok_or_else(|| self.error("bytes past the end"))?;
        let out = &self.b[self.i..end];
        self.i = end;
        Ok(out)
    }

    /// A packed list of floats `N` bytes wide: each one's bytes, read in place (no alignment asked).
    fn packed_floats<const N: usize>(&mut self) -> Result<std::slice::Iter<'b, [u8; N]>, DecodeError> {
        let n = self.count(N)?;
        let bytes = self.take(n * N)?;
        Ok(bytes.as_chunks::<N>().0.iter())
    }

    /// A length and that many bytes.
    fn length_bytes(&mut self) -> Result<&'b [u8], DecodeError> {
        let len = usize::try_from(self.varint()?).map_err(|_| self.error("a length beyond memory"))?;
        self.take(len)
    }

    /// A string: new (read, and added to the table) or the table's.
    fn string(&mut self) -> Result<&'b str, DecodeError> {
        let i = self.string_index()?;
        Ok(self.strings[i])
    }

    fn string_index(&mut self) -> Result<usize, DecodeError> {
        let v = self.varint()?;
        if v & NEW_STRING == 0 {
            let i = (v >> 1) as usize;
            return if i < self.strings.len() { Ok(i) } else { Err(self.error(format!("string {i} isn't in the table"))) };
        }
        let len = usize::try_from(v >> 1).map_err(|_| self.error("a length beyond memory"))?;
        let at = self.i;
        let s = std::str::from_utf8(self.take(len)?).map_err(|_| DecodeError { message: "a string isn't UTF-8".into(), offset: at })?;
        self.strings.push(s);
        Ok(self.strings.len() - 1)
    }

    /// A map key: shared with every other map that has it.
    fn shared_key(&mut self) -> Result<Arc<str>, DecodeError> {
        let i = self.string_index()?;
        if self.keys.len() <= i {
            self.keys.resize(i + 1, None);
        }
        Ok(self.keys[i].get_or_insert_with(|| self.strings[i].into()).clone())
    }

    fn deeper(&mut self) -> Result<(), DecodeError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error(format!("nested deeper than {MAX_DEPTH} levels")));
        }
        Ok(())
    }

    /// The next value, owned.
    pub fn value(&mut self) -> Result<Value, DecodeError> {
        Ok(match self.byte()? {
            tag::NULL => Value::Null,
            tag::FALSE => Value::Bool(false),
            tag::TRUE => Value::Bool(true),
            tag::INT => Value::Int(unzigzag(self.varint()?)),
            tag::FLOAT => Value::Float(f64::from_le_bytes(self.fixed()?)),
            tag::FLOAT32 => Value::Float(f64::from(f32::from_le_bytes(self.fixed()?))),
            tag::FLOATS => Value::List(self.packed_floats::<F64_BYTES>()?.map(|f| Value::Float(f64::from_le_bytes(*f))).collect()),
            tag::FLOATS32 => Value::List(self.packed_floats::<F32_BYTES>()?.map(|f| Value::Float(f64::from(f32::from_le_bytes(*f)))).collect()),
            tag::STR => Value::Str(self.string()?.to_owned()),
            tag::LIST => {
                let n = self.count(1)?;
                self.deeper()?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.value()?);
                }
                self.depth -= 1;
                Value::List(items)
            }
            tag::MAP => {
                let n = self.count(MAP_ENTRY_MIN)?;
                self.deeper()?;
                let mut map = Map::with_capacity(n);
                for _ in 0..n {
                    let k = self.shared_key()?;
                    map.push_unchecked(k, self.value()?);
                }
                self.depth -= 1;
                Value::Map(map)
            }
            t @ (tag::REF | tag::TAG) => {
                let r = Ref { kind: self.string()?.to_owned(), id: self.string()?.to_owned() };
                if t == tag::REF { Value::Ref(r) } else { Value::Tag(r) }
            }
            tag::RES => Value::Res(Res { set: self.string()?.to_owned(), key: self.string()?.to_owned() }),
            tag::TEXT => Value::Text(self.string()?.to_owned()),
            tag::IMAGE => Value::Image(self.string()?.to_owned()),
            tag::COLOR => Value::Color(u32::from_le_bytes(self.fixed()?)),
            tag::LANG => Value::Lang(self.string()?.to_owned()),
            tag::BYTES => Value::Bytes(self.length_bytes()?.to_vec()),
            tag::INTS => {
                let n = self.count(1)?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(Value::Int(unzigzag(self.varint()?)));
                }
                Value::List(items)
            }
            tag::STREAM => Value::List(self.stream_values()?),
            t => {
                self.i -= 1;
                return Err(self.error(format!("unknown tag {t}")));
            }
        })
    }

    /// The next value, borrowed: strings point into the bytes, lists and maps go in `arena`.
    pub fn value_ref<'a>(&mut self, arena: &'a Bump) -> Result<ValueRef<'a>, DecodeError>
    where
        'b: 'a,
    {
        let mut stack = Vec::new();
        self.value_ref_in(arena, &mut stack)
    }

    /// [`Decoder::value_ref`]: children gather on `stack`, then move into `arena` as one slice.
    fn value_ref_in<'a>(&mut self, arena: &'a Bump, stack: &mut Vec<ValueRef<'a>>) -> Result<ValueRef<'a>, DecodeError>
    where
        'b: 'a,
    {
        Ok(match self.byte()? {
            tag::NULL => ValueRef::Null,
            tag::FALSE => ValueRef::Bool(false),
            tag::TRUE => ValueRef::Bool(true),
            tag::INT => ValueRef::Int(unzigzag(self.varint()?)),
            tag::FLOAT => ValueRef::Float(f64::from_le_bytes(self.fixed()?)),
            tag::FLOAT32 => ValueRef::Float(f64::from(f32::from_le_bytes(self.fixed()?))),
            t @ (tag::FLOATS | tag::FLOATS32) => {
                let items: Vec<ValueRef<'a>> = if t == tag::FLOATS {
                    self.packed_floats::<F64_BYTES>()?.map(|f| ValueRef::Float(f64::from_le_bytes(*f))).collect()
                } else {
                    self.packed_floats::<F32_BYTES>()?.map(|f| ValueRef::Float(f64::from(f32::from_le_bytes(*f)))).collect()
                };
                ValueRef::List(arena.alloc_slice_copy(&items))
            }
            tag::STR => ValueRef::Str(self.string()?),
            tag::LIST => {
                let n = self.count(1)?;
                self.deeper()?;
                let start = stack.len();
                for _ in 0..n {
                    let v = self.value_ref_in(arena, stack)?;
                    stack.push(v);
                }
                let items = arena.alloc_slice_copy(&stack[start..]);
                stack.truncate(start);
                self.depth -= 1;
                ValueRef::List(items)
            }
            tag::MAP => {
                let n = self.count(MAP_ENTRY_MIN)?;
                self.deeper()?;
                let mut entries = bumpalo::collections::Vec::with_capacity_in(n, arena);
                for _ in 0..n {
                    let k = self.string()?;
                    entries.push((k, self.value_ref_in(arena, stack)?));
                }
                self.depth -= 1;
                ValueRef::Map(MapRef(entries.into_bump_slice()))
            }
            t @ (tag::REF | tag::TAG) => {
                let (kind, id) = (self.string()?, self.string()?);
                if t == tag::REF { ValueRef::Ref { kind, id } } else { ValueRef::Tag { kind, id } }
            }
            tag::RES => ValueRef::Res { set: self.string()?, key: self.string()? },
            tag::TEXT => ValueRef::Text(self.string()?),
            tag::IMAGE => ValueRef::Image(self.string()?),
            tag::COLOR => ValueRef::Color(u32::from_le_bytes(self.fixed()?)),
            tag::LANG => ValueRef::Lang(self.string()?),
            tag::BYTES => ValueRef::Bytes(self.length_bytes()?),
            tag::INTS => {
                let n = self.count(1)?;
                let mut items = bumpalo::collections::Vec::with_capacity_in(n, arena);
                for _ in 0..n {
                    items.push(ValueRef::Int(unzigzag(self.varint()?)));
                }
                ValueRef::List(items.into_bump_slice())
            }
            tag::STREAM => {
                let mut d = Decoder::body(self.stream_body()?);
                d.depth = self.depth;
                let n = d.count(1)?;
                let start = stack.len();
                for _ in 0..n {
                    let v = d.value_ref_in(arena, stack)?;
                    stack.push(v);
                }
                d.end()?;
                let items = arena.alloc_slice_copy(&stack[start..]);
                stack.truncate(start);
                ValueRef::List(items)
            }
            t => {
                self.i -= 1;
                return Err(self.error(format!("unknown tag {t}")));
            }
        })
    }

    // ── Step by step, for a schema reading itself ───────────────────────────

    /// The next tag, if it's `t`: read.
    fn next_is(&mut self, t: u8) -> Option<()> {
        (self.b.get(self.i) == Some(&t)).then(|| self.i += 1)
    }

    /// Whether a null is next: read if it is.
    pub fn null(&mut self) -> bool {
        self.next_is(tag::NULL).is_some()
    }

    pub fn bool(&mut self) -> Option<bool> {
        match *self.b.get(self.i)? {
            tag::FALSE | tag::TRUE => {
                self.i += 1;
                Some(self.b[self.i - 1] == tag::TRUE)
            }
            _ => None,
        }
    }

    pub fn int(&mut self) -> Option<i64> {
        self.next_is(tag::INT)?;
        self.varint().ok().map(unzigzag)
    }

    /// A float — or an int, as a number.
    pub fn float(&mut self) -> Option<f64> {
        match *self.b.get(self.i)? {
            tag::FLOAT => {
                self.i += 1;
                self.fixed().ok().map(f64::from_le_bytes)
            }
            tag::FLOAT32 => {
                self.i += 1;
                self.fixed().ok().map(|b| f64::from(f32::from_le_bytes(b)))
            }
            tag::INT => self.int().map(|n| n as f64),
            _ => None,
        }
    }

    pub fn str(&mut self) -> Option<&'b str> {
        self.next_is(tag::STR)?;
        self.string().ok()
    }

    pub fn bytes(&mut self) -> Option<&'b [u8]> {
        self.next_is(tag::BYTES)?;
        self.length_bytes().ok()
    }

    /// A list next: how many values; [`Decoder::leave`] after them.
    pub fn list(&mut self) -> Option<usize> {
        self.next_is(tag::LIST)?;
        let n = self.count(1).ok()?;
        self.deeper().ok()?;
        Some(n)
    }

    /// A packed list of ints next ([`Decoder::packed_int`] each): how many.
    pub fn ints(&mut self) -> Option<usize> {
        self.next_is(tag::INTS)?;
        self.count(1).ok()
    }

    /// A stream next: its body (count and values, strings its own).
    pub fn stream(&mut self) -> Option<&'b [u8]> {
        self.next_is(tag::STREAM)?;
        self.stream_body().ok()
    }

    /// What's left to read.
    pub(crate) fn rest(&self) -> &'b [u8] {
        &self.b[self.i..]
    }

    /// A stream body's count, first thing in it.
    pub(crate) fn stream_count(&mut self) -> Option<usize> {
        self.count(1).ok()
    }

    /// A packed list of floats next: each one.
    pub fn floats(&mut self) -> Option<Vec<f64>> {
        self.next_is(tag::FLOATS)?;
        Some(self.packed_floats::<F64_BYTES>().ok()?.map(|f| f64::from_le_bytes(*f)).collect())
    }

    /// A packed list of 32-bit floats next: each one.
    pub fn floats32(&mut self) -> Option<Vec<f32>> {
        self.next_is(tag::FLOATS32)?;
        Some(self.packed_floats::<F32_BYTES>().ok()?.map(|f| f32::from_le_bytes(*f)).collect())
    }

    /// One int of a packed list.
    pub fn packed_int(&mut self) -> Option<i64> {
        self.varint().ok().map(unzigzag)
    }

    /// A map next: how many entries, each a [`Decoder::key`] and a value;
    /// [`Decoder::leave`] after them.
    pub fn map(&mut self) -> Option<usize> {
        self.next_is(tag::MAP)?;
        let n = self.count(MAP_ENTRY_MIN).ok()?;
        self.deeper().ok()?;
        Some(n)
    }

    /// The end of a list or map.
    pub fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub fn key(&mut self) -> Option<&'b str> {
        self.string().ok()
    }

    /// The next value, read and left (its strings still join the table).
    pub fn skip(&mut self) -> Option<()> {
        let arena = Bump::new();
        self.value_ref(&arena).ok().map(drop)
    }
}
