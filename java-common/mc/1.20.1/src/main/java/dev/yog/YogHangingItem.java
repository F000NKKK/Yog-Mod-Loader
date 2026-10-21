package dev.yog;

import dev.yog.data.YogContent;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.UseOnContext;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.gameevent.GameEvent;

/**
 * An item that hangs on a block's side ({@code item.hangs}): one of it goes
 * into a new hanging entity ({@link YogHanging}) there, as a painting's item
 * places a painting.
 */
public class YogHangingItem extends YogItem {
    private final YogContent.Hangs hangs;

    public YogHangingItem(Properties properties, String displayName, String tooltip, YogContent.Hangs hangs) {
        super(properties, displayName, tooltip);
        this.hangs = hangs;
    }

    @SuppressWarnings("unchecked") // checked to be a hanging entity type before the cast
    private EntityType<? extends YogHanging> type() {
        ResourceLocation id = ResourceLocation.tryParse(hangs.entity());
        EntityType<?> type = id == null ? null : BuiltInRegistries.ENTITY_TYPE.getOptional(id).orElse(null);
        YogContent.Entity def = type == null ? null : YogEntityTypes.def(type);
        return def != null && def.hanging() ? (EntityType<? extends YogHanging>) type : null;
    }

    @Override
    public InteractionResult useOn(UseOnContext ctx) {
        Direction side = ctx.getClickedFace();
        BlockPos at = ctx.getClickedPos().relative(side);
        Player player = ctx.getPlayer();
        ItemStack stack = ctx.getItemInHand();
        EntityType<? extends YogHanging> type = type();
        if (type == null) return InteractionResult.PASS;
        if (player != null && !mayPlace(player, side, stack, at)) return InteractionResult.FAIL;
        Level level = ctx.getLevel();
        YogHanging hanging = new YogHanging(type, level, at).hang(side, hangs, stack.copyWithCount(1));
        CompoundTag tag = stack.getTag();
        if (tag != null) EntityType.updateCustomEntityTag(level, player, hanging, tag);
        if (!hanging.survives()) return InteractionResult.CONSUME;
        if (!level.isClientSide) {
            hanging.playPlacementSound();
            level.gameEvent(player, GameEvent.ENTITY_PLACE, hanging.position());
            hanging.share();
            level.addFreshEntity(hanging);
        }
        stack.shrink(1);
        return InteractionResult.sidedSuccess(level.isClientSide);
    }

    /** Only on a wall, where the player may build. */
    protected boolean mayPlace(Player player, Direction side, ItemStack stack, BlockPos pos) {
        return !side.getAxis().isVertical() && player.mayUseItemAt(pos, side, stack);
    }
}
