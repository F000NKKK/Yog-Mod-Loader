//! Blocks: the host's actions.

use crate::pos::Pos;
use yog_core::BlockPos;
use yog_ids as ids;

actions! {
    /// A block entity's data as SNBT.
    Nbt { dim: String, #[data(with = "Pos")] pos: BlockPos } -> Option<String> = ids::service_block::NBT, "getBlockNbt";
    /// Merges `snbt` into a block entity's data.
    SetNbt { dim: String, #[data(with = "Pos")] pos: BlockPos, snbt: String } -> bool = ids::service_block::SET_NBT, "setBlockNbt";
    /// The block state at `pos` (`minecraft:oak_log[axis=y]`).
    State { dim: String, #[data(with = "Pos")] pos: BlockPos } -> Option<String> = ids::service_block::STATE, "getBlockState";
    /// Sets the block state at `pos`.
    SetState { dim: String, #[data(with = "Pos")] pos: BlockPos, state: String } -> bool = ids::service_block::SET_STATE, "setBlockState";
    /// The block's state (`yog_core::BlockInfo`'s line).
    Info { dim: String, #[data(with = "Pos")] pos: BlockPos } -> Option<String> = ids::service_block::INFO, "blockInfo";
    /// Breaks the block (dropping it or not).
    Destroy { dim: String, #[data(with = "Pos")] pos: BlockPos, drop: bool, breaker: String } -> bool = ids::service_block::DESTROY, "breakBlock";
    /// Applies bone meal to the block.
    Bonemeal { dim: String, #[data(with = "Pos")] pos: BlockPos } -> bool = ids::service_block::BONEMEAL, "bonemeal";
    /// Uses `item` on a face of the block, as `player` would.
    UseItem { dim: String, #[data(with = "Pos")] pos: BlockPos, face: u8, item: String, player: String } -> bool = ids::service_block::USE_ITEM, "useItemOnBlock";
    /// Empties a bucket at `pos`.
    PourFluid { dim: String, #[data(with = "Pos")] pos: BlockPos, bucket: String } -> bool = ids::service_block::POUR_FLUID, "pourFluid";
    /// The redstone power reaching the block.
    RedstonePower { dim: String, #[data(with = "Pos")] pos: BlockPos } -> i32 = ids::service_block::REDSTONE_POWER, "redstonePower";
    /// The strongest signal around the block.
    RedstoneSignal { dim: String, #[data(with = "Pos")] pos: BlockPos } -> i32 = ids::service_block::REDSTONE_SIGNAL, "redstoneSignal";
    /// Sets a mod block's redstone output.
    SetRedstoneOutput { dim: String, #[data(with = "Pos")] pos: BlockPos, level: i32 } -> bool = ids::service_block::SET_REDSTONE_OUTPUT, "setRedstoneOutput";
    /// A container's slot: `item\tcount\tsnbt`.
    InventorySlot { dim: String, #[data(with = "Pos")] pos: BlockPos, slot: u32 } -> Option<String> = ids::service_block::INVENTORY_SLOT, "getInventorySlot";
    /// Sets a container's slot.
    SetInventorySlot { dim: String, #[data(with = "Pos")] pos: BlockPos, slot: u32, item: String, count: u32 } -> bool = ids::service_block::SET_INVENTORY_SLOT, "setInventorySlot";
}
