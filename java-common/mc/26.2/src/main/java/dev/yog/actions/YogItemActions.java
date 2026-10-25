package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogServiceHost;
import dev.yog.bus.YogTypes;

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

    /** What eating {@code itemId} gives ({@code item::Food}); null when it isn't food. */
    @YogServiceHost
    public static YogTypes.Food itemFood(String itemId) {
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return null;
        var components = BuiltInRegistries.ITEM.getValue(id).components();
        var f = components.get(net.minecraft.core.component.DataComponents.FOOD);
        if (f == null) return null;
        var consumed = components.get(net.minecraft.core.component.DataComponents.CONSUMABLE);
        java.util.List<YogTypes.FoodEffect> effects = new java.util.ArrayList<>();
        boolean fast = false;
        if (consumed != null) {
            fast = consumed.consumeSeconds() < net.minecraft.world.item.component.Consumable.DEFAULT_CONSUME_SECONDS;
            for (var c : consumed.onConsumeEffects()) {
                if (!(c instanceof net.minecraft.world.item.consume_effects.ApplyStatusEffectsConsumeEffect apply)) continue;
                for (var e : apply.effects()) {
                    net.minecraft.world.effect.MobEffect m = e.getEffect().value();
                    effects.add(new YogTypes.FoodEffect(BuiltInRegistries.MOB_EFFECT.getKey(m).toString(), apply.probability(), m.isBeneficial()));
                }
            }
        }
        return new YogTypes.Food(f.nutrition(), f.saturation(), f.canAlwaysEat(), fast, effects);
    }
}
