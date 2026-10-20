//! Sending over the network: the host's actions.

use yog_data::with::Raw;
use yog_ids as ids;

actions! {
    /// Sends `payload` on `channel` to `player` (server side).
    SendToPlayer { player: String, channel: String, #[data(with = "Raw")] payload: Vec<u8> } -> bool = ids::service_net::SEND_TO_PLAYER, "sendToPlayer";
    /// Sends `payload` on `channel` to the server (client side).
    SendToServer { channel: String, #[data(with = "Raw")] payload: Vec<u8> } -> bool = ids::service_net::SEND_TO_SERVER, "YogClient.sendToServer";
}
