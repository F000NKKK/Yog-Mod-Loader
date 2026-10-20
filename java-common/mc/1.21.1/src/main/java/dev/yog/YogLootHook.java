package dev.yog;

import dev.yog.bus.YogEvents;
import dev.yog.bus.YogTypes;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.storage.loot.LootContext;
import net.minecraft.world.level.storage.loot.LootTable;
import net.minecraft.world.level.storage.loot.parameters.LootContextParams;
import net.minecraft.world.phys.Vec3;

/**
 * Loot (ABI 3.4): a loot table rolled at the top (a chest's, a block's
 * drops, a mob's) — what it gives goes past the mods first
 * ({@link YogEvents.World.Loot.Roll}). Called by {@code LootTableMixin}.
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
        if (id == null || ROLLING.get() || !YogEvents.World.Loot.Roll.heard()) return false;
        List<ItemStack> rolled = new ArrayList<>();
        ROLLING.set(true);
        try {
            table.getRandomItems(ctx, rolled::add);
        } finally {
            ROLLING.set(false);
        }
        List<YogTypes.LootStack> items = new ArrayList<>();
        for (ItemStack s : rolled) items.add(stack(s));
        Vec3 origin = ctx.getParamOrNull(LootContextParams.ORIGIN);
        Entity entity = ctx.getParamOrNull(LootContextParams.THIS_ENTITY);
        Entity killer = ctx.getParamOrNull(LootContextParams.ATTACKING_ENTITY);
        ItemStack tool = ctx.getParamOrNull(LootContextParams.TOOL);
        YogEvents.World.Loot.Roll roll = new YogEvents.World.Loot.Roll(
                id,
                ctx.getLevel().dimension().location().toString(),
                origin == null ? null : new double[] {origin.x, origin.y, origin.z},
                entity == null ? null : entity.getUUID().toString(),
                killer == null ? null : killer.getUUID().toString(),
                tool == null || tool.isEmpty() ? null : stack(tool),
                tool == null ? List.of() : enchantments(tool),
                items);
        List<YogTypes.LootStack> given = roll.emitChanged().items();
        if (given.equals(items)) {
            rolled.forEach(out);
            return true;
        }
        for (YogTypes.LootStack s : given) {
            ItemStack stack = NativeBridge.buildStack(s.id(), (int) s.count(), s.nbt());
            if (stack == null) continue;
            // As the game gives them: no stack above its size.
            int max = Math.max(1, stack.getMaxStackSize());
            for (int left = stack.getCount(); left > 0; left -= max) out.accept(stack.copyWithCount(Math.min(left, max)));
        }
        return true;
    }

    private static YogTypes.LootStack stack(ItemStack s) {
        return new YogTypes.LootStack(net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(s.getItem()).toString(), s.getCount(), NativeBridge.stackSnbt(s));
    }

    private static List<List<Object>> enchantments(ItemStack tool) {
        List<List<Object>> out = new ArrayList<>();
        for (var e : tool.getEnchantments().entrySet()) out.add(List.of(e.getKey().getRegisteredName(), (long) e.getIntValue()));
        return out;
    }
}
