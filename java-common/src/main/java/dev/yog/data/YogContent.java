package dev.yog.data;

import dev.yog.util.YogInventoryDef;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
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
    /**
     * An item; {@code wearable} is the slot it's worn in without being armor ({@code head}, …), or null;
     * {@code hangs} what it hangs on a wall, or null.
     */
    public record Item(String id, int maxStack, String name, String tooltip, int maxDamage,
                       boolean fireResistant, int fuelTicks, Food food, boolean book, Tool tool, Armor armor,
                       String wearable, Hangs hangs) {}

    /** What an item hangs on a block's side: an entity of a hanging type, {@code width}×{@code height} blocks. */
    public record Hangs(String entity, int width, int height) {}

    /**
     * A sound group, whole — the game's or a mod's: sound event ids the host
     * looks up (or makes) by id, so it needs no list of groups of its own.
     */
    public record Sound(String breakSound, String step, String place, String hit, String fall, float volume, float pitch) {}

    /**
     * A block; {@code shape} ({@code x1, y1, z1, x2, y2, z2} in pixels), {@code sound},
     * {@code inventory}, {@code powered} and {@code comparator} are null when not set.
     * {@code facing}: {@code ""}, {@code "horizontal"} or {@code "all"}; {@code placement}:
     * {@code "toward"} (its front to the placer) or {@code "away"}.
     * {@code kind}: one of the game's kinds of block ({@code "stairs"}, {@code "door"}…, see
     * {@code YogBlockKinds}) or {@code ""}; {@code base} (the block it's made from) and {@code set}
     * (a block set or wood type by name) are null when not set.
     */
    public record Block(String id, float hardness, float resistance, String name, float[] shape, int light,
                        Sound sound, boolean requiresTool, boolean noCollision, float slipperiness,
                        boolean connects, List<String> connectGroups, String inventory, boolean redstone,
                        String facing, String placement, boolean attach, boolean waterlogged,
                        List<Property> properties, String powered, String comparator, boolean acceptsItems,
                        boolean noOcclusion, String kind, String base, String set, int pressTicks,
                        boolean mobsOnly) {
        /** Whether it's a {@code YogStateBlock}: states of its own, or it takes items. */
        public boolean stateful() {
            return !facing.isEmpty() || attach || waterlogged || !properties.isEmpty() || acceptsItems;
        }
    }

    /** A property of a block's own: its values, the first the default; {@code false, true} is a yes/no. */
    public record Property(String name, List<String> values) {
        public boolean yesNo() {
            return values.equals(List.of("false", "true"));
        }
    }

    /** A recipe in Yog's own format, as a JSON tree (what {@code YogDataLayout.yogRecipeToVanilla} reads). */
    public record Recipe(String namespace, String name, com.google.gson.JsonObject data) {}

    /** A placed feature (an ore's) added to the biomes of a tag at a generation step ({@code underground_ores}). */
    public record BiomeFeature(String feature, String biomes, String step) {}

    /** A dimension: its id and {@code YogDimensionDef} as a JSON tree. */
    public record Dimension(String id, com.google.gson.JsonObject def) {}

    /**
     * A mod's entity type ({@code yog_registry::EntityDef}). {@code kind}:
     * {@code object}, {@code living}, {@code mob} or {@code hanging} (then
     * {@code width}×{@code height} are blocks of wall); attributes by id;
     * {@code spawnEgg} {base, spots} or null; a hanging thing's sound
     * events, its place and break ones.
     */
    public record Entity(String id, String kind, String category, float width, float height, Float eyeHeight,
                         boolean fireImmune, boolean noGravity, Boolean pushable, boolean solid,
                         int trackingRange, int updateInterval, Map<String, Double> attributes,
                         int[] spawnEgg, List<String> goals, boolean ticks, String placeSound, String breakSound) {
        public boolean living() {
            return kind.equals("living") || kind.equals("mob");
        }

        public boolean hanging() {
            return kind.equals("hanging");
        }

        public boolean mob() {
            return kind.equals("mob");
        }
    }

    /** A number every player has; effects change it. */
    public record Attribute(String id, double defaultValue, double min, double max) {}

    /** How a modifier changes an attribute ({@code yog_registry::ModifierOperation}). */
    public enum Operation {
        ADD, MULTIPLY_BASE, MULTIPLY_TOTAL;

        /** The operation the data names ({@code multiply_base}); {@link #ADD} if none. */
        static Operation of(String word) {
            return word == null ? ADD : valueOf(word.toUpperCase(Locale.ROOT));
        }
    }

    /** Whether an effect is good or bad for whoever has it ({@code yog_registry::EffectCategory}). */
    public enum EffectCategory {
        BENEFICIAL, HARMFUL, NEUTRAL;

        static EffectCategory of(String word) {
            return word == null ? NEUTRAL : valueOf(word.toUpperCase(Locale.ROOT));
        }
    }

    /** An attribute an effect changes, by {@code amount} a level. */
    public record Modifier(String attribute, double amount, Operation operation) {}

    /** A mob effect: {@code beneficial}, {@code harmful} or {@code neutral}; {@code 0xRRGGBB}. */
    public record Effect(String id, EffectCategory category, int color, List<Modifier> modifiers) {}

    /** An effect a potion gives: ticks, 0 for level I. */
    public record PotionEffect(String effect, int duration, int amplifier) {}

    /** A brewing stand recipe: the potion it's brewed from and the ingredient. */
    public record Brew(String from, String with) {}

    /** A potion: its translations' name, its effects, how it's brewed. */
    public record Potion(String id, String name, List<PotionEffect> effects, List<Brew> brewed) {}

    public final List<Attribute> attributes = new ArrayList<>();
    public final List<Effect> effects = new ArrayList<>();
    public final List<Potion> potions = new ArrayList<>();

    /**
     * A Yog font as the game's text takes it (the runtime's atlas): one
     * cell per character, {@code rows} of them in the PNG, its baseline
     * {@code ascent} GUI pixels down a cell {@code height} high.
     */
    public record Font(String id, byte[] png, List<String> rows, int ascent, int height) {}

    public final List<Font> fonts = new ArrayList<>();

    public final List<Item> items;
    public final List<Block> blocks;
    /** By id. */
    public final Map<String, YogInventoryDef> inventories;
    public final List<Recipe> recipes;
    public final List<Dimension> dimensions;
    /** For the hosts that add features to biomes in code (Fabric). */
    public final List<BiomeFeature> biomeFeatures;
    public final List<Entity> entities;

    private YogContent(List<Item> items, List<Block> blocks, Map<String, YogInventoryDef> inventories,
                       List<Recipe> recipes, List<Dimension> dimensions, List<BiomeFeature> biomeFeatures,
                       List<Entity> entities) {
        this.entities = entities;
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

    /** Forgets the content read (a hot reload brought new code): the next phase reads it again. */
    public static synchronized void invalidate() {
        cached = null;
    }

    /** Reads the runtime's blob; {@code null} (no runtime) is no content. */
    public static YogContent read(byte[] blob) {
        if (blob == null || blob.length == 0) {
            return new YogContent(Collections.emptyList(), Collections.emptyList(), Collections.emptyMap(),
                    Collections.emptyList(), Collections.emptyList(), Collections.emptyList(), Collections.emptyList());
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
                    tool(m.get("tool")), armor(m.get("armor")), YogData.str(m.get("wearable"), null),
                    hangs(m.get("hangs"))));
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
                    YogData.str(m.get("inventory"), null), YogData.bool(m.get("redstone"), false),
                    YogData.str(m.get("facing"), ""), YogData.str(m.get("placement"), "toward"),
                    YogData.bool(m.get("attach"), false), YogData.bool(m.get("waterlogged"), false),
                    properties(m.get("properties")), YogData.str(m.get("powered"), null),
                    YogData.str(m.get("comparator"), null), YogData.bool(m.get("accepts_items"), false),
                    YogData.bool(m.get("no_occlusion"), false), YogData.str(m.get("kind"), ""),
                    YogData.str(m.get("base"), null), YogData.str(m.get("set"), null),
                    (int) YogData.integer(m.get("press_ticks"), 30), YogData.bool(m.get("mobs_only"), false)));
        }
        // A block made from another (stairs from planks) comes after it: the host makes it of it.
        List<Block> bases = new ArrayList<>();
        List<Block> made = new ArrayList<>();
        for (Block b : blocks) (b.base() == null ? bases : made).add(b);
        blocks = new ArrayList<>(bases);
        blocks.addAll(made);
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
        List<Entity> entities = new ArrayList<>();
        for (Object o : YogData.list(root.get("entities"))) {
            Map<String, Object> m = YogData.map(o);
            Map<String, Double> attributes = new java.util.LinkedHashMap<>();
            if (m.get("attributes") != null) {
                for (Map.Entry<String, Object> a : YogData.map(m.get("attributes")).entrySet()) {
                    attributes.put(a.getKey(), YogData.number(a.getValue(), 0));
                }
            }
            int[] egg = null;
            List<Object> eggColors = m.get("spawn_egg") == null ? null : YogData.list(m.get("spawn_egg"));
            if (eggColors != null && eggColors.size() == 2) {
                egg = new int[] {(int) YogData.integer(eggColors.get(0), 0), (int) YogData.integer(eggColors.get(1), 0)};
            }
            List<String> goals = new ArrayList<>();
            for (Object g : YogData.list(m.get("goals"))) goals.add(YogData.str(g, ""));
            entities.add(new Entity(YogData.str(m.get("id"), ""), YogData.str(m.get("kind"), "object"),
                    YogData.str(m.get("category"), "misc"), (float) YogData.number(m.get("width"), 1),
                    (float) YogData.number(m.get("height"), 1),
                    m.get("eye_height") == null ? null : (float) YogData.number(m.get("eye_height"), 0),
                    YogData.bool(m.get("fire_immune"), false), YogData.bool(m.get("no_gravity"), false),
                    m.get("pushable") == null ? null : YogData.bool(m.get("pushable"), false),
                    YogData.bool(m.get("solid"), false), (int) YogData.integer(m.get("tracking_range"), 8),
                    (int) YogData.integer(m.get("update_interval"), 3), attributes, egg, goals,
                    YogData.bool(m.get("ticks"), false),
                    YogData.str(m.get("place_sound"), "minecraft:entity.painting.place"),
                    YogData.str(m.get("break_sound"), "minecraft:entity.painting.break")));
        }
        YogContent c = new YogContent(items, blocks, inventories, recipes, dimensions, biomeFeatures, entities);
        for (Object o : YogData.list(root.get("attributes"))) {
            Map<String, Object> m = YogData.map(o);
            c.attributes.add(new Attribute(YogData.str(m.get("id"), ""), YogData.number(m.get("default"), 0),
                    YogData.number(m.get("min"), 0), YogData.number(m.get("max"), 1)));
        }
        for (Object o : YogData.list(root.get("effects"))) {
            Map<String, Object> m = YogData.map(o);
            List<Modifier> modifiers = new ArrayList<>();
            for (Object mo : YogData.list(m.get("modifiers"))) {
                Map<String, Object> mm = YogData.map(mo);
                modifiers.add(new Modifier(id(mm.get("attribute")), YogData.number(mm.get("amount"), 0), Operation.of(YogData.str(mm.get("operation"), null))));
            }
            c.effects.add(new Effect(YogData.str(m.get("id"), ""), EffectCategory.of(YogData.str(m.get("category"), null)),
                    (int) YogData.integer(m.get("color"), 0), modifiers));
        }
        for (Object o : YogData.list(root.get("potions"))) {
            Map<String, Object> m = YogData.map(o);
            String id = YogData.str(m.get("id"), "");
            List<PotionEffect> potionEffects = new ArrayList<>();
            for (Object eo : YogData.list(m.get("effects"))) {
                Map<String, Object> em = YogData.map(eo);
                potionEffects.add(new PotionEffect(id(em.get("effect")), (int) YogData.integer(em.get("duration"), 0),
                        (int) YogData.integer(em.get("amplifier"), 0)));
            }
            List<Brew> brewed = new ArrayList<>();
            for (Object bo : YogData.list(m.get("brewed"))) {
                Map<String, Object> bm = YogData.map(bo);
                brewed.add(new Brew(id(bm.get("from")), id(bm.get("with"))));
            }
            String name = YogData.str(m.get("name"), null);
            c.potions.add(new Potion(id, name != null ? name : id.substring(id.indexOf(':') + 1), potionEffects, brewed));
        }
        for (Object o : YogData.list(root.get("fonts"))) {
            Map<String, Object> m = YogData.map(o);
            List<String> rows = new ArrayList<>();
            for (Object r : YogData.list(m.get("rows"))) rows.add(YogData.str(r, ""));
            byte[] png = m.get("png") instanceof byte[] b ? b : new byte[0];
            c.fonts.add(new Font(YogData.str(m.get("id"), ""), png, rows, (int) YogData.integer(m.get("ascent"), 0),
                    (int) YogData.integer(m.get("height"), 0)));
        }
        return c;
    }

    /** A reference's id ({@code @effect:mymod:clarity}) or a plain one. */
    private static String id(Object v) {
        if (v instanceof YogData.Ref r) return r.id();
        return YogData.str(v, "");
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

    private static Hangs hangs(Object v) {
        if (v == null) return null;
        Map<String, Object> m = YogData.map(v);
        return new Hangs(YogData.str(m.get("entity"), ""), (int) YogData.integer(m.get("width"), 1),
                (int) YogData.integer(m.get("height"), 1));
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
        com.google.gson.JsonElement e = toGson(v);
        return e.isJsonObject() ? e.getAsJsonObject() : new com.google.gson.JsonObject();
    }

    /** A list of numbers as floats. */
    public static float[] floats(Object v) {
        List<Object> l = YogData.list(v);
        float[] out = new float[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = (float) YogData.number(l.get(i), 0);
        return out;
    }

    private static List<Property> properties(Object o) {
        List<Property> out = new ArrayList<>();
        for (Object p : YogData.list(o)) {
            Map<String, Object> m = YogData.map(p);
            List<String> values = new ArrayList<>();
            for (Object v : YogData.list(m.get("values"))) values.add(YogData.str(v, ""));
            out.add(new Property(YogData.str(m.get("name"), ""), values));
        }
        return out;
    }

    /**
     * {@code v} as Gson's JSON tree — what {@link YogData#toJson} writes, without
     * the text in between (typed values as their string forms).
     */
    static com.google.gson.JsonElement toGson(Object v) {
        if (v == null) return com.google.gson.JsonNull.INSTANCE;
        if (v instanceof Boolean b) return new com.google.gson.JsonPrimitive(b);
        if (v instanceof Long || v instanceof Integer || v instanceof Double) return new com.google.gson.JsonPrimitive((Number) v);
        if (v instanceof String s) return new com.google.gson.JsonPrimitive(s);
        if (v instanceof List<?> l) {
            com.google.gson.JsonArray a = new com.google.gson.JsonArray(l.size());
            for (Object x : l) a.add(toGson(x));
            return a;
        }
        if (v instanceof Map<?, ?> m) {
            com.google.gson.JsonObject o = new com.google.gson.JsonObject();
            for (Map.Entry<?, ?> e : m.entrySet()) o.add(String.valueOf(e.getKey()), toGson(e.getValue()));
            return o;
        }
        if (v instanceof YogData.Text t) return new com.google.gson.JsonPrimitive(t.text());
        StringBuilder one = new StringBuilder();
        one.append(YogData.toJson(v));
        return com.google.gson.JsonParser.parseString(one.toString());
    }
}
