//! The binary form — what the build compiles content to, what the bus
//! carries, and what the runtime and the Java hosts read, like NBT:
//!
//! ```text
//! "YOGD" version:u8
//! root value
//! ```
//!
//! A value is a tag byte and its payload. A string is written where it's
//! first used and referred to after that: a varint `v`, then — when `v` is
//! odd — `v >> 1` bytes of UTF-8, the next entry of the string table; when
//! it's even, the table's entry `v >> 1`. Repeated keys and ids cost one
//! varint each, and the writer never goes back over what it wrote.
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
//! | 15 | bytes | varint length, the bytes |
//! | 16 | list of ints, packed | varint count, zigzag varints |
//!
//! Varints are LEB128. Decoding checks every length against the input, so
//! malformed bytes are an error, never a panic or a huge allocation.
//!
//! A [`crate::Schema`] writes itself here straight ([`crate::Schema::encode`])
//! and reads itself back the same way ([`crate::Schema::decode`]) — no value
//! tree on the way.

mod decoder;
mod writer;
#[cfg(test)]
mod tests;

use std::fmt;

use bumpalo::Bump;

pub use decoder::Decoder;
pub use writer::Writer;

use crate::borrowed::ValueRef;
use crate::value::Value;

pub const MAGIC: &[u8; 4] = b"YOGD";
pub const VERSION: u8 = 2;

/// Nested deeper than this is malformed.
pub const MAX_DEPTH: usize = 256;

pub(crate) mod tag {
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
    pub const BYTES: u8 = 15;
    /// A list of ints without a tag each: what a schema of numbers writes its lists as.
    pub const INTS: u8 = 16;
}

/// A string's varint: a new one's (odd, its length) or a reference to the
/// table (even, its index).
const NEW_STRING: u64 = 1;

/// The low seven bits of a varint byte, and the bit saying more follow.
const VARINT_BITS: u64 = 0x7F;
const VARINT_MORE: u8 = 0x80;
const VARINT_SHIFT: u32 = 7;

pub(crate) fn varint(out: &mut Vec<u8>, mut n: u64) {
    while n > VARINT_BITS {
        out.push((n as u8) | VARINT_MORE);
        n >>= VARINT_SHIFT;
    }
    out.push(n as u8);
}

/// `n` zigzagged: small negative numbers stay small varints.
pub(crate) fn zigzag(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

pub(crate) fn unzigzag(z: u64) -> i64 {
    ((z >> 1) as i64) ^ -((z & 1) as i64)
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

/// `value` in the binary form.
pub fn encode(value: &Value) -> Vec<u8> {
    let mut w = Writer::new();
    w.value(value);
    w.finish()
}

/// Reads the binary form.
pub fn decode(bytes: &[u8]) -> Result<Value, DecodeError> {
    let mut d = Decoder::open(bytes)?;
    let v = d.value()?;
    d.end()?;
    Ok(v)
}

/// Reads the binary form borrowed: every string points into `bytes`,
/// lists and maps go in `arena` — nothing is copied.
pub fn decode_ref<'a>(bytes: &'a [u8], arena: &'a Bump) -> Result<ValueRef<'a>, DecodeError> {
    let mut d = Decoder::open(bytes)?;
    let v = d.value_ref(arena)?;
    d.end()?;
    Ok(v)
}
