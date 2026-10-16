package dev.yog;

import dev.yog.data.YogContent;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.item.ItemUsageContext;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.registry.Registries;
import net.minecraft.util.ActionResult;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.world.World;
import net.minecraft.world.event.GameEvent;

/**
 * An item that hangs on a block's side ({@code item.hangs}): one of it goes
 * into a new hanging entity ({@link YogHanging}) there, as a painting's item
 * places a painting.
 */
public class YogHangingItem extends YogItem {
    private final YogContent.Hangs hangs;

    public YogHangingItem(Settings settings, String displayName, String tooltip, YogContent.Hangs hangs) {
        super(settings, displayName, tooltip);
        this.hangs = hangs;
    }

    @SuppressWarnings("unchecked")
    private EntityType<? extends YogHanging> type() {
        Identifier id = Identifier.tryParse(hangs.entity());
        EntityType<?> type = id == null ? null : Registries.ENTITY_TYPE.get(id);
        YogContent.Entity def = type == null ? null : YogEntityTypes.def(type);
        return def != null && def.hanging() ? (EntityType<? extends YogHanging>) type : null;
    }

    @Override
    public ActionResult useOnBlock(ItemUsageContext ctx) {
        Direction side = ctx.getSide();
        BlockPos at = ctx.getBlockPos().offset(side);
        PlayerEntity player = ctx.getPlayer();
        ItemStack stack = ctx.getStack();
        EntityType<? extends YogHanging> type = type();
        if (type == null) return ActionResult.PASS;
        if (player != null && !canPlaceOn(player, side, stack, at)) return ActionResult.FAIL;
        World world = ctx.getWorld();
        YogHanging hanging = new YogHanging(type, world, at, side, hangs.width(), hangs.height(), stack.copyWithCount(1));
        NbtCompound tag = stack.getNbt();
        if (tag != null) EntityType.loadFromEntityNbt(world, player, hanging, tag);
        if (!hanging.canStayAttached()) return ActionResult.CONSUME;
        if (!world.isClient) {
            hanging.onPlace();
            world.emitGameEvent(player, GameEvent.ENTITY_PLACE, hanging.getPos());
            hanging.share();
            world.spawnEntity(hanging);
        }
        stack.decrement(1);
        return ActionResult.success(world.isClient);
    }

    /** Only on a wall, where the player may build. */
    protected boolean canPlaceOn(PlayerEntity player, Direction side, ItemStack stack, BlockPos pos) {
        return !side.getAxis().isVertical() && player.canPlaceOn(pos, side, stack);
    }
}
