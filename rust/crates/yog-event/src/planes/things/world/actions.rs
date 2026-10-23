//! The host's actions on a world.

use yog_core::{BlockHit, BlockPos, PlacedBlock};
use yog_ids as ids;

use crate::Point;

actions! {
    /// The block at `pos` (`minecraft:stone`); `None` without the dimension.
    GetBlock { dim: String, pos: BlockPos } -> Option<String> = ids::service_world::GET_BLOCK, "YogWorldActions.getBlock";
    /// Sets the block at `pos`; whether it did.
    SetBlock { dim: String, pos: BlockPos, block: String } -> bool = ids::service_world::SET_BLOCK, "YogWorldActions.setBlock";
    /// The time of day of `dim`; `None` without the dimension.
    Time { dim: String } -> Option<i64> = ids::service_world::TIME, "YogWorldActions.worldTime";
    /// Sets the time of day (0 dawn, 6000 noon, 12000 dusk, 18000 midnight).
    SetTime { dim: String, time: i64 } -> bool = ids::service_world::SET_TIME, "YogWorldActions.worldSetTime";
    /// Whether it rains in `dim`.
    IsRaining { dim: String } -> bool = ids::service_world::IS_RAINING, "YogWorldActions.worldIsRaining";
    /// Starts or stops rain for `duration` ticks.
    SetWeather { dim: String, raining: bool, duration: i32 } -> bool = ids::service_world::SET_WEATHER, "YogWorldActions.worldSetWeather";
    /// Whether it thunders in `dim`.
    IsThundering { dim: String } -> bool = ids::service_world::IS_THUNDERING, "YogWorldActions.worldIsThundering";
    /// Starts or stops thunder.
    SetThunder { dim: String, thundering: bool, duration: i32 } -> bool = ids::service_world::SET_THUNDER, "YogWorldActions.worldSetThunder";
    /// Fills a box with `state`; how many blocks changed (`None`: failed).
    FillRegion { dim: String, from: BlockPos, to: BlockPos, state: String } -> Option<u32> = ids::service_world::FILL_REGION, "YogQueries.fillRegion";
    /// The non-air blocks in a box.
    ScanRegion { dim: String, from: BlockPos, to: BlockPos } -> Vec<PlacedBlock> = ids::service_world::SCAN_REGION, "YogQueries.scanRegion";
    /// Places a configured feature at `pos`.
    PlaceFeature { dim: String, pos: BlockPos, feature: String } -> bool = ids::service_world::PLACE_FEATURE, "YogWorldActions.placeFeature";
    /// Makes an explosion.
    Explode { dim: String, pos: Point, power: f32, fire: bool, source: String } -> bool = ids::service_world::EXPLODE, "YogWorldActions.explode";
    /// Rolls a loot table and drops its items at `pos`.
    DropLoot { table: String, dim: String, pos: Point } -> bool = ids::service_world::DROP_LOOT, "YogWorldActions.dropLoot";
    /// How many entities of `entity_type` are in `dim` (`None`: no such dimension or type).
    EntityCount { dim: String, entity_type: String } -> Option<u32> = ids::service_world::ENTITY_COUNT, "YogQueries.worldEntityCount";
    /// The uuids of the entities in a box.
    EntitiesInBox { dim: String, min: Point, max: Point } -> Vec<String> = ids::service_world::ENTITIES_IN_BOX, "YogQueries.entitiesInBox";
    /// The first block a ray hits.
    RaycastBlock { dim: String, from: Point, to: Point } -> Option<BlockHit> = ids::service_world::RAYCAST_BLOCK, "YogQueries.raycastBlock";
    /// The first entity a ray hits: its uuid.
    RaycastEntity { dim: String, from: Point, to: Point, exclude: String } -> Option<String> = ids::service_world::RAYCAST_ENTITY, "YogWorldActions.raycastEntity";
}
