//! Messages to players: the host's actions.

use yog_ids as ids;

actions! {
    /// Sends `message` to every player.
    Broadcast { message: String } -> bool = ids::service_chat::BROADCAST, "broadcast";
    /// Sends `json` (a text component) to `player`'s chat.
    Tellraw { player: String, json: String } -> bool = ids::service_chat::TELLRAW, "tellraw";
    /// Shows a title (and subtitle) to `player`; times in ticks.
    Title { player: String, title: String, subtitle: String, fade_in: i32, stay: i32, fade_out: i32 } -> bool = ids::service_chat::TITLE, "sendTitle";
    /// Shows `message` above `player`'s hotbar.
    Actionbar { player: String, message: String } -> bool = ids::service_chat::ACTIONBAR, "sendActionbar";
    /// Shows a text component above `player`'s hotbar.
    ActionbarJson { player: String, json: String } -> bool = ids::service_chat::ACTIONBAR_JSON, "actionbarJson";
}
