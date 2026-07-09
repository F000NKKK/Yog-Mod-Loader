package dev.yog;

import net.minecraft.core.BlockPos;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.storage.loot.LootParams;
import net.minecraft.world.level.storage.loot.parameters.LootContextParams;
import net.minecraft.world.phys.BlockHitResult;

import java.util.List;

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
    public InteractionResult use(BlockState state, Level level, BlockPos pos, Player player, InteractionHand hand, BlockHitResult hit) {
        if (level.isClientSide) return InteractionResult.SUCCESS;
        if (level.getBlockEntity(pos) instanceof YogInventoryBlockEntity be) {
            player.openMenu(be);
        }
        return InteractionResult.CONSUME;
    }

    // ── Break-preserves-contents (phase 6, shulker-box pattern) ──────────────

    @Override
    public void playerWillDestroy(Level level, BlockPos pos, BlockState state, Player player) {
        BlockEntity be = level.getBlockEntity(pos);
        if (be instanceof YogInventoryBlockEntity inv && !level.isClientSide) {
            if (player.isCreative() && !inv.isEmpty()) {
                ItemStack stack = new ItemStack(this);
                inv.saveToItemStack(stack);
                ItemEntity ie = new ItemEntity(level,
                        pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5, stack);
                ie.setDefaultPickUpDelay();
                level.addFreshEntity(ie);
            }
        }
        super.playerWillDestroy(level, pos, state, player);
    }

    @Override
    public List<ItemStack> getDrops(BlockState state, LootParams.Builder builder) {
        List<ItemStack> drops = super.getDrops(state, builder);
        BlockEntity be = builder.getOptionalParameter(LootContextParams.BLOCK_ENTITY);
        if (be instanceof YogInventoryBlockEntity inv && !inv.isEmpty()) {
            for (ItemStack stack : drops) {
                inv.saveToItemStack(stack);
            }
        }
        return drops;
    }

    @Override
    public ItemStack getCloneItemStack(BlockGetter level, BlockPos pos, BlockState state) {
        ItemStack stack = super.getCloneItemStack(level, pos, state);
        if (level instanceof Level lvl) {
            BlockEntity be = lvl.getBlockEntity(pos);
            if (be instanceof YogInventoryBlockEntity inv) {
                inv.saveToItemStack(stack);
            }
        }
        return stack;
    }
}
