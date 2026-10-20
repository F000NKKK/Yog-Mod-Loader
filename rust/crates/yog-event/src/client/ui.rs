//! Screens opened and closed.

use yog_data::Schema;

/// Fired when a GUI screen opens.
#[derive(Debug, Clone, Schema)]
pub struct Open {
    /// Simple class name of the screen, e.g. `"InventoryScreen"`, `"ChestScreen"`.
    pub screen_class: String,
}

/// Fired when a GUI screen closes.
#[derive(Debug, Clone, Schema)]
pub struct Close {
    /// Simple class name of the screen.
    pub screen_class: String,
}

/// Input on a mod's screen; keyed by the screen's id. `event` is
/// `click:X:Y`, `drag:X:Y`, `release:X:Y`, `scroll:DY`, `key:KEY:MODS`
/// (GLFW key code and modifier bits, on press and repeat) or
/// `char:CODEPOINT`. Before: a handler that used a `key:`/`char:` cancels
/// it — the host then skips its own handling (Esc closing the screen).
#[derive(Debug, Clone, Schema)]
pub struct Input {
    pub ui_id: String,
    pub event: String,
}
