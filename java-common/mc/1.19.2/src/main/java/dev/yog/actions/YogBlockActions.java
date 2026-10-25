package dev.yog.actions;

import net.minecraft.core.Registry;
import dev.yog.NativeBridge;
import dev.yog.YogLog;
import dev.yog.YogServiceHost;

import net.minecraft.world.level.block.LevelEvent;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.world.item.ItemStack;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.Direction;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.BoneMealItem;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.Vec3;

/** The host's block actions: the methods {@code yog_event::block} names. */
public final class YogBlockActions {
    private YogBlockActions() {}

    @YogServiceHost
    public static String getBlockNbt(String dimension, int x, int y, int z) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w == null) return null;
        BlockEntity be = w.getBlockEntity(new BlockPos(x, y, z));
        if (be == null) return null;
        CompoundTag nbt = be.saveWithFullMetadata();
        return nbt.toString();
    }

    @YogServiceHost
    public static boolean setBlockNbt(String dimension, int x, int y, int z, String snbt) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w == null) return false;
        BlockEntity be = w.getBlockEntity(new BlockPos(x, y, z));
        if (be == null) return false;
        try { CompoundTag nbt = TagParser.parseTag(snbt); be.load(nbt); be.setChanged(); return true; }
        catch (Exception e) { return false; }
    }

    /**
     * Sets a container slot to {@code count} of {@code item} with data
     * {@code snbt} (empty: none); count 0 empties the slot.
     */
    @YogServiceHost
    public static boolean setInventorySlot(String dimension, int x, int y, int z, int slot, String item, int count, String snbt) {
        net.minecraft.world.Container inv = NativeBridge.containerAt(dimension, x, y, z);
        if (inv == null || slot < 0 || slot >= inv.getContainerSize()) return false;
        if (count <= 0) {
            inv.setItem(slot, ItemStack.EMPTY);
            inv.setChanged();
            return true;
        }
        ResourceLocation id = ResourceLocation.tryParse(item);
        if (id == null || !Registry.ITEM.containsKey(id)) return false;
        var stack = new ItemStack(Registry.ITEM.get(id), count);
        if (!snbt.isEmpty()) {
            try {
                stack.setTag(TagParser.parseTag(snbt));
            } catch (Exception e) {
                YogLog.warn("setInventorySlot: bad SNBT for " + item + ": " + e.getMessage());
                return false;
            }
        }
        inv.setItem(slot, stack);
        inv.setChanged();
        return true;
    }

    /** Break the block (dropping items if `drop`); false for air / unbreakable blocks. */
    @YogServiceHost
    public static boolean breakBlock(String dim, int x, int y, int z, boolean drop, String breakerUuid) {
        try {
            ServerLevel w = NativeBridge.worldFor(dim);
            if (w == null) return false;
            BlockPos pos = new BlockPos(x, y, z);
            BlockState st = w.getBlockState(pos);
            if (st.isAir() || st.getDestroySpeed(w, pos) < 0) return false;
            return w.destroyBlock(pos, drop, NativeBridge.optionalEntity(breakerUuid));
        } catch (Throwable t) {
            return false;
        }
    }

    /** Apply bone meal at the position (crops / saplings / grass, or water plants). */
    @YogServiceHost
    public static boolean bonemeal(String dim, int x, int y, int z) {
        try {
            ServerLevel w = NativeBridge.worldFor(dim);
            if (w == null) return false;
            BlockPos pos = new BlockPos(x, y, z);
            boolean ok = BoneMealItem.growCrop(new ItemStack(Items.BONE_MEAL), w, pos)
                    || BoneMealItem.growWaterPlant(new ItemStack(Items.BONE_MEAL), w, pos, null);
            if (ok) w.levelEvent(LevelEvent.PARTICLES_AND_SOUND_PLANT_GROWTH, pos, 0);
            return ok;
        } catch (Throwable t) {
            return false;
        }
    }

    /** Use a fresh single item on a block face, as the named player (or nobody). */
    @YogServiceHost
    public static boolean useItemOnBlock(String dim, int x, int y, int z, int face, String itemId, String player) {
        try {
            ServerLevel w = NativeBridge.worldFor(dim);
            ResourceLocation id = ResourceLocation.tryParse(itemId);
            if (w == null || id == null || !Registry.ITEM.containsKey(id) || face < 0 || face > 5) return false;
            ServerPlayer p = player.isEmpty() ? null : NativeBridge.playerByName(player);
            BlockPos pos = new BlockPos(x, y, z);
            ItemStack stack = new ItemStack(Registry.ITEM.get(id));
            BlockHitResult hit = new BlockHitResult(Vec3.atCenterOf(pos), Direction.from3DDataValue(face), pos, false);
            net.minecraft.world.item.context.UseOnContext ctx =
                    new net.minecraft.world.item.context.UseOnContext(w, p, InteractionHand.MAIN_HAND, stack, hit) {};
            return stack.getItem().useOn(ctx).consumesAction();
        } catch (Throwable t) {
            return false;
        }
    }

    /** Empty a bucket item's fluid at the position, the way a player would. */
    @YogServiceHost
    public static boolean pourFluid(String dim, int x, int y, int z, String bucketId) {
        try {
            ServerLevel w = NativeBridge.worldFor(dim);
            ResourceLocation id = ResourceLocation.tryParse(bucketId);
            if (w == null || id == null || !Registry.ITEM.containsKey(id)) return false;
            if (!(Registry.ITEM.get(id) instanceof net.minecraft.world.item.BucketItem bucket)) return false;
            BlockPos pos = new BlockPos(x, y, z);
            BlockHitResult hit = new BlockHitResult(Vec3.atCenterOf(pos), Direction.UP, pos, false);
            return bucket.emptyContents(null, w, pos, hit);
        } catch (Throwable t) {
            return false;
        }
    }

    /** Sets the power (0–15) a redstone-source Yog block at a position gives off. */
    @YogServiceHost
    public static boolean setRedstoneOutput(String dimension, int x, int y, int z, int level) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w == null) return false;
        String key = NativeBridge.redstoneKey(dimension, x, y, z);
        int clamped = Math.max(0, Math.min(15, level));
        // Zeros are kept too: the first level set always updates the
        // neighbours — a lamp saved lit goes out when its source starts at 0.
        Integer before = NativeBridge.REDSTONE_OUTPUT.put(key, clamped);
        if (before == null || before != clamped) {
            net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(x, y, z);
            w.updateNeighborsAt(pos, w.getBlockState(pos).getBlock());
        }
        return true;
    }

    /** The full state of the block at a position: "minecraft:repeater[delay=2,facing=east,...]". */
    @YogServiceHost
    public static String getBlockState(String dimension, int x, int y, int z) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w == null) return null;
        net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(x, y, z);
        return net.minecraft.commands.arguments.blocks.BlockStateParser.serialize(w.getBlockState(pos));
    }

    /** Places a block in a full state ("minecraft:repeater[delay=2]"); false if it doesn't parse. */
    @YogServiceHost
    public static boolean setBlockState(String dimension, int x, int y, int z, String state) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w == null) return false;
        try {
            var parsed = net.minecraft.commands.arguments.blocks.BlockStateParser.parseForBlock(Registry.BLOCK, state, false).blockState();
            net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(x, y, z);
            return w.setBlock(pos, parsed, 3);
        } catch (Exception e) {
            YogLog.warn("setBlockState: " + state + ": " + e.getMessage());
            return false;
        }
    }
}
