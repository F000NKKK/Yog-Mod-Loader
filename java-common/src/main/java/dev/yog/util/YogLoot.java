package dev.yog.util;

import dev.yog.YogLog;

import dev.yog.data.YogData;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;

/**
 * Loot (ABI 3.4): what a rolled loot table gives, asked of the mods before
 * it's handed out — {@code yog_registry::Loot} in Yog Data. The hosts'
 * mixins gather the table's context and stacks; this encodes them, calls
 * the runtime and reads back the stacks to give instead.
 */
public final class YogLoot {
    private YogLoot() {}

    /** A stack: item id, count, custom data SNBT ({@code ""} for none). */
    public record Stack(String id, int count, String nbt) {}

    /** What a loot table was rolled for. Null fields aren't known. */
    public record Context(String table, String dimension, double[] pos, String entity, String killer,
                          Stack tool, Map<String, Integer> enchantments) {}

    /**
     * The stacks to give instead of {@code items}, or null to give them.
     *
     * @param nativeLoot {@code NativeBridge::nativeLoot}
     */
    public static List<Stack> run(Function<byte[], byte[]> nativeLoot, Context ctx, List<Stack> items) {
        Map<String, Object> loot = new LinkedHashMap<>();
        loot.put("table", ctx.table());
        if (ctx.dimension() != null) loot.put("dimension", ctx.dimension());
        if (ctx.pos() != null) loot.put("pos", List.of(ctx.pos()[0], ctx.pos()[1], ctx.pos()[2]));
        if (ctx.entity() != null) loot.put("entity", ctx.entity());
        if (ctx.killer() != null) loot.put("killer", ctx.killer());
        if (ctx.tool() != null) loot.put("tool", stack(ctx.tool()));
        if (ctx.enchantments() != null && !ctx.enchantments().isEmpty()) {
            List<Object> e = new ArrayList<>();
            ctx.enchantments().forEach((id, level) -> e.add(List.of(id, (long) level)));
            loot.put("enchantments", e);
        }
        List<Object> list = new ArrayList<>();
        for (Stack s : items) list.add(stack(s));
        loot.put("items", list);
        byte[] out;
        try {
            out = nativeLoot.apply(YogData.encode(loot));
        } catch (Throwable t) {
            YogLog.warn("loot " + ctx.table() + ": " + t);
            return null;
        }
        if (out == null) return null;
        List<Stack> given = new ArrayList<>();
        for (Object o : YogData.list(YogData.decode(out))) {
            Map<String, Object> m = YogData.map(o);
            String id = YogData.str(m.get("id"), null);
            int count = (int) YogData.integer(m.get("count"), 0);
            if (id != null && count > 0) given.add(new Stack(id, count, YogData.str(m.get("nbt"), "")));
        }
        return given;
    }

    private static Map<String, Object> stack(Stack s) {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("id", s.id());
        m.put("count", (long) s.count());
        if (s.nbt() != null && !s.nbt().isEmpty() && !s.nbt().equals("{}")) m.put("nbt", s.nbt());
        return m;
    }
}
