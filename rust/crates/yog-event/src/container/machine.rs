//! Machines: an inventory's `.yui` `<RecipeArea>` and the recipes its slots make.

use yog_core::BlockPos;
use yog_data::Schema;

use crate::pos::Pos;

/// A machine and the recipe its slots make. Pre: whether it may — the
/// result shows only if every handler allows it (a workbench short of a
/// resource says no); `player` empty. Post: a player took the result — the
/// mod takes its share (the host takes the ingredients).
#[derive(Debug, Clone, Schema)]
pub struct Craft {
    /// The machine's inventory (`mymod:press`).
    pub machine: String,
    /// The recipe's kind and id.
    pub kind: String,
    pub recipe: String,
    pub result_item: String,
    pub result_count: u32,
    /// The recipe's values (`{ "energy": 4 }`), as JSON.
    pub values: String,
    #[data(with = "Pos")]
    pub pos: BlockPos,
    pub dimension: String,
    /// Who took the result; empty before.
    pub player: String,
}
