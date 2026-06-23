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
}

macro_rules! impl_fixed {
    ($T:ty, $N:literal) => {
        impl PacketField for $T {
            fn write_to(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_le_bytes());
            }
            fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self> {
                let end = pos.checked_add($N)?;
                if end > buf.len() { return None; }
                let v = Self::from_le_bytes(buf[*pos..end].try_into().ok()?);
                *pos = end;
                Some(v)
            }
        }
    };
}

impl_fixed!(i32, 4);
impl_fixed!(i64, 8);
impl_fixed!(u32, 4);
impl_fixed!(u64, 8);
impl_fixed!(f32, 4);
impl_fixed!(f64, 8);

impl PacketField for bool {
    fn write_to(&self, buf: &mut Vec<u8>) { buf.push(*self as u8); }
    fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let b = *buf.get(*pos)?;
        *pos += 1;
        Some(b != 0)
    }
}

impl PacketField for String {
    fn write_to(&self, buf: &mut Vec<u8>) {
        let bytes = self.as_bytes();
        (bytes.len() as u32).write_to(buf);
        buf.extend_from_slice(bytes);
    }
    fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let len = u32::read_from(buf, pos)? as usize;
        let end = pos.checked_add(len)?;
        if end > buf.len() { return None; }
        let s = std::str::from_utf8(&buf[*pos..end]).ok()?.to_owned();
        *pos = end;
        Some(s)
    }
}

impl PacketField for Vec<u8> {
    fn write_to(&self, buf: &mut Vec<u8>) {
        (self.len() as u32).write_to(buf);
        buf.extend_from_slice(self);
    }
    fn read_from(buf: &[u8], pos: &mut usize) -> Option<Self> {
        let len = u32::read_from(buf, pos)? as usize;
        let end = pos.checked_add(len)?;
        if end > buf.len() { return None; }
        let v = buf[*pos..end].to_vec();
        *pos = end;
        Some(v)
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
                let mut buf = ::std::vec::Vec::new();
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
