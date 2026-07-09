package dev.yog;

import net.minecraft.block.Block;
import net.minecraft.block.BlockEntityProvider;
import net.minecraft.block.BlockState;
import net.minecraft.block.entity.BlockEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.server.world.ServerWorld;
import net.minecraft.util.ActionResult;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.math.BlockPos;
import net.minecraft.world.World;

/**
 * A block backed by a real Container/Menu inventory (see
 * rust/crates/yog-inventory/DESIGN.md), as opposed to plain `Block` or
 * `YogShapedBlock`/`YogConnectingBlock`. Right-click opens
 * {@link YogInventoryMenu} via vanilla's own menu-sync machinery.
 */
public class YogInventoryBlock extends Block implements BlockEntityProvider {
    private final String defId;
    private final int slotCount;

    public YogInventoryBlock(Settings settings, String defId, int slotCount) {
        super(settings);
        this.defId = defId;
        this.slotCount = slotCount;
    }

    public String defId() { return defId; }
    public int slotCount() { return slotCount; }

    @Override
    public BlockEntity createBlockEntity(BlockPos pos, BlockState state) {
        return new YogInventoryBlockEntity(pos, state, defId, slotCount);
    }

    @Override
    protected ActionResult onUse(BlockState state, World world, BlockPos pos, PlayerEntity player, BlockHitResult hit) {
        if (world.isClient) return ActionResult.SUCCESS;
        if (world.getBlockEntity(pos) instanceof YogInventoryBlockEntity be) {
            player.openHandledScreen(be);
        }
        return ActionResult.CONSUME;
    }
}
