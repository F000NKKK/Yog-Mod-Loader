//! What happens to blocks.

use yog_core::BlockPos;
use yog_data::Schema;

use crate::pos::Pos;

/// Fired when a player breaks a block (server side).
#[derive(Debug, Clone, Schema)]
pub struct Break {
    pub player_name: String,
    /// Registry id of the block, e.g. `minecraft:stone`.
    pub block_id: String,
    #[data(with = "Pos")]
    pub pos: BlockPos,
    /// Dimension the block was broken in, e.g. `minecraft:overworld`.
    pub dimension: String,
}

/// Fired when a player places a block (server side).
///
/// Passed to handlers registered via `Registry::on_player_place_block` with an
/// [`EventPhase`] argument:
/// - `Pre`  — fires before placement; return `false` to cancel.
/// - `Post` — fires after the block has been placed.
#[derive(Debug, Clone, Schema)]
pub struct Place {
    pub player_name: String,
    /// Registry id of the block being placed, e.g. `minecraft:stone`.
    pub block_id: String,
    #[data(with = "Pos")]
    pub pos: BlockPos,
    pub dimension: String,
    /// Whether the player was sneaking (shift).
    pub sneaking: bool,
}

/// Fired when a player right-clicks a block (server side).
#[derive(Debug, Clone, Schema)]
pub struct Use {
    pub player_name: String,
    /// Registry id of the targeted block, e.g. `minecraft:chest`.
    pub block_id: String,
    #[data(with = "Pos")]
    pub pos: BlockPos,
    pub dimension: String,
    /// Whether the player was sneaking (shift).
    pub sneaking: bool,
}

/// A hopper or dropper offers an item to a block that accepts items
/// (`block.accepts_items`): a service each such block provides (keyed by
/// its id), answering whether it takes the item.
#[derive(Debug, Clone, Schema)]
pub struct Insert {
    /// Registry id of the block.
    pub block_id: String,
    #[data(with = "Pos")]
    pub pos: BlockPos,
    pub dimension: String,
    pub item_id: String,
    pub count: u32,
    /// The item's data as SNBT (`{}` for none).
    pub nbt: String,
    /// Only whether it would take it — nothing changes yet; else it takes it.
    pub simulate: bool,
}
