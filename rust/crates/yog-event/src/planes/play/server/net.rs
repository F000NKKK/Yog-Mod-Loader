//! What the server hears over the network.

use yog_data::Schema;
use yog_data::with::Raw;

/// A packet a client sent on a mod's channel; keyed by the channel.
#[derive(Debug, Clone, Schema)]
pub struct Packet {
    /// The channel (`mymod:sync`).
    pub channel: String,
    /// The sender's name.
    pub player: String,
    /// The bytes as the mod wrote them.
    #[data(with = "Raw")]
    pub payload: Vec<u8>,
}
