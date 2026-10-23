//! Loot tables rolled.

/// A loot table rolled — a chest's, a block's drops, a mob's — before its
/// items are given: a handler changes `items`, each seeing the one before's.
pub use yog_registry::Loot as Roll;
pub use yog_registry::LootStack as Stack;
