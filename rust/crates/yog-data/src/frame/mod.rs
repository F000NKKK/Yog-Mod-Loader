//! Frames: Yog Data packed for the bus — both sides know the schema, so
//! nothing says what a value is or what it's called. A record's fields go
//! in the order they're declared, after the record's length (so a reader
//! that knows fewer fields skips the rest; one that knows more takes their
//! defaults: fields are only ever appended). Strings go in a table at the
//! end, each once; the body refers to them by index.
//!
//! ```text
//! frame  = body, table, table length (a varint written backwards)
//! table  = (varint length, UTF-8) per string, in the order first used
//! record = varint length, its fields
//! ```
//!
//! | Value | Packed |
//! |---|---|
//! | bool | a byte, 0 or 1 |
//! | int | zigzag varint |
//! | `f64`, `f32` | 8, 4 bytes, little-endian |
//! | string | varint index into the table |
//! | bytes | varint length, the bytes |
//! | `Option` | a byte, 0 (none) or 1, then the value |
//! | list | varint count, the values |
//! | list of ints | varint count, bits each (a byte), the values bit-packed (signed ones zigzagged) |
//! | list of floats | varint count, `f32`-exact (a byte), the values raw — 4 bytes each if they all are `f32`s |
//! | unit enum | varint variant index (variants only appended) |
//! | anything else | its binary form ([`crate::binary`]), as bytes |
//!
//! Nothing is aligned, every value takes at least a byte (a count is
//! checked against the bytes left), and a malformed frame reads as `None`,
//! never a panic.
//!
//! ```
//! #[derive(yog_data::Schema, Debug, PartialEq)]
//! struct Hit { id: String, count: u32, ratio: f32, next: Option<Box<Hit>> }
//!
//! let hit = Hit { id: "minecraft:stone".into(), count: 3, ratio: 0.5, next: None };
//! let frame = yog_data::frame::to_frame(&hit);
//! assert_eq!(yog_data::frame::read_frame::<Hit>(&frame), Some(hit));
//! ```

mod shaped;
mod view;

pub use shaped::{read_shaped, unpack_shaped};
pub use view::{Floats, FromF64, Ints, View, read_view};

use std::sync::Arc;

use bumpalo::Bump;

use crate::binary::interner::Interner;
use crate::binary::writer::START_BYTES;
use crate::binary::{Decoder, MAX_DEPTH, VARINT_BITS, VARINT_MAX_SHIFT, VARINT_MORE, VARINT_SHIFT, Writer, read_varint, unzigzag, varint, zigzag};
use crate::schema::{Schema, read_ref_value};

/// The bytes of a `f64` and of a `f32`.
const F64_BYTES: usize = 8;
const F32_BYTES: usize = 4;
/// A record's length, reserved before its fields are known: one byte, the
/// common case; a longer record moves its fields to make room.
const LENGTH_RESERVED: usize = 1;
/// The biggest length one varint byte holds.
const ONE_BYTE_MAX: usize = 0x7F;
/// Strings a reader keeps in itself before it needs the heap.
const INLINE_STRINGS: usize = 16;

/// `v` as a frame.
pub fn to_frame<T: Schema>(v: &T) -> Vec<u8> {
    let mut p = Packer::new();
    v.pack(&mut p);
    p.finish()
}

thread_local! {
    /// This thread's packer, reused: a frame built without allocating once
    /// its buffers have grown to the thread's messages.
    static PACKER: std::cell::RefCell<Packer> = std::cell::RefCell::new(Packer::new());
}

/// `f` on `v` as a frame, packed in this thread's reused buffers (a fresh
/// packer when they're busy — `f` packing another).
pub fn with_frame<T: Schema, R>(v: &T, f: impl FnOnce(&[u8]) -> R) -> R {
    PACKER.with(|cell| match cell.try_borrow_mut() {
        Ok(mut p) => {
            p.clear();
            v.pack(&mut p);
            let p = &mut *p;
            p.seal();
            let out = f(&p.out);
            p.out.clear();
            out
        }
        Err(_) => f(&to_frame(v)),
    })
}

/// A `T` out of a frame; `None` if the frame isn't one.
pub fn read_frame<T: Schema>(bytes: &[u8]) -> Option<T> {
    let mut u = Unpacker::open(bytes)?;
    let v = T::unpack(&mut u)?;
    u.at_end().then_some(v)
}

/// Writes a frame.
pub struct Packer {
    out: Vec<u8>,
    table: Vec<u8>,
    strings: Interner,
}

