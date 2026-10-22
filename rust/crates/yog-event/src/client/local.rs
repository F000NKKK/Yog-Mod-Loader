//! What this client's player is and sees: the host's actions (nothing on
//! a dedicated server, nor without a player).

use yog_core::{BlockArea, BlockHit, DigSpeed, Point, BlockPos, LocalState, NearbyEntity, PlacedBlock, SlotStack};
use yog_ids as ids;

actions! {
    /// Where the player is and how it fares.
    State {  } -> Option<LocalState> = ids::service_client_local::STATE, "YogClientOps.state";
    /// The player's stacks, by slot: 0–8 the hotbar, 9–35 the rest, 36–39 armor (feet first), 40 the off hand.
    Inventory {  } -> Vec<SlotStack> = ids::service_client_local::INVENTORY, "YogClientOps.inventory";
    /// The loaded blocks among `ids` (`minecraft:diamond_ore`, `#minecraft:logs` for a tag) between `min` and `max`.
    Scan { min: BlockPos, max: BlockPos, ids: Vec<String> } -> Vec<PlacedBlock> = ids::service_client_local::SCAN, "YogClientOps.scan";
    /// The blocks between `min` and `max`, for finding a way; `None` for a box too big.
    Area { min: BlockPos, max: BlockPos } -> Option<BlockArea> = ids::service_client_local::AREA, "YogClientOps.area";
    /// The entities within `radius` blocks of the player (not the player).
    Entities { radius: f64 } -> Vec<NearbyEntity> = ids::service_client_local::ENTITIES, "YogClientOps.entities";
    /// How fast each of `blocks` (ids) is dug with the best tool in the hotbar, in order.
    DigSpeeds { blocks: Vec<String> } -> Vec<DigSpeed> = ids::service_client_local::DIG_SPEEDS, "YogDigSpeeds.of";
    /// The first block outline a ray from `from` to `to` hits in the client's world (fluids aside).
    Raycast { from: Point, to: Point } -> Option<BlockHit> = ids::service_client_local::RAYCAST, "YogClientOps.raycast";
    /// The block the crosshair is on, if it's on one.
    Target {  } -> Option<BlockHit> = ids::service_client_local::TARGET, "YogClientOps.target";
    /// Shows `text` in this player's own chat (no one else sees it).
    Message { text: String } -> bool = ids::service_client_local::MESSAGE, "YogClientChat.message";
}
