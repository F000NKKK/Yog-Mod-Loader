package dev.yog;

import dev.yog.util.YogLoot;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;
import net.minecraft.entity.Entity;
import net.minecraft.item.ItemStack;
import net.minecraft.loot.LootTable;
import net.minecraft.loot.context.LootContext;
import net.minecraft.loot.context.LootContextParameters;
import net.minecraft.util.math.Vec3d;

/**
 * Loot (ABI 3.4): a loot table rolled at the top (a chest's, a block's
 * drops, a mob's) — what it gives goes past the mods first
 * ({@link YogLoot}). Called by {@code LootTableMixin}.
 */
public final class YogLootHook {
    private YogLootHook() {}

    /** Set while the table rolls again for the mods: that roll is the game's. */
    private static final ThreadLocal<Boolean> ROLLING = ThreadLocal.withInitial(() -> false);

    /**
     * Rolls {@code table} ({@code id}) for the mods and gives {@code out}
     * what they leave; false when there's nothing to ask (the game rolls as usual).
     */
    public static boolean roll(LootTable table, String id, LootContext ctx, Consumer<ItemStack> out) {
        if (id == null || ROLLING.get() || !NativeBridge.nativeHasLoot()) return false;
        List<ItemStack> rolled = new ArrayList<>();
        ROLLING.set(true);
        try {
            table.generateLoot(ctx, rolled::add);
        } finally {
            ROLLING.set(false);
        }
        List<YogLoot.Stack> items = new ArrayList<>();
        for (ItemStack s : rolled) items.add(stack(s));
        Vec3d origin = ctx.get(LootContextParameters.ORIGIN);
        Entity entity = ctx.get(LootContextParameters.THIS_ENTITY);
        Entity killer = ctx.get(LootContextParameters.KILLER_ENTITY);
        ItemStack tool = ctx.get(LootContextParameters.TOOL);
        YogLoot.Context c = new YogLoot.Context(
                id,
                ctx.getWorld().getRegistryKey().getValue().toString(),
                origin == null ? null : new double[] {origin.x, origin.y, origin.z},
                entity == null ? null : entity.getUuid().toString(),
                killer == null ? null : killer.getUuid().toString(),
                tool == null || tool.isEmpty() ? null : stack(tool),
                tool == null ? null : enchantments(tool));
        List<YogLoot.Stack> given = YogLoot.run(NativeBridge::nativeLoot, c, items);
        if (given == null) {
            rolled.forEach(out);
            return true;
        }
        for (YogLoot.Stack s : given) {
            ItemStack stack = NativeBridge.buildStack(s.id(), s.count(), s.nbt());
            if (stack == null) continue;
            // As the game gives them: no stack above its size.
            int max = Math.max(1, stack.getMaxCount());
            for (int left = stack.getCount(); left > 0; left -= max) out.accept(stack.copyWithCount(Math.min(left, max)));
        }
        return true;
    }

    private static YogLoot.Stack stack(ItemStack s) {
        return new YogLoot.Stack(net.minecraft.registry.Registries.ITEM.getId(s.getItem()).toString(), s.getCount(), NativeBridge.stackSnbt(s));
    }

    private static Map<String, Integer> enchantments(ItemStack tool) {
        Map<String, Integer> out = new LinkedHashMap<>();
        for (var e : tool.getEnchantments().getEnchantmentEntries()) out.put(e.getKey().getIdAsString(), e.getIntValue());
        return out;
    }
}
