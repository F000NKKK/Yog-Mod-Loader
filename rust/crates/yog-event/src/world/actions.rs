//! The host's actions on a world.

use yog_core::BlockPos;
use yog_ids as ids;

use crate::Point;
use crate::pos::Pos;

actions! {
    /// The block at `pos` (`minecraft:stone`); `None` without the dimension.
    GetBlock { dim: String, #[data(with = "Pos")] pos: BlockPos } -> Option<String> = ids::service_world::GET_BLOCK, "getBlock";
    /// Sets the block at `pos`; whether it did.
    SetBlock { dim: String, #[data(with = "Pos")] pos: BlockPos, block: String } -> bool = ids::service_world::SET_BLOCK, "setBlock";
    /// The time of day of `dim`; `None` without the dimension.
    Time { dim: String } -> Option<i64> = ids::service_world::TIME, "worldTime";
    /// Sets the time of day (0 dawn, 6000 noon, 12000 dusk, 18000 midnight).
    SetTime { dim: String, time: i64 } -> bool = ids::service_world::SET_TIME, "worldSetTime";
    /// Whether it rains in `dim`.
    IsRaining { dim: String } -> bool = ids::service_world::IS_RAINING, "worldIsRaining";
    /// Starts or stops rain for `duration` ticks.
    SetWeather { dim: String, raining: bool, duration: i32 } -> bool = ids::service_world::SET_WEATHER, "worldSetWeather";
    /// Whether it thunders in `dim`.
    IsThundering { dim: String } -> bool = ids::service_world::IS_THUNDERING, "worldIsThundering";
    /// Starts or stops thunder.
    SetThunder { dim: String, thundering: bool, duration: i32 } -> bool = ids::service_world::SET_THUNDER, "worldSetThunder";
    /// Fills a box with `state`; how many blocks changed (−1: failed).
    FillRegion { dim: String, #[data(with = "Pos")] from: BlockPos, #[data(with = "Pos")] to: BlockPos, state: String } -> i32 = ids::service_world::FILL_REGION, "fillRegion";
    /// The non-air blocks in a box: `x y z state` a line.
    ScanRegion { dim: String, #[data(with = "Pos")] from: BlockPos, #[data(with = "Pos")] to: BlockPos } -> Option<String> = ids::service_world::SCAN_REGION, "scanRegion";
    /// Places a configured feature at `pos`.
    PlaceFeature { dim: String, #[data(with = "Pos")] pos: BlockPos, feature: String } -> bool = ids::service_world::PLACE_FEATURE, "placeFeature";
    /// Makes an explosion.
    Explode { dim: String, pos: Point, power: f32, fire: bool, source: String } -> bool = ids::service_world::EXPLODE, "explode";
    /// Rolls a loot table and drops its items at `pos`.
    DropLoot { table: String, dim: String, pos: Point } -> bool = ids::service_world::DROP_LOOT, "dropLoot";
    /// How many entities of `entity_type` are in `dim`.
    EntityCount { dim: String, entity_type: String } -> i32 = ids::service_world::ENTITY_COUNT, "worldEntityCount";
    /// The uuids of the entities in a box, one a line.
    EntitiesInBox { dim: String, min: Point, max: Point } -> Option<String> = ids::service_world::ENTITIES_IN_BOX, "entitiesInBox";
    /// The first block a ray hits (`yog_core::BlockHit`'s line).
    RaycastBlock { dim: String, from: Point, to: Point } -> Option<String> = ids::service_world::RAYCAST_BLOCK, "raycastBlock";
    /// The first entity a ray hits: its uuid.
    RaycastEntity { dim: String, from: Point, to: Point, exclude: String } -> Option<String> = ids::service_world::RAYCAST_ENTITY, "raycastEntity";
}
