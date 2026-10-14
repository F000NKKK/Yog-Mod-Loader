package dev.yog.data;

import dev.yog.util.YogInventoryDef;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * What the native runtime has registered — items, blocks, inventories — as
 * the host reads it: one Yog Data blob ({@code NativeBridge.nativeContent()},
 * the runtime's {@code content_blob}) decoded into plain records. Pure
 * Java, shared by every host.
 */
public final class YogContent {
    public record Food(int nutrition, float saturation, boolean alwaysEdible) {}

    /** An item; {@code name}/{@code tooltip} are empty when not set, {@code food} null. */
    public record Item(String id, int maxStack, String name, String tooltip, int maxDamage,
                       boolean fireResistant, int fuelTicks, Food food) {}

    /** A block; {@code shape} ({@code x1, y1, z1, x2, y2, z2} in pixels), {@code sound} and {@code inventory} are null when not set. */
    public record Block(String id, float hardness, float resistance, String name, float[] shape, int light,
                        String sound, boolean requiresTool, boolean noCollision, float slipperiness,
                        boolean connects, List<String> connectGroups, String inventory, boolean redstone) {}

    /** A recipe in Yog's own format, as JSON (what {@code YogDataLayout.yogRecipeToVanilla} reads). */
    public record Recipe(String namespace, String name, String json) {}

    /** A dimension: its id and {@code YogDimensionDef} as JSON. */
    public record Dimension(String id, String json) {}

    public final List<Item> items;
    public final List<Block> blocks;
    /** By id. */
    public final Map<String, YogInventoryDef> inventories;
    public final List<Recipe> recipes;
    public final List<Dimension> dimensions;

    private YogContent(List<Item> items, List<Block> blocks, Map<String, YogInventoryDef> inventories,
                       List<Recipe> recipes, List<Dimension> dimensions) {
        this.items = items;
        this.blocks = blocks;
        this.inventories = inventories;
        this.recipes = recipes;
        this.dimensions = dimensions;
    }

    /** Reads the runtime's blob; {@code null} (no runtime) is no content. */
    public static YogContent read(byte[] blob) {
        if (blob == null || blob.length == 0) {
            return new YogContent(Collections.emptyList(), Collections.emptyList(), Collections.emptyMap(),
                    Collections.emptyList(), Collections.emptyList());
        }
        Map<String, Object> root = YogData.map(YogData.decode(blob));
        List<Item> items = new ArrayList<>();
        for (Object o : YogData.list(root.get("items"))) {
            Map<String, Object> m = YogData.map(o);
            Food food = null;
            if (m.get("food") != null) {
                Map<String, Object> f = YogData.map(m.get("food"));
                food = new Food((int) YogData.integer(f.get("nutrition"), 0), (float) YogData.number(f.get("saturation"), 0),
                        YogData.bool(f.get("always_edible"), false));
            }
            items.add(new Item(YogData.str(m.get("id"), ""), (int) YogData.integer(m.get("max_stack"), 64),
                    YogData.str(m.get("name"), ""), YogData.str(m.get("tooltip"), ""),
                    (int) YogData.integer(m.get("max_damage"), 0), YogData.bool(m.get("fire_resistant"), false),
                    (int) YogData.integer(m.get("fuel_ticks"), 0), food));
        }
        List<Block> blocks = new ArrayList<>();
        for (Object o : YogData.list(root.get("blocks"))) {
            Map<String, Object> m = YogData.map(o);
            float[] shape = null;
            if (m.get("shape") != null) shape = floats(m.get("shape"));
            List<String> groups = new ArrayList<>();
            for (Object g : YogData.list(m.get("connect_groups"))) groups.add(YogData.str(g, ""));
            blocks.add(new Block(YogData.str(m.get("id"), ""), (float) YogData.number(m.get("hardness"), 1.5),
                    (float) YogData.number(m.get("resistance"), 6.0), YogData.str(m.get("name"), ""), shape,
                    (int) YogData.integer(m.get("light"), 0), YogData.str(m.get("sound"), null),
                    YogData.bool(m.get("requires_tool"), false), YogData.bool(m.get("no_collision"), false),
                    (float) YogData.number(m.get("slipperiness"), 0), YogData.bool(m.get("connects"), false), groups,
                    YogData.str(m.get("inventory"), null), YogData.bool(m.get("redstone"), false)));
        }
        Map<String, YogInventoryDef> inventories = new HashMap<>();
        for (Object o : YogData.list(root.get("inventories"))) {
            YogInventoryDef def = YogInventoryDef.fromData(YogData.map(o));
            inventories.put(def.id, def);
        }
        List<Recipe> recipes = new ArrayList<>();
        for (Object o : YogData.list(root.get("recipes"))) {
            Map<String, Object> m = YogData.map(o);
            recipes.add(new Recipe(YogData.str(m.get("namespace"), ""), YogData.str(m.get("name"), ""), YogData.toJson(m.get("recipe"))));
        }
        List<Dimension> dimensions = new ArrayList<>();
        for (Object o : YogData.list(root.get("dimensions"))) {
            Map<String, Object> m = YogData.map(o);
            dimensions.add(new Dimension(YogData.str(m.get("id"), ""), YogData.toJson(m.get("def"))));
        }
        return new YogContent(items, blocks, inventories, recipes, dimensions);
    }

    /** A list of numbers as floats. */
    public static float[] floats(Object v) {
        List<Object> l = YogData.list(v);
        float[] out = new float[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = (float) YogData.number(l.get(i), 0);
        return out;
    }
}
