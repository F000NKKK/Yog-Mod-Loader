//! The binary form — what the build compiles content to and the runtime and
//! the Java hosts read, like NBT:
//!
//! ```text
//! "YOGD" version:u8
//! strings: varint count, then each as varint length + UTF-8
//! root value
//! ```
//!
//! A value is a tag byte and its payload; every string (map keys too) is an
//! index into the table, so repeated keys and ids cost one varint each.
//!
//! | Tag | Value | Payload |
//! |---:|---|---|
//! | 0 | null | — |
//! | 1, 2 | false, true | — |
//! | 3 | int | zigzag varint |
//! | 4 | float | f64, little-endian |
//! | 5 | string | string |
//! | 6 | list | varint count, values |
//! | 7 | map | varint count, (key string, value) pairs |
//! | 8 | ref | kind string, id string |
//! | 9 | tag | kind string, id string |
//! | 10 | resource | set string, key string |
//! | 11 | text | string |
//! | 12 | image | string |
//! | 13 | color | u32, little-endian (`0xAARRGGBB`) |
//! | 14 | translation key | string |
//!
//! Varints are LEB128. Decoding checks every length against the input, so
//! malformed bytes are an error, never a panic or a huge allocation.

use std::collections::HashMap;
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};

use bumpalo::Bump;

use crate::borrowed::{MapRef, ValueRef};
use crate::value::{Map, Ref, Res, Value};

pub const MAGIC: &[u8; 4] = b"YOGD";
pub const VERSION: u8 = 1;

/// Nested deeper than this is malformed.
pub const MAX_DEPTH: usize = 256;

mod tag {
    pub const NULL: u8 = 0;
    pub const FALSE: u8 = 1;
    pub const TRUE: u8 = 2;
    pub const INT: u8 = 3;
    pub const FLOAT: u8 = 4;
    pub const STR: u8 = 5;
    pub const LIST: u8 = 6;
    pub const MAP: u8 = 7;
    pub const REF: u8 = 8;
    pub const TAG: u8 = 9;
    pub const RES: u8 = 10;
    pub const TEXT: u8 = 11;
    pub const IMAGE: u8 = 12;
    pub const COLOR: u8 = 13;
    pub const LANG: u8 = 14;
}

/// `value` in the binary form.
pub fn encode(value: &Value) -> Vec<u8> {
    let mut w = Writer::new();
    w.value(value);
    w.finish()
}

/// Writes the binary form piece by piece, from data that isn't a [`Value`]
/// — nothing is built or copied on the way; strings are borrowed until
/// [`Writer::finish`]. A map or list is given its length, then that many
/// entries (`key` then a value) or values.
///
/// ```
/// let mut w = yog_data::binary::Writer::new();
/// w.map(2).key("id").str("mymod:ruby").key("count").int(3);
/// let v = yog_data::binary::decode(&w.finish()).unwrap();
/// assert_eq!(v.get("count").and_then(|c| c.as_i64()), Some(3));
/// ```
pub struct Writer<'v>(Encoder<'v>);

impl Default for Writer<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'v> Writer<'v> {
    pub fn new() -> Self {
        Writer(Encoder { body: Vec::new(), strings: HashMap::default(), table: Vec::new() })
    }

    pub fn null(&mut self) -> &mut Self {
        self.0.body.push(tag::NULL);
        self
    }

    pub fn bool(&mut self, b: bool) -> &mut Self {
        self.0.body.push(if b { tag::TRUE } else { tag::FALSE });
        self
    }

    pub fn int(&mut self, n: i64) -> &mut Self {
        self.0.body.push(tag::INT);
        varint(&mut self.0.body, ((n << 1) ^ (n >> 63)) as u64);
        self
    }

    pub fn float(&mut self, f: f64) -> &mut Self {
        self.0.body.push(tag::FLOAT);
        self.0.body.extend_from_slice(&f.to_le_bytes());
        self
    }

