//! Menu entries mods declare: buttons the host puts on the game's screens.

use yog_data::Schema;

/// A button (the `menu/declare` service) the host shows on the title screen (the mod list on Forge and
/// NeoForge), opening a mod's screen.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct MenuEntry {
    /// Its text, a translation key.
    pub label: String,
    /// The screen it opens (`mymod:menu`).
    pub ui_id: String,
}

/// Every declared menu entry, for the host.
#[derive(Debug, Clone, Schema)]
pub struct Declared {}
