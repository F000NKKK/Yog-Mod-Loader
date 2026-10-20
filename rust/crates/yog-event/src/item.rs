//! Items' services: what a mod says about its items' stacks.

use yog_data::Schema;

/// A stack's own name and tooltip, from its custom data (a written
/// blueprint shows its design) — keyed by the item, answered by its mod;
/// `None`: the item's own.
#[derive(Debug, Clone, Schema)]
pub struct Text {
    pub item: String,
    /// The stack's custom data, as SNBT.
    pub snbt: String,
}

/// What a stack shows instead of its item's own name and description.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct Shown {
    /// Its name; `None`: the item's own.
    pub name: Option<String>,
    /// The tooltip lines that stand instead of its description.
    pub lines: Vec<String>,
}