impl Default for Packer {
    fn default() -> Self {
        Self::new()
    }
}

impl Packer {
    pub fn new() -> Self {
        Packer { out: Vec::with_capacity(START_BYTES), table: Vec::new(), strings: Interner::new() }
    }

    /// An unsigned number (a count, an index).
    pub fn put_u(&mut self, n: u64) {
        varint(&mut self.out, n);
    }

    pub fn put_i(&mut self, n: i64) {
        varint(&mut self.out, zigzag(n));
    }

    pub fn put_bool(&mut self, b: bool) {
        self.out.push(u8::from(b));
    }

    pub fn put_f64(&mut self, f: f64) {
        self.out.extend_from_slice(&f.to_le_bytes());
    }

    pub fn put_f32(&mut self, f: f32) {
        self.out.extend_from_slice(&f.to_le_bytes());
    }

    /// Floats, raw: a count, whether they're kept as `f32` (when every one
    /// is one exactly — half the bytes), then each one's bytes.
    pub fn put_f64s(&mut self, items: &[f64]) {
        self.put_u(items.len() as u64);
        let narrow = items.iter().all(|f| f64::from(*f as f32).to_bits() == f.to_bits());
        self.put_bool(narrow);
        for f in items {
            if narrow {
                self.put_f32(*f as f32);
            } else {
                self.put_f64(*f);
            }
        }
    }

    /// Unsigned numbers bit-packed: a count, how many bits each takes (the
    /// biggest one's, one at least), then the bits, little-endian.
    pub fn put_uints(&mut self, len: usize, items: impl Iterator<Item = u64> + Clone) {
        self.put_u(len as u64);
        let max = items.clone().fold(0, u64::max);
        let bits = (u64::BITS - max.leading_zeros()).max(1);
        self.out.push(bits as u8);
        let mut acc: u128 = 0;
        let mut have = 0u32;
        for v in items {
            acc |= u128::from(v) << have;
            have += bits;
            while have >= u8::BITS {
                self.out.push(acc as u8);
                acc >>= u8::BITS;
                have -= u8::BITS;
            }
        }
        if have > 0 {
            self.out.push(acc as u8);
        }
    }

    pub fn put_f32s(&mut self, items: &[f32]) {
        self.put_u(items.len() as u64);
        for f in items {
            self.put_f32(*f);
        }
    }

    /// A string: its index in the table, which gets it the first time.
    pub fn put_str(&mut self, s: &str) {
        let i = match self.strings.find(&self.table, s) {
            Some(i) => i,
            None => {
                varint(&mut self.table, s.len() as u64);
                let at = self.table.len();
                self.table.extend_from_slice(s.as_bytes());
                self.strings.add(&self.table, s, at)
            }
        };
        self.put_u(i as u64);
    }

    pub fn put_bytes(&mut self, b: &[u8]) {
        self.put_u(b.len() as u64);
        self.out.extend_from_slice(b);
    }

    /// `v` in its binary form, as bytes: what a frame holds of a type that
    /// doesn't pack itself.
    pub fn put_tagged<T: Schema>(&mut self, v: &T) {
        let mut w = Writer::body();
        v.encode(&mut w);
        self.put_bytes(&w.finish());
    }

    /// A record starts: where its length goes ([`Packer::end`] fills it in).
    pub fn begin(&mut self) -> usize {
        let at = self.out.len();
        self.out.extend_from_slice(&[0; LENGTH_RESERVED]);
        at
    }

    /// The record begun at `at` ends: its length written.
    pub fn end(&mut self, at: usize) {
        let len = self.out.len() - at - LENGTH_RESERVED;
        if len <= ONE_BYTE_MAX {
            self.out[at] = len as u8;
            return;
        }
        let mut length = Vec::new();
        varint(&mut length, len as u64);
        self.out.splice(at..at + LENGTH_RESERVED, length);
    }

    /// The frame: its body, its strings, their length backwards.
    pub fn finish(mut self) -> Vec<u8> {
        self.seal();
        self.out
    }

    /// The table and its length put after the body: `out` is the frame.
    fn seal(&mut self) {
        let table_len = self.table.len();
        self.out.extend_from_slice(&self.table);
        // The length written backwards: read from the frame's end.
        let at = self.out.len();
        varint(&mut self.out, table_len as u64);
        self.out[at..].reverse();
    }

    /// Starts over, keeping its buffers.
    fn clear(&mut self) {
        self.out.clear();
        self.table.clear();
        self.strings.clear();
    }
}

