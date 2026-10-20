//! A block's loot table: what breaking it drops.

use serde_json::{Value, json};
use yog_formats::content::{BlockKind, DropCount};
use yog_ids::loot as l;

use crate::content::model::state_properties;
use crate::content::num_value;

/// What a double block (a double slab) drops of itself.
const DOUBLE: u32 = 2;

/// The loot table dropping `item` (`count` of it), when it survives the explosion.
pub(in crate::content) fn drops_table(item: &str, count: Option<DropCount>) -> Value {
    let mut entry = json!({ l::TYPE: l::ITEM_ENTRY, l::NAME: item });
    if let Some(c) = count {
        let count = match c {
            DropCount::Exactly(n) => num_value(n),
            DropCount::Between(min, max) => {
                json!({ l::TYPE: l::UNIFORM, l::MIN: num_value(min), l::MAX: num_value(max) })
            }
        };
        entry[l::FUNCTIONS] = json!([{ l::FUNCTION: l::SET_COUNT, l::COUNT: count }]);
    }
    json!({
        l::TYPE: l::BLOCK_TABLE,
        l::POOLS: [{
            l::ROLLS: 1,
            l::ENTRIES: [entry],
            l::CONDITIONS: [{ l::CONDITION: l::SURVIVES_EXPLOSION }]
        }]
    })
}

/// What a block of a game's kind drops: itself — only in its `drops_when`
/// states (a door's bottom half), two in its `drops_two_when` (a double slab).
pub(in crate::content) fn kind_drops(id: &str, k: &BlockKind) -> Value {
    let mut table = drops_table(id, None);
    let when = |state: &str| json!({ l::CONDITION: l::BLOCK_STATE_PROPERTY, l::BLOCK: id, l::PROPERTIES: state_properties(state) });
    let pool = &mut table[l::POOLS][0];
    if let Some(state) = &k.drops_when
        && let Some(conditions) = pool[l::CONDITIONS].as_array_mut()
    {
        conditions.insert(0, when(state));
    }
    if let Some(state) = &k.drops_two_when {
        // As the game's slab: two when double, and explosions take their share.
        pool.as_object_mut().map(|o| o.remove(l::CONDITIONS));
        pool[l::ENTRIES][0][l::FUNCTIONS] = json!([
            { l::FUNCTION: l::SET_COUNT, l::COUNT: DOUBLE, l::CONDITIONS: [when(state)] },
            { l::FUNCTION: l::EXPLOSION_DECAY }
        ]);
    }
    table
}
