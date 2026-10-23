//! Mods' screens: the host's actions.

use yog_ids as ids;

actions! {
    /// Opens screen `ui_id`; `modal` blocks the game's input, `pause` pauses singleplayer.
    Open { ui_id: String, modal: bool, pause: bool } -> bool = ids::service_ui::OPEN, "actions.YogUiActions.openUI";
    /// Opens screen `ui_id` with [`flags`] bits.
    OpenWithFlags { ui_id: String, flags: i32 } -> bool = ids::service_ui::OPEN_WITH_FLAGS, "client.YogClientApi.openUI";
    /// Closes screen `ui_id` if it's the one open.
    Close { ui_id: String } -> bool = ids::service_ui::CLOSE, "actions.YogUiActions.closeUI";
}

/// Flag bits for [`OpenWithFlags`].
pub mod flags {
    /// Blocks game input (what [`super::Open`]'s `modal` does).
    pub const MODAL: u32 = 1;
    /// Pauses a singleplayer game.
    pub const PAUSE: u32 = 1 << 1;
    /// Draws no background behind the screen: the world stays visible.
    pub const NO_BACKGROUND: u32 = 1 << 2;
}
