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
