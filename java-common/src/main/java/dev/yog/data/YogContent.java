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

    /**
     * What tools are made of, whole (the game's or a mod's): {@code repair} an item id or {@code #tag};
     * {@code levelIndex} 1.20's level number, {@code incorrect} 1.21's tag of what it can't mine.
     */
    public record ToolMaterial(String id, int durability, float speed, float attackDamage, int enchantability,
                               String repair, int levelIndex, String incorrect) {}

    /** A tool: {@code pickaxe}, {@code axe}, {@code shovel}, {@code hoe}, {@code sword}, or a mineable tag id. */
    public record Tool(String kind, float attackDamage, float attackSpeed, ToolMaterial material) {}

    /** What armor is made of, whole; {@code durability} multiplies each piece's base. */
    public record ArmorMaterial(String id, int durability, int head, int chest, int legs, int feet, int enchantability,
                                String equipSound, float toughness, float knockbackResistance, String repair,
                                String texture) {}

    /** A piece of armor: {@code head}, {@code chest}, {@code legs} or {@code feet}. */
    public record Armor(String slot, ArmorMaterial material) {}

    /**
     * An item; {@code name}/{@code tooltip} are empty when not set, {@code food}, {@code tool} and
     * {@code armor} null; {@code book}: opens the book of its id.
     */
    public record Item(String id, int maxStack, String name, String tooltip, int maxDamage,
                       boolean fireResistant, int fuelTicks, Food food, boolean book, Tool tool, Armor armor) {}

    /**
     * A sound group, whole — the game's or a mod's: sound event ids the host
     * looks up (or makes) by id, so it needs no list of groups of its own.
     */
    public record Sound(String breakSound, String step, String place, String hit, String fall, float volume, float pitch) {}

    /** A block; {@code shape} ({@code x1, y1, z1, x2, y2, z2} in pixels), {@code sound} and {@code inventory} are null when not set. */
    public record Block(String id, float hardness, float resistance, String name, float[] shape, int light,
                        Sound sound, boolean requiresTool, boolean noCollision, float slipperiness,
                        boolean connects, List<String> connectGroups, String inventory, boolean redstone) {}

    /** A recipe in Yog's own format, as a JSON tree (what {@code YogDataLayout.yogRecipeToVanilla} reads). */
    public record Recipe(String namespace, String name, com.google.gson.JsonObject data) {}

    /** A placed feature (an ore's) added to the biomes of a tag at a generation step ({@code underground_ores}). */
    public record BiomeFeature(String feature, String biomes, String step) {}

    /** A dimension: its id and {@code YogDimensionDef} as a JSON tree. */
    public record Dimension(String id, com.google.gson.JsonObject def) {}

    public final List<Item> items;
    public final List<Block> blocks;
    /** By id. */
    public final Map<String, YogInventoryDef> inventories;
    public final List<Recipe> recipes;
    public final List<Dimension> dimensions;
    /** For the hosts that add features to biomes in code (Fabric). */
    public final List<BiomeFeature> biomeFeatures;

    private YogContent(List<Item> items, List<Block> blocks, Map<String, YogInventoryDef> inventories,
                       List<Recipe> recipes, List<Dimension> dimensions, List<BiomeFeature> biomeFeatures) {
        this.items = items;
        this.blocks = blocks;
        this.inventories = inventories;
        this.recipes = recipes;
        this.dimensions = dimensions;
        this.biomeFeatures = biomeFeatures;
    }

    private static YogContent cached;

    /**
     * The runtime's content, read once: mods register it all while the
     * runtime loads, before any host phase asks — every phase after the
     * first gets the same records.
     */
    public static synchronized YogContent cached(java.util.function.Supplier<byte[]> blob) {
        if (cached == null) cached = read(blob.get());
        return cached;
    }

    /** Reads the runtime's blob; {@code null} (no runtime) is no content. */
    public static YogContent read(byte[] blob) {
        if (blob == null || blob.length == 0) {
            return new YogContent(Collections.emptyList(), Collections.emptyList(), Collections.emptyMap(),
                    Collections.emptyList(), Collections.emptyList(), Collections.emptyList());
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
                    (int) YogData.integer(m.get("fuel_ticks"), 0), food, YogData.bool(m.get("book"), false),
                    tool(m.get("tool")), armor(m.get("armor"))));
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
                    (int) YogData.integer(m.get("light"), 0), sound(m.get("sound")),
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
            recipes.add(new Recipe(YogData.str(m.get("namespace"), ""), YogData.str(m.get("name"), ""), object(m.get("recipe"))));
        }
        List<Dimension> dimensions = new ArrayList<>();
        for (Object o : YogData.list(root.get("dimensions"))) {
            Map<String, Object> m = YogData.map(o);
            dimensions.add(new Dimension(YogData.str(m.get("id"), ""), object(m.get("def"))));
        }
        List<BiomeFeature> biomeFeatures = new ArrayList<>();
        for (Object o : YogData.list(root.get("biome_features"))) {
            Map<String, Object> m = YogData.map(o);
            biomeFeatures.add(new BiomeFeature(YogData.str(m.get("feature"), ""), YogData.str(m.get("biomes"), "minecraft:is_overworld"),
                    YogData.str(m.get("step"), "underground_ores")));
        }
        return new YogContent(items, blocks, inventories, recipes, dimensions, biomeFeatures);
    }

    private static Tool tool(Object v) {
        if (v == null) return null;
        Map<String, Object> t = YogData.map(v);
        Map<String, Object> m = YogData.map(t.get("material"));
        ToolMaterial material = new ToolMaterial(YogData.str(m.get("id"), ""), (int) YogData.integer(m.get("durability"), 1),
                (float) YogData.number(m.get("speed"), 1), (float) YogData.number(m.get("attack_damage"), 0),
                (int) YogData.integer(m.get("enchantability"), 0), YogData.str(m.get("repair"), ""),
                (int) YogData.integer(m.get("level_index"), 0), YogData.str(m.get("incorrect"), "minecraft:incorrect_for_wooden_tool"));
        return new Tool(YogData.str(t.get("kind"), "pickaxe"), (float) YogData.number(t.get("attack_damage"), 1),
                (float) YogData.number(t.get("attack_speed"), -2.8), material);
    }

    private static Armor armor(Object v) {
        if (v == null) return null;
        Map<String, Object> a = YogData.map(v);
        Map<String, Object> m = YogData.map(a.get("material"));
        Map<String, Object> d = YogData.map(m.get("defense"));
        ArmorMaterial material = new ArmorMaterial(YogData.str(m.get("id"), ""), (int) YogData.integer(m.get("durability"), 1),
                (int) YogData.integer(d.get("head"), 0), (int) YogData.integer(d.get("chest"), 0),
                (int) YogData.integer(d.get("legs"), 0), (int) YogData.integer(d.get("feet"), 0),
                (int) YogData.integer(m.get("enchantability"), 0), YogData.str(m.get("equip_sound"), "minecraft:item.armor.equip_generic"),
                (float) YogData.number(m.get("toughness"), 0), (float) YogData.number(m.get("knockback_resistance"), 0),
                YogData.str(m.get("repair"), ""), YogData.str(m.get("texture"), ""));
        return new Armor(YogData.str(a.get("slot"), "head"), material);
    }

    /** Base durability of an armor piece (helmet 11, chestplate 16, leggings 15, boots 13), times the material's. */
    public static int armorDurability(Armor a) {
        int base = switch (a.slot()) {
            case "chest" -> 16;
            case "legs" -> 15;
            case "feet" -> 13;
            default -> 11;
        };
        return base * a.material().durability();
    }

    /** A piece's defense points. */
    public static int armorDefense(Armor a) {
        ArmorMaterial m = a.material();
        return switch (a.slot()) {
            case "chest" -> m.chest();
            case "legs" -> m.legs();
            case "feet" -> m.feet();
            default -> m.head();
        };
    }

    private static Sound sound(Object v) {
        if (v == null) return null;
        Map<String, Object> m = YogData.map(v);
        return new Sound(YogData.str(m.get("break"), ""), YogData.str(m.get("step"), ""), YogData.str(m.get("place"), ""),
                YogData.str(m.get("hit"), ""), YogData.str(m.get("fall"), ""),
                (float) YogData.number(m.get("volume"), 1), (float) YogData.number(m.get("pitch"), 1));
    }

    private static com.google.gson.JsonObject object(Object v) {
        com.google.gson.JsonElement e = YogData.toGson(v);
        return e.isJsonObject() ? e.getAsJsonObject() : new com.google.gson.JsonObject();
    }

    /** A list of numbers as floats. */
    public static float[] floats(Object v) {
        List<Object> l = YogData.list(v);
        float[] out = new float[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = (float) YogData.number(l.get(i), 0);
        return out;
    }
}
