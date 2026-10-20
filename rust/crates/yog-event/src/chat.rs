//! Messages to players: the host's actions.

use yog_ids as ids;

actions! {
    /// Sends `message` to every player.
    Broadcast { message: String } -> bool = ids::service_chat::BROADCAST, "broadcast";
}
