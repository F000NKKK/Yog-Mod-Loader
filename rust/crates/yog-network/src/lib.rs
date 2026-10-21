//! Networking — mods' packets over named channels, as Yog Data.
//!
//! A packet's payload is bytes; a typed packet is any [`Schema`] type, sent
//! and read the way everything else Yog sends is (`yog_data::to_bytes` /
//! `read_bytes`). NBT is only ever built when something must be handed to
//! the game itself.
//!
//! ```
//! use yog_data::Schema;
//! use yog_network::Packet;
//!
//! #[derive(Schema, Debug, PartialEq)]
//! pub struct Teleport {
//!     x: f64,
//!     y: f64,
//!     z: f64,
//!     player: String,
//! }
//!
//! let pkt = Teleport { x: 0.0, y: 64.0, z: 0.0, player: "Steve".into() };
//! assert_eq!(Teleport::from_payload(&pkt.to_payload()), Some(pkt));
//! ```

use yog_data::Schema;

/// A packet's payload as bytes and back — every [`Schema`] type is one.
pub trait Packet: Sized {
    fn to_payload(&self) -> Vec<u8>;
    /// `None` for bytes that aren't this packet.
    fn from_payload(bytes: &[u8]) -> Option<Self>;
}

impl<T: Schema> Packet for T {
    fn to_payload(&self) -> Vec<u8> {
        yog_data::to_bytes(self)
    }

    fn from_payload(bytes: &[u8]) -> Option<Self> {
        yog_data::read_bytes(bytes).ok()
    }
}
