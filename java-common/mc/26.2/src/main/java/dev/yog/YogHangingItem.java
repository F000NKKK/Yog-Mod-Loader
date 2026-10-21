package dev.yog;

import dev.yog.bus.YogTypes;
import dev.yog.data.YogContent;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
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
    private final YogTypes.Hangs hangs;

    public YogHangingItem(Properties settings, String displayName, String tooltip, YogTypes.Hangs hangs) {
        super(settings, displayName, tooltip);
        this.hangs = hangs;
    }

    @SuppressWarnings("unchecked") // checked to be a hanging entity type before the cast
    private EntityType<? extends YogHanging> type() {
        Identifier id = Identifier.tryParse(hangs.entity());
        EntityType<?> type = id == null ? null : BuiltInRegistries.ENTITY_TYPE.getValue(id);
        YogTypes.EntityDef def = type == null ? null : YogEntityTypes.def(type);
        return def != null && YogContent.hanging(def) ? (EntityType<? extends YogHanging>) type : null;
    }

    @Override
    public InteractionResult useOn(UseOnContext ctx) {
        Direction side = ctx.getClickedFace();
        BlockPos at = ctx.getClickedPos().relative(side);
        Player player = ctx.getPlayer();
        ItemStack stack = ctx.getItemInHand();
        EntityType<? extends YogHanging> type = type();
        if (type == null) return InteractionResult.PASS;
        if (player != null && !canPlaceOn(player, side, stack, at)) return InteractionResult.FAIL;
        Level world = ctx.getLevel();
        YogHanging hanging = new YogHanging(type, world, at).hang(side, hangs, stack.copyWithCount(1));
        EntityType.<YogHanging>createDefaultStackConfig(world, stack, player).apply(hanging);
        if (!hanging.survives()) return InteractionResult.CONSUME;
        if (!world.isClientSide()) {
            hanging.playPlacementSound();
            world.gameEvent(player, GameEvent.ENTITY_PLACE, hanging.position());
            hanging.share();
            world.addFreshEntity(hanging);
        }
        stack.shrink(1);
        return InteractionResult.SUCCESS;
    }

    /** Only on a wall, where the player may build. */
    protected boolean canPlaceOn(Player player, Direction side, ItemStack stack, BlockPos pos) {
        return !side.getAxis().isVertical() && player.mayUseItemAt(pos, side, stack);
    }
}
