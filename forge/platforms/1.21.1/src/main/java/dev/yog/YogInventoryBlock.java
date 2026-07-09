package dev.yog;

import net.minecraft.core.BlockPos;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;

/**
 * A block backed by a real Container/Menu inventory (see
 * rust/crates/yog-inventory/DESIGN.md), as opposed to plain `Block` or
 * `YogShapedBlock`/`YogConnectingBlock`. Right-click opens
 * {@link YogInventoryMenu} via vanilla's own menu-sync machinery.
 */
public class YogInventoryBlock extends Block implements EntityBlock {
    private final String defId;
    private final int slotCount;

    public YogInventoryBlock(Properties properties, String defId, int slotCount) {
        super(properties);
        this.defId = defId;
        this.slotCount = slotCount;
    }

    public String defId() { return defId; }
    public int slotCount() { return slotCount; }

    @Override
    public BlockEntity newBlockEntity(BlockPos pos, BlockState state) {
        return new YogInventoryBlockEntity(pos, state, defId, slotCount);
    }

    @Override
    protected InteractionResult useWithoutItem(BlockState state, Level level, BlockPos pos, Player player, BlockHitResult hit) {
        if (level.isClientSide) return InteractionResult.SUCCESS;
        if (level.getBlockEntity(pos) instanceof YogInventoryBlockEntity be) {
            player.openMenu(be);
        }
        return InteractionResult.CONSUME;
    }
}
