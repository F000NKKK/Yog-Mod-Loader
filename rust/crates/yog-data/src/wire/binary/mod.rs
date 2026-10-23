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
//! | 17 | stream | u32 byte length, then varint count and values with their own strings |
//! | 18 | float, 32-bit | f32, little-endian |
//! | 19 | list of floats, packed | varint count, f64 each, little-endian |
//! | 20 | list of 32-bit floats, packed | varint count, f32 each, little-endian |
//!
//! Nothing is aligned: numbers are read out of the bytes wherever they are,
//! so no padding is ever written.
//!
//! Varints are LEB128. Decoding checks every length against the input, so
//! malformed bytes are an error, never a panic or a huge allocation.
//!
//! A [`crate::Schema`] writes itself here straight ([`crate::Schema::encode`])
//! and reads itself back the same way ([`crate::Schema::decode`]) — no value
//! tree on the way.

mod decoder;
pub(crate) mod interner;
#[cfg(test)]
mod tests;
pub(crate) mod writer;

use std::fmt;

use bumpalo::Bump;

pub use decoder::Decoder;
pub use writer::Writer;

use crate::model::borrowed::ValueRef;
use crate::model::value::Value;

pub const MAGIC: &[u8; 4] = b"YOGD";
pub const VERSION: u8 = 2;

/// Nested deeper than this is malformed.
pub const MAX_DEPTH: usize = 256;

/// A value's first byte: what it is.
pub mod tag {
    /// The tags, each a constant, and [`ALL`] of them by name.
    macro_rules! tags {
        ($($(#[$doc:meta])* $name:ident = $value:expr;)*) => {
            $($(#[$doc])* pub const $name: u8 = $value;)*
            /// Every tag, by its name (what other languages' readers are generated from).
            pub const ALL: &[(&str, u8)] = &[$((stringify!($name), $value)),*];
        };
    }

    tags! {
        NULL = 0;
        FALSE = 1;
        TRUE = 2;
        INT = 3;
        FLOAT = 4;
        STR = 5;
        LIST = 6;
        MAP = 7;
        REF = 8;
        TAG = 9;
        RES = 10;
        TEXT = 11;
        IMAGE = 12;
        COLOR = 13;
        LANG = 14;
        BYTES = 15;
        /// A list of ints without a tag each: what a schema of numbers writes its lists as.
        INTS = 16;
        /// A list in a body of its own (its own strings): read lazily, cut out whole ([`crate::Stream`]).
        STREAM = 17;
        FLOAT32 = 18;
        /// Lists of floats without a tag each.
        FLOATS = 19;
        FLOATS32 = 20;
    }
}

/// A stream's length: fixed width, so the writer fills it in after its items.
pub(crate) const STREAM_LEN_BYTES: usize = 4;

/// A string's varint: a new one's (odd, its length) or a reference to the
/// table (even, its index).
pub(crate) const NEW_STRING: u64 = 1;

/// The low seven bits of a varint byte, and the bit saying more follow.
pub(crate) const VARINT_BITS: u64 = 0x7F;
pub(crate) const VARINT_MORE: u8 = 0x80;
pub(crate) const VARINT_SHIFT: u32 = 7;

pub(crate) fn varint(out: &mut Vec<u8>, mut n: u64) {
    while n > VARINT_BITS {
        out.push((n as u8) | VARINT_MORE);
        n >>= VARINT_SHIFT;
    }
    out.push(n as u8);
}

/// The bits a varint can hold.
pub(crate) const VARINT_MAX_SHIFT: u32 = 64;

/// The varint at `*i` in `b`, `*i` moved past it; `None` cut short or longer than 64 bits.
pub(crate) fn read_varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut n = 0u64;
    let mut shift = 0;
    while shift < VARINT_MAX_SHIFT {
        let byte = *b.get(*i)?;
        *i += 1;
        n |= (u64::from(byte) & VARINT_BITS) << shift;
        if byte & VARINT_MORE == 0 {
            return Some(n);
        }
        shift += VARINT_SHIFT;
    }
    None
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
