//! Networking — custom packets as raw bytes over named channels.
//!
//! Yog never puts NBT on the wire: a packet payload is just `Vec<u8>`, so mod
//! authors serialize with whatever is fastest for them (bincode, protobuf,
//! FlatBuffers, plain bytes). NBT is only ever built when something must be
//! handed to the game itself.
//!
//! For ergonomic typed packets, use the [`packet!`] macro:
//!
//! ```
//! use yog_network::{packet, Packet};
//!
//! packet! {
//!     pub struct TeleportPacket {
//!         x: f64,
//!         y: f64,
//!         z: f64,
//!         player: String,
//!     }
//! }
//!
//! let pkt = TeleportPacket { x: 0.0, y: 64.0, z: 0.0, player: "Steve".into() };
//! let bytes = pkt.encode();
//! let decoded = TeleportPacket::decode(&bytes).unwrap();
//! assert_eq!(decoded.player, "Steve");
//! ```

/// A packet received on a channel.
#[derive(Debug, Clone)]
pub struct PacketEvent {
    /// Channel id, e.g. `mymod:sync`.
    pub channel: String,
    /// Sender's player name on the server side; empty for packets the client
    /// received from the server.
    pub player: String,
    /// Raw payload bytes.
    pub payload: Vec<u8>,
}

// ── Typed packet support ──────────────────────────────────────────────────────

/// Encode/decode a single field to/from a byte buffer.
///
/// Implemented for the types supported by [`packet!`]:
/// `bool`, `i32`, `i64`, `u32`, `u64`, `f32`, `f64`, `String`, `Vec<u8>`.
pub trait PacketField: Sized {
    fn write_to(&self, buf: &mut Vec<u8>);
    fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self>;
    /// Exact number of bytes [`write_to`](Self::write_to) appends; lets
    /// [`packet!`] allocate the buffer once. A low estimate is still correct,
    /// just slower.
    fn encoded_len(&self) -> usize {
        0
    }
}

/// Byte width of the length prefix in front of strings and byte blobs.
const LEN_PREFIX: usize = std::mem::size_of::<u32>();

/// The next `len` bytes at `*pos`, advancing past them.
#[inline]
fn take<'a>(buf: &'a [u8], pos: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = pos.checked_add(len)?;
    let bytes = buf.get(*pos..end)?;
    *pos = end;
    Some(bytes)
}

/// A `u32`-length-prefixed byte run at `*pos`.
#[inline]
fn take_prefixed<'a>(buf: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    let len = u32::read_from(buf, pos)? as usize;
    take(buf, pos, len)
}

fn write_prefixed(bytes: &[u8], buf: &mut Vec<u8>) {
    (bytes.len() as u32).write_to(buf);
    buf.extend_from_slice(bytes);
}

macro_rules! impl_fixed {
    ($($T:ty),*) => {$(
        impl PacketField for $T {
            fn write_to(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_le_bytes());
            }
            fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self> {
                const N: usize = std::mem::size_of::<$T>();
                let bytes: [u8; N] = buf.get(*pos..pos.checked_add(N)?)?.try_into().ok()?;
                *pos += N;
                Some(Self::from_le_bytes(bytes))
            }
            fn encoded_len(&self) -> usize {
                std::mem::size_of::<$T>()
            }
        }
    )*};
}

impl_fixed!(i32, i64, u32, u64, f32, f64);

impl PacketField for bool {
    fn write_to(&self, buf: &mut Vec<u8>) {
        buf.push(*self as u8);
    }
    fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let b = *buf.get(*pos)?;
        *pos += 1;
        Some(b != 0)
    }
    fn encoded_len(&self) -> usize {
        1
    }
}

impl PacketField for String {
    fn write_to(&self, buf: &mut Vec<u8>) {
        write_prefixed(self.as_bytes(), buf);
    }
    fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self> {
        std::str::from_utf8(take_prefixed(buf, pos)?)
            .ok()
            .map(str::to_owned)
    }
    fn encoded_len(&self) -> usize {
        LEN_PREFIX + self.len()
    }
}

impl PacketField for Vec<u8> {
    fn write_to(&self, buf: &mut Vec<u8>) {
        write_prefixed(self, buf);
    }
    fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self> {
        take_prefixed(buf, pos).map(<[u8]>::to_vec)
    }
    fn encoded_len(&self) -> usize {
        LEN_PREFIX + self.len()
    }
}

/// A typed packet that can be encoded to / decoded from a raw byte buffer.
///
/// Implement this trait via the [`packet!`] macro — do not implement manually.
pub trait Packet: Sized {
    fn encode(&self) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Option<Self>;
}

/// Declare a typed packet struct whose fields are automatically encoded/decoded.
///
/// All field types must implement [`PacketField`]: `bool`, `i32`, `i64`,
/// `u32`, `u64`, `f32`, `f64`, `String`, `Vec<u8>`.
///
/// ```
/// use yog_network::{packet, Packet};
///
/// packet! {
///     pub struct PingPacket {
///         seq: u32,
///         message: String,
///     }
/// }
///
/// let p = PingPacket { seq: 1, message: "hello".into() };
/// let decoded = PingPacket::decode(&p.encode()).unwrap();
/// assert_eq!(decoded.seq, 1);
/// ```
#[macro_export]
macro_rules! packet {
    (
        $(#[$meta:meta])*
        $vis:vis struct $Name:ident {
            $( $fvis:vis $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $Name {
            $( $fvis $field : $ty, )*
        }

        impl $crate::Packet for $Name {
            fn encode(&self) -> ::std::vec::Vec<u8> {
                let mut buf = ::std::vec::Vec::with_capacity(
                    0 $( + $crate::PacketField::encoded_len(&self.$field) )*
                );
                $( $crate::PacketField::write_to(&self.$field, &mut buf); )*
                buf
            }
            fn decode(bytes: &[u8]) -> ::std::option::Option<Self> {
                let mut pos = 0usize;
                $(
                    let $field = <$ty as $crate::PacketField>::read_from(bytes, &mut pos)?;
                )*
                ::std::option::Option::Some(Self { $( $field, )* })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    packet! {
        struct Mixed {
            a: bool,
            b: i32,
            c: f64,
            s: String,
            v: Vec<u8>,
        }
    }

    #[test]
    fn round_trip_and_exact_capacity() {
        let p = Mixed {
            a: true,
            b: -7,
            c: 1.5,
            s: "hé".into(),
            v: vec![1, 2, 3],
        };
        let bytes = p.encode();
        assert_eq!(bytes.len(), bytes.capacity());
        let d = Mixed::decode(&bytes).unwrap();
        assert!(d.a && d.b == -7 && d.c == 1.5 && d.s == "hé" && d.v == [1, 2, 3]);
    }

    #[test]
    fn truncated_input_is_rejected() {
        let bytes = Mixed {
            a: false,
            b: 1,
            c: 2.0,
            s: "x".into(),
            v: vec![9; 8],
        }
        .encode();
        for cut in 0..bytes.len() {
            assert!(Mixed::decode(&bytes[..cut]).is_none(), "cut at {cut}");
        }
    }
}
