//! Entities living in the game.

use yog_data::Schema;

/// A tick of an entity of a mod's type (one with `ticks`); keyed by its
/// type — handlers hear only their own type's.
#[derive(Debug, Clone, Schema)]
pub struct Tick {
    /// The entity's type (`mymod:golem`).
    pub entity_type: String,
    pub uuid: String,
}