/// Reads a frame. Every read answers `None` for bytes that aren't what
/// [`Packer`] writes there.
pub struct Unpacker<'b> {
    body: &'b [u8],
    i: usize,
    table: &'b [u8],
    /// How far the table is read.
    table_at: usize,
    strings: Strings<'b>,
    /// Strings handed out shared ([`Unpacker::get_shared_str`]), by table index:
    /// a string a frame repeats is allocated once.
    shared: Vec<Option<Arc<str>>>,
    depth: usize,
}

/// The table's strings read so far: the first few in place, the rest on the heap.
struct Strings<'b> {
    inline: [&'b str; INLINE_STRINGS],
    len: usize,
    spill: Vec<&'b str>,
}

impl<'b> Strings<'b> {
    fn new() -> Self {
        Strings { inline: [""; INLINE_STRINGS], len: 0, spill: Vec::new() }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn push(&mut self, s: &'b str) {
        match self.inline.get_mut(self.len) {
            Some(slot) => *slot = s,
            None => self.spill.push(s),
        }
        self.len += 1;
    }

    fn get(&self, i: usize) -> Option<&'b str> {
        if i >= self.len {
            return None;
        }
        Some(if i < INLINE_STRINGS { self.inline[i] } else { self.spill[i - INLINE_STRINGS] })
    }
}

impl<'b> Unpacker<'b> {
    /// The frame's body and table found; the root value next.
    pub fn open(bytes: &'b [u8]) -> Option<Self> {
        let (table_len, trailer) = read_varint_back(bytes)?;
        let body_end = bytes.len().checked_sub(trailer)?.checked_sub(table_len)?;
        Some(Unpacker {
            body: &bytes[..body_end],
            i: 0,
            table: &bytes[body_end..bytes.len() - trailer],
            table_at: 0,
            strings: Strings::new(),
            shared: Vec::new(),
            depth: 0,
        })
    }

    /// Whether the body is read through.
    pub fn at_end(&self) -> bool {
        self.i == self.body.len()
    }

