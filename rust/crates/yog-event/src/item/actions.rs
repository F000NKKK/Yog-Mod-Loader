//! Items and item stacks: the host's actions.

use crate::Point;
use yog_core::Stack;
use yog_ids as ids;

actions! {
    /// Drops a stack into the world; the item entity's uuid.
    SpawnItem { dim: String, pos: Point, velocity: Point, item: String, count: u32, snbt: String } -> Option<String> = ids::service_item::SPAWN, "spawnItem";
    /// An item entity's stack.
    EntityStack { uuid: String } -> Option<Stack> = ids::service_item::ENTITY_STACK, "YogQueries.itemEntityStack";
    /// Sets an item entity's stack.
    SetEntityStack { uuid: String, item: String, count: u32, snbt: String } -> bool = ids::service_item::SET_ENTITY_STACK, "itemEntitySetStack";
    /// The block `item` places, if any.
    Block { item: String } -> Option<String> = ids::service_item::BLOCK, "itemBlock";
}
