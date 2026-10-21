package dev.yog;

import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.ItemStack;

/** The host's item actions: the methods {@code yog_event::item} names. */
public final class YogItemActions {
    private YogItemActions() {}

    /** Spawn an item entity with the given velocity; returns its uuid or null. */
    @YogServiceHost
    public static String spawnItem(String dim, double x, double y, double z,
            double vx, double vy, double vz, String itemId, int count, String snbt) {
        try {
            ServerLevel w = NativeBridge.worldFor(dim);
            if (w == null) return null;
            ItemStack stack = NativeBridge.buildStack(itemId, count, snbt);
            if (stack == null || stack.isEmpty()) return null;
            ItemEntity ie = new ItemEntity(w, x, y, z, stack);
            ie.setDeltaMovement(vx, vy, vz);
            if (!w.addFreshEntity(ie)) return null;
            return ie.getStringUUID();
        } catch (Throwable t) {
            return null;
        }
    }

    /** Replace an item entity's stack; count <= 0 or empty id discards the entity. */
    @YogServiceHost
    public static boolean itemEntitySetStack(String uuid, String itemId, int count, String snbt) {
        try {
            if (!(NativeBridge.entityByUuid(uuid) instanceof ItemEntity ie)) return false;
            if (count <= 0 || itemId == null || itemId.isEmpty()) {
                ie.discard();
                return true;
            }
            ItemStack stack = NativeBridge.buildStack(itemId, count, snbt);
            if (stack == null) return false;
            ie.setItem(stack);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    /** The block id a block item places, or null. */
    @YogServiceHost
    public static String itemBlock(String itemId) {
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return null;
        if (!(BuiltInRegistries.ITEM.getValue(id) instanceof BlockItem bi)) return null;
        return BuiltInRegistries.BLOCK.getKey(bi.getBlock()).toString();
    }
}