    pub fn str(&mut self, s: &'v str) -> &mut Self {
        self.0.body.push(tag::STR);
        self.0.string(s);
        self
    }

    /// A list of `len` values.
    pub fn list(&mut self, len: usize) -> &mut Self {
        self.0.body.push(tag::LIST);
        varint(&mut self.0.body, len as u64);
        self
    }

    /// A map of `len` entries, each a [`Writer::key`] and a value.
    pub fn map(&mut self, len: usize) -> &mut Self {
        self.0.body.push(tag::MAP);
        varint(&mut self.0.body, len as u64);
        self
    }

    pub fn key(&mut self, k: &'v str) -> &mut Self {
        self.0.string(k);
        self
    }

    pub fn value(&mut self, v: &'v Value) -> &mut Self {
        self.0.value(v);
        self
    }

    /// The bytes.
    pub fn finish(self) -> Vec<u8> {
        let enc = self.0;
        let table_len: usize = enc.table.iter().map(|s| s.len() + 2).sum();
        let mut out = Vec::with_capacity(5 + table_len + enc.body.len() + 4);
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        varint(&mut out, enc.table.len() as u64);
        for s in &enc.table {
            varint(&mut out, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        out.extend_from_slice(&enc.body);
        out
    }
}

struct Encoder<'v> {
    body: Vec<u8>,
    strings: HashMap<&'v str, u32, BuildHasherDefault<Fx>>,
    table: Vec<&'v str>,
}

impl<'v> Encoder<'v> {
    fn string(&mut self, s: &'v str) {
        let next = self.table.len() as u32;
        let i = *self.strings.entry(s).or_insert_with(|| {
            self.table.push(s);
            next
        });
        varint(&mut self.body, i as u64);
    }

