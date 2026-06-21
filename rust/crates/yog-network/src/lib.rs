//! Networking — custom packets as raw bytes over named channels.
//!
//! Yog never puts NBT on the wire: a packet payload is just `Vec<u8>`, so mod
//! authors serialize with whatever is fastest for them (bincode, protobuf,
//! FlatBuffers, plain bytes). NBT is only ever built when something must be
//! handed to the game itself.

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
