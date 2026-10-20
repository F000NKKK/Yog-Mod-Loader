//! The host's actions on a world.

use yog_core::BlockPos;
use yog_ids as ids;

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
}