    fn value(&mut self, v: &'v Value) {
        match v {
            Value::Null => self.body.push(tag::NULL),
            Value::Bool(false) => self.body.push(tag::FALSE),
            Value::Bool(true) => self.body.push(tag::TRUE),
            Value::Int(n) => {
                self.body.push(tag::INT);
                varint(&mut self.body, ((n << 1) ^ (n >> 63)) as u64);
            }
            Value::Float(f) => {
                self.body.push(tag::FLOAT);
                self.body.extend_from_slice(&f.to_le_bytes());
            }
            Value::Str(s) => {
                self.body.push(tag::STR);
                self.string(s);
            }
            Value::List(items) => {
                self.body.push(tag::LIST);
                varint(&mut self.body, items.len() as u64);
                for item in items {
                    self.value(item);
                }
            }
            Value::Map(map) => {
                self.body.push(tag::MAP);
                varint(&mut self.body, map.len() as u64);
                for (k, v) in map.iter() {
                    self.string(k);
                    self.value(v);
                }
            }
            Value::Ref(r) | Value::Tag(r) => {
                self.body.push(if matches!(v, Value::Ref(_)) {
                    tag::REF
                } else {
                    tag::TAG
                });
                self.string(&r.kind);
                self.string(&r.id);
            }
            Value::Res(r) => {
                self.body.push(tag::RES);
                self.string(&r.set);
                self.string(&r.key);
            }
            Value::Text(s) => {
                self.body.push(tag::TEXT);
                self.string(s);
            }
            Value::Image(s) => {
                self.body.push(tag::IMAGE);
                self.string(s);
            }
            Value::Lang(s) => {
                self.body.push(tag::LANG);
                self.string(s);
            }
            Value::Color(c) => {
                self.body.push(tag::COLOR);
                self.body.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
}

/// rustc's Fx hash — the string table's keys are short and not adversarial.
#[derive(Default)]
struct Fx(u64);

impl Fx {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl Hasher for Fx {
    fn write(&mut self, bytes: &[u8]) {
        let (words, rest) = bytes.as_chunks::<8>();
        for w in words {
            self.add(u64::from_le_bytes(*w));
        }
        if !rest.is_empty() {
            let mut b = [0u8; 8];
            b[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(b));
        }
    }

    fn write_u8(&mut self, b: u8) {
        self.add(b as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

fn varint(out: &mut Vec<u8>, mut n: u64) {
    while n >= 0x80 {
        out.push((n as u8) | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
}

/// Malformed binary data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    pub message: String,
    pub offset: usize,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for DecodeError {}

/// Reads the binary form.
pub fn decode(bytes: &[u8]) -> Result<Value, DecodeError> {
    let mut d = Decoder::open(bytes)?;
    let v = d.value(0)?;
    d.end()?;
    Ok(v)
}

/// Reads the binary form borrowed: every string points into `bytes`,
/// lists and maps go in `arena` — nothing is copied. What a schema reads
/// fastest ([`crate::read_bytes`]).
pub fn decode_ref<'a>(bytes: &'a [u8], arena: &'a Bump) -> Result<ValueRef<'a>, DecodeError> {
    let mut d = Decoder::open(bytes)?;
    let mut stack = Vec::new();
    let v = d.value_ref(0, arena, &mut stack)?;
    d.end()?;
    Ok(v)
}

impl<'b> Decoder<'b> {
    /// The header and the string table read; the root value next.
    fn open(bytes: &'b [u8]) -> Result<Self, DecodeError> {
        let mut d = Decoder {
            b: bytes,
            i: 0,
            strings: Vec::new(),
            keys: Vec::new(),
        };
        if !bytes.starts_with(MAGIC) {
            return Err(d.error("not Yog Data (no `YOGD` header)"));
        }
        d.i = 4;
        let version = d.byte()?;
        if version != VERSION {
            return Err(d.error(format!(
                "Yog Data version {version}, this reads version {VERSION}"
            )));
        }
        let count = d.count(1)?;
        d.strings.reserve(count);
        for _ in 0..count {
            let len = d.count(1)?;
            let start = d.i;
            let s = std::str::from_utf8(&bytes[start..start + len]).map_err(|_| DecodeError {
                message: "a string isn't UTF-8".into(),
                offset: start,
            })?;
            d.strings.push(s);
            d.i += len;
        }
        Ok(d)
    }

    fn end(&self) -> Result<(), DecodeError> {
        if self.i != self.b.len() {
            return Err(self.error("bytes after the value"));
        }
        Ok(())
    }
}

struct Decoder<'b> {
    b: &'b [u8],
    i: usize,
    strings: Vec<&'b str>,
    /// Map keys made so far, by table index: each key is allocated once.
    keys: Vec<Option<std::sync::Arc<str>>>,
}

impl<'b> Decoder<'b> {
    fn error(&self, message: impl Into<String>) -> DecodeError {
        DecodeError {
            message: message.into(),
            offset: self.i,
        }
    }

    fn byte(&mut self) -> Result<u8, DecodeError> {
        let b = *self
            .b
            .get(self.i)
            .ok_or_else(|| self.error("unexpected end"))?;
        self.i += 1;
        Ok(b)
    }

    fn varint(&mut self) -> Result<u64, DecodeError> {
        let mut n = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            n |= ((b & 0x7F) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(n);
            }
        }
        Err(self.error("varint longer than 64 bits"))
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
        let bytes = self
            .b
            .get(self.i..self.i + N)
            .ok_or_else(|| self.error("unexpected end"))?;
        self.i += N;
        Ok(bytes.try_into().unwrap())
    }

    fn string(&mut self) -> Result<&'b str, DecodeError> {
        let i = self.varint()?;
        self.strings
            .get(i as usize)
            .copied()
            .ok_or_else(|| self.error(format!("string {i} isn't in the table")))
    }

    /// A map key: shared with every other map that has it.
    fn key(&mut self) -> Result<std::sync::Arc<str>, DecodeError> {
        let i = self.varint()? as usize;
        if let Some(Some(k)) = self.keys.get(i) {
            return Ok(k.clone());
        }
        let s = *self.strings.get(i).ok_or_else(|| self.error(format!("string {i} isn't in the table")))?;
        let k: std::sync::Arc<str> = s.into();
        if self.keys.len() <= i {
            self.keys.resize(self.strings.len(), None);
        }
        self.keys[i] = Some(k.clone());
        Ok(k)
    }

    /// [`Decoder::value`], borrowed: children gather on `stack`, then move
    /// into `arena` as one slice.
    fn value_ref<'a>(&mut self, depth: usize, arena: &'a Bump, stack: &mut Vec<ValueRef<'a>>) -> Result<ValueRef<'a>, DecodeError>
    where
        'b: 'a,
    {
        if depth > MAX_DEPTH {
            return Err(self.error(format!("nested deeper than {MAX_DEPTH} levels")));
        }
        Ok(match self.byte()? {
            tag::NULL => ValueRef::Null,
            tag::FALSE => ValueRef::Bool(false),
            tag::TRUE => ValueRef::Bool(true),
            tag::INT => {
                let z = self.varint()?;
                ValueRef::Int(((z >> 1) as i64) ^ -((z & 1) as i64))
            }
            tag::FLOAT => ValueRef::Float(f64::from_le_bytes(self.fixed()?)),
            tag::STR => ValueRef::Str(self.string()?),
            tag::LIST => {
                let n = self.count(1)?;
                let start = stack.len();
                for _ in 0..n {
                    let v = self.value_ref(depth + 1, arena, stack)?;
                    stack.push(v);
                }
                let items = arena.alloc_slice_copy(&stack[start..]);
                stack.truncate(start);
                ValueRef::List(items)
            }
            tag::MAP => {
                let n = self.count(2)?;
                let mut entries = bumpalo::collections::Vec::with_capacity_in(n, arena);
                for _ in 0..n {
                    let k = self.string()?;
                    entries.push((k, self.value_ref(depth + 1, arena, stack)?));
                }
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
            t => {
                self.i -= 1;
                return Err(self.error(format!("unknown tag {t}")));
            }
        })
    }

    fn value(&mut self, depth: usize) -> Result<Value, DecodeError> {
        if depth > MAX_DEPTH {
            return Err(self.error(format!("nested deeper than {MAX_DEPTH} levels")));
        }
        Ok(match self.byte()? {
            tag::NULL => Value::Null,
            tag::FALSE => Value::Bool(false),
            tag::TRUE => Value::Bool(true),
            tag::INT => {
                let z = self.varint()?;
                Value::Int(((z >> 1) as i64) ^ -((z & 1) as i64))
            }
            tag::FLOAT => Value::Float(f64::from_le_bytes(self.fixed()?)),
            tag::STR => Value::Str(self.string()?.to_string()),
            tag::LIST => {
                let n = self.count(1)?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.value(depth + 1)?);
                }
                Value::List(items)
            }
            tag::MAP => {
                let n = self.count(2)?;
                let mut map = Map::with_capacity(n);
                for _ in 0..n {
                    let k = self.key()?;
                    map.push_unchecked(k, self.value(depth + 1)?);
                }
                Value::Map(map)
            }
            t @ (tag::REF | tag::TAG) => {
                let r = Ref {
                    kind: self.string()?.to_string(),
                    id: self.string()?.to_string(),
                };
                if t == tag::REF {
                    Value::Ref(r)
                } else {
                    Value::Tag(r)
                }
            }
            tag::RES => Value::Res(Res {
                set: self.string()?.to_string(),
                key: self.string()?.to_string(),
            }),
            tag::TEXT => Value::Text(self.string()?.to_string()),
            tag::IMAGE => Value::Image(self.string()?.to_string()),
            tag::COLOR => Value::Color(u32::from_le_bytes(self.fixed()?)),
            tag::LANG => Value::Lang(self.string()?.to_string()),
            t => {
                self.i -= 1;
                return Err(self.error(format!("unknown tag {t}")));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        let mut m = Map::new();
        m.insert("name", Value::Str("Ruby".into()));
        m.insert(
            "n",
            Value::List(vec![
                Value::Int(0),
                Value::Int(-1),
                Value::Int(i64::MAX),
                Value::Int(i64::MIN),
                Value::Float(0.5),
                Value::Null,
                Value::Bool(true),
                Value::Bool(false),
            ]),
        );
        m.insert("item", Value::Ref(Ref::new("item", "minecraft:stick")));
        m.insert("tag", Value::Tag(Ref::new("item", "minecraft:planks")));
        m.insert(
            "title",
            Value::Res(Res {
                set: "Strings".into(),
                key: "title".into(),
            }),
        );
        m.insert("text", Value::Text("{b|hi}".into()));
        m.insert("icon", Value::Image("sprite:x".into()));
        m.insert("color", Value::Color(0x80FF0000));
        m.insert("lang", Value::Lang("gui.done".into()));
        m.insert("again", Value::Str("Ruby".into()));
        Value::Map(m)
    }

    #[test]
    fn round_trips_every_type() {
        let v = sample();
        let bytes = encode(&v);
        assert_eq!(decode(&bytes).unwrap(), v);
        // "Ruby" is stored once.
        assert_eq!(bytes.windows(4).filter(|w| w == b"Ruby").count(), 1);
    }

    #[test]
    fn the_borrowed_decode_reads_the_same_without_copying() {
        let v = sample();
        let bytes = encode(&v);
        let arena = Bump::new();
        let r = decode_ref(&bytes, &arena).unwrap();
        assert_eq!(r.to_value(), v);
        assert_eq!(r, v.borrow(&arena), "borrowing a Value gives the same");
        // Its strings point into the bytes.
        let span = bytes.as_ptr_range();
        let mut strings = Vec::new();
        fn walk<'a>(v: &ValueRef<'a>, out: &mut Vec<&'a str>) {
            match v {
                ValueRef::Str(s) | ValueRef::Text(s) | ValueRef::Image(s) | ValueRef::Lang(s) => out.push(s),
                ValueRef::List(l) => l.iter().for_each(|x| walk(x, out)),
                ValueRef::Map(m) => m.iter().for_each(|(k, x)| {
                    out.push(k);
                    walk(x, out)
                }),
                _ => {}
            }
        }
        walk(&r, &mut strings);
        assert!(!strings.is_empty() && strings.iter().all(|s| span.contains(&s.as_ptr())));
        // Malformed bytes are errors here too.
        for len in 0..bytes.len() {
            assert!(decode_ref(&bytes[..len], &arena).is_err(), "truncated at {len}");
        }
    }

    #[test]
    fn malformed_bytes_are_errors() {
        let bytes = encode(&sample());
        // Every truncation fails cleanly.
        for len in 0..bytes.len() {
            assert!(decode(&bytes[..len]).is_err(), "truncated at {len}");
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(
            decode(&extra)
                .unwrap_err()
                .message
                .contains("after the value")
        );
        assert!(
            decode(b"YOGD\x02\x00\x00")
                .unwrap_err()
                .message
                .contains("version 2")
        );
        assert!(
            decode(b"YOGD\x01\x00\x63")
                .unwrap_err()
                .message
                .contains("unknown tag")
        );
        assert!(
            decode(b"YOGD\x01\x00\x05\x00")
                .unwrap_err()
                .message
                .contains("isn't in the table")
        );
        // A huge count with nothing behind it doesn't allocate.
        assert!(
            decode(b"YOGD\x01\x00\x06\xff\xff\xff\xff\x0f")
                .unwrap_err()
                .message
                .contains("past the end")
        );
        let deep: Vec<u8> = b"YOGD\x01\x00"
            .iter()
            .copied()
            .chain(std::iter::repeat_n([6u8, 1], 300).flatten())
            .chain([0])
            .collect();
        assert!(decode(&deep).unwrap_err().message.contains("nested deeper"));
    }
}