    fn take(&mut self, len: usize) -> Option<&'b [u8]> {
        let end = self.i.checked_add(len).filter(|&e| e <= self.body.len())?;
        let out = &self.body[self.i..end];
        self.i = end;
        Some(out)
    }

    pub fn get_u(&mut self) -> Option<u64> {
        read_varint(self.body, &mut self.i)
    }

    pub fn get_i(&mut self) -> Option<i64> {
        self.get_u().map(unzigzag)
    }

    pub fn get_bool(&mut self) -> Option<bool> {
        match self.take(1)? {
            [0] => Some(false),
            [1] => Some(true),
            _ => None,
        }
    }

    pub fn get_f64(&mut self) -> Option<f64> {
        Some(f64::from_le_bytes(self.take(F64_BYTES)?.try_into().ok()?))
    }

    pub fn get_f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(F32_BYTES)?.try_into().ok()?))
    }

    /// A count of things each at least `min` bytes: no bigger than what's left.
    fn count_of(&mut self, min: usize) -> Option<usize> {
        let n = usize::try_from(self.get_u()?).ok()?;
        (n <= (self.body.len() - self.i) / min.max(1)).then_some(n)
    }

    /// A list's count (every value takes a byte at least).
    pub fn count(&mut self) -> Option<usize> {
        self.count_of(1)
    }

    pub fn get_f64s(&mut self) -> Option<Vec<f64>> {
        let (_, narrow, raw) = self.packed_floats(true)?;
        Some(if narrow {
            raw.as_chunks::<F32_BYTES>().0.iter().map(|b| f64::from(f32::from_le_bytes(*b))).collect()
        } else {
            raw.as_chunks::<F64_BYTES>().0.iter().map(|b| f64::from_le_bytes(*b)).collect()
        })
    }

    /// Bit-packed numbers ([`Packer::put_uints`]), each made a `T` by `of`.
    pub fn get_uints<T>(&mut self, of: impl Fn(u64) -> Option<T>) -> Option<Vec<T>> {
        let (n, bits, raw) = self.packed_ints()?;
        let mask = u128::from(u64::MAX) >> (u64::BITS - bits);
        let mut bytes = raw.iter();
        let (mut acc, mut have) = (0u128, 0u32);
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            while have < bits {
                acc |= u128::from(*bytes.next()?) << have;
                have += u8::BITS;
            }
            out.push(of((acc & mask) as u64)?);
            acc >>= bits;
            have -= bits;
        }
        Some(out)
    }

    pub fn get_f32s(&mut self) -> Option<Vec<f32>> {
        let (_, _, raw) = self.packed_floats(false)?;
        Some(raw.as_chunks::<F32_BYTES>().0.iter().map(|b| f32::from_le_bytes(*b)).collect())
    }

    /// A packed list of ints, unread: how many, bits each, their bytes.
    pub fn packed_ints(&mut self) -> Option<(usize, u32, &'b [u8])> {
        let n = usize::try_from(self.get_u()?).ok()?;
        let bits = u32::from(*self.take(1)?.first()?);
        if !(1..=u64::BITS).contains(&bits) || n > (self.body.len() - self.i) * u8::BITS as usize / bits as usize {
            return None;
        }
        let bytes = self.take((n * bits as usize).div_ceil(u8::BITS as usize))?;
        Some((n, bits, bytes))
    }

    /// A list of floats, unread: how many, whether they're `f32`s, their
    /// bytes. A list of `f64` says whether it's narrowed (`wide`); one of
    /// `f32` always is.
    pub fn packed_floats(&mut self, wide: bool) -> Option<(usize, bool, &'b [u8])> {
        let n = self.count()?;
        let narrow = if wide { self.get_bool()? } else { true };
        let width = if narrow { F32_BYTES } else { F64_BYTES };
        Some((n, narrow, self.take(n.checked_mul(width)?)?))
    }

    /// A string, borrowed from the frame: the table read as far as it's needed.
    pub fn get_str(&mut self) -> Option<&'b str> {
        let i = self.string_index()?;
        self.strings.get(i)
    }

    /// A string as an `Arc<str>`: one allocation for every time the frame has it.
    pub fn get_shared_str(&mut self) -> Option<Arc<str>> {
        let i = self.string_index()?;
        if self.shared.len() <= i {
            self.shared.resize(i + 1, None);
        }
        let s = self.strings.get(i)?;
        Some(self.shared[i].get_or_insert_with(|| s.into()).clone())
    }

    /// A string's index, the table read up to it.
    fn string_index(&mut self) -> Option<usize> {
        let i = usize::try_from(self.get_u()?).ok()?;
        while self.strings.len() <= i {
            let len = usize::try_from(read_varint(self.table, &mut self.table_at)?).ok()?;
            let end = self.table_at.checked_add(len).filter(|&e| e <= self.table.len())?;
            let s = std::str::from_utf8(&self.table[self.table_at..end]).ok()?;
            self.table_at = end;
            self.strings.push(s);
        }
        Some(i)
    }

    pub fn get_bytes(&mut self) -> Option<&'b [u8]> {
        let len = self.count()?;
        self.take(len)
    }

    /// A value held in its binary form ([`Packer::put_tagged`]).
    pub fn get_tagged<T: Schema>(&mut self) -> Option<T> {
        let bytes = self.get_bytes()?;
        let mut d = Decoder::body(bytes);
        if let Some(v) = T::decode(&mut d)
            && d.at_end()
        {
            return Some(v);
        }
        let arena = Bump::new();
        let mut d = Decoder::body(bytes);
        let v = d.value_ref(&arena).ok()?;
        d.at_end().then_some(())?;
        read_ref_value(&v).ok()
    }

    /// A record starts: where it ends.
    pub fn begin(&mut self) -> Option<usize> {
        let len = self.count()?;
        self.depth += 1;
        (self.depth <= MAX_DEPTH).then_some(self.i + len)
    }

    /// Whether the record ending at `end` has more fields.
    pub fn more(&self, end: usize) -> bool {
        self.i < end
    }

    /// The record ending at `end` is done: fields this reader doesn't know skipped.
    pub fn leave(&mut self, end: usize) -> Option<()> {
        self.depth -= 1;
        (self.i <= end).then(|| self.i = end)
    }
}

/// A varint written backwards at the end of `bytes`: its value, and how
/// many bytes it took.
fn read_varint_back(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut n = 0u64;
    let mut shift = 0;
    for (taken, b) in bytes.iter().rev().enumerate() {
        if shift >= VARINT_MAX_SHIFT {
            return None;
        }
        n |= (u64::from(*b) & VARINT_BITS) << shift;
        if b & VARINT_MORE == 0 {
            return Some((usize::try_from(n).ok()?, taken + 1));
        }
        shift += VARINT_SHIFT;
    }
    None
}
