//! Key bindings mods declare: keys the player sees and rebinds in the
//! game's controls; presses come as `client::input::Binding`.

use yog_data::Schema;

/// A key binding (the `key_binding/declare` service, declared while the mod registers; the game keeps the key).
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct KeyBinding {
    /// Its id (`mymod:xray`), the key of its presses.
    pub id: String,
    /// Its label's translation key.
    pub name: String,
    /// A GLFW key code; `-1`: unbound.
    pub default_key: i32,
}

/// Every declared key binding, for the host.
#[derive(Debug, Clone, Schema)]
pub struct Declared {}

/// A declared key binding as the host registers it: under its mod's category.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct CategorizedBinding {
    pub id: String,
    pub name: String,
    pub default_key: i32,
    /// The category's label: its mod's name (its namespace when no mod has it).
    pub category: String,
}
