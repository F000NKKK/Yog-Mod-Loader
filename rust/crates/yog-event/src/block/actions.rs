//! Blocks: the host's actions.

use yog_core::{BlockInfo, BlockPos, Stack};
use yog_ids as ids;

actions! {
    /// A block entity's data as SNBT.
    Nbt { dim: String, pos: BlockPos } -> Option<String> = ids::service_block::NBT, "YogBlockActions.getBlockNbt";
    /// Merges `snbt` into a block entity's data.
    SetNbt { dim: String, pos: BlockPos, snbt: String } -> bool = ids::service_block::SET_NBT, "YogBlockActions.setBlockNbt";
    /// The block state at `pos` (`minecraft:oak_log[axis=y]`).
    State { dim: String, pos: BlockPos } -> Option<String> = ids::service_block::STATE, "YogBlockActions.getBlockState";
    /// Sets the block state at `pos`.
    SetState { dim: String, pos: BlockPos, state: String } -> bool = ids::service_block::SET_STATE, "YogBlockActions.setBlockState";
    /// The block's state.
    Info { dim: String, pos: BlockPos } -> Option<BlockInfo> = ids::service_block::INFO, "YogQueries.blockInfo";
    /// Breaks the block (dropping it or not).
    Destroy { dim: String, pos: BlockPos, drop: bool, breaker: String } -> bool = ids::service_block::DESTROY, "YogBlockActions.breakBlock";
    /// Applies bone meal to the block.
    Bonemeal { dim: String, pos: BlockPos } -> bool = ids::service_block::BONEMEAL, "YogBlockActions.bonemeal";
    /// Uses `item` on a face of the block, as `player` would.
    UseItem { dim: String, pos: BlockPos, face: u8, item: String, player: String } -> bool = ids::service_block::USE_ITEM, "YogBlockActions.useItemOnBlock";
    /// Empties a bucket at `pos`.
    PourFluid { dim: String, pos: BlockPos, bucket: String } -> bool = ids::service_block::POUR_FLUID, "YogBlockActions.pourFluid";
    /// The redstone power reaching the block; `None` without the dimension.
    RedstonePower { dim: String, pos: BlockPos } -> Option<u8> = ids::service_block::REDSTONE_POWER, "YogQueries.redstonePower";
    /// The strongest signal the block gives out; `None` if it gives none.
    RedstoneSignal { dim: String, pos: BlockPos } -> Option<u8> = ids::service_block::REDSTONE_SIGNAL, "YogQueries.redstoneSignal";
    /// Sets a mod block's redstone output.
    SetRedstoneOutput { dim: String, pos: BlockPos, level: i32 } -> bool = ids::service_block::SET_REDSTONE_OUTPUT, "YogBlockActions.setRedstoneOutput";
    /// A container's slot.
    InventorySlot { dim: String, pos: BlockPos, slot: u32 } -> Option<Stack> = ids::service_block::INVENTORY_SLOT, "YogQueries.getInventorySlot";
    /// Sets a container's slot.
    SetInventorySlot { dim: String, pos: BlockPos, slot: u32, item: String, count: u32, snbt: String } -> bool = ids::service_block::SET_INVENTORY_SLOT, "YogBlockActions.setInventorySlot";
}
