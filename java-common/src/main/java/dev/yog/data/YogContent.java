package dev.yog.data;

import dev.yog.YogLog;
import dev.yog.bus.YogTypes;
import dev.yog.util.YogInventoryDef;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;

/**
 * What the native runtime has registered, as the host reads it: the
 * runtime's content blob ({@code NativeBridge.nativeContent()}, the Rust
 * {@code yog_event::content::Content}) through the records generated from it
 * ({@link YogTypes.Content}), and what the hosts ask of those records.
 */
public final class YogContent {
    public final List<YogTypes.Item> items;
    /** Blocks made from another (stairs from planks) after the ones they're made from. */
    public final List<YogTypes.Block> blocks;
    /** By id. */
    public final Map<String, YogInventoryDef> inventories = new HashMap<>();
    public final List<YogTypes.Recipe> recipes;
    public final List<YogTypes.Dimension> dimensions;
    /** For the hosts that add features to biomes in code (Fabric). */
    public final List<YogTypes.BiomeFeature> biomeFeatures;
    public final List<YogTypes.EntityDef> entities;
    public final List<YogTypes.AttributeDef> attributes;
    public final List<YogTypes.EffectDef> effects;
    public final List<YogTypes.PotionDef> potions;
    public final List<YogTypes.Font> fonts;
    public final List<YogTypes.ParticleDef> particles;

    private YogContent(YogTypes.Content c) {
        items = c.items();
        blocks = c.blocks();
        for (YogTypes.Inventory inv : c.inventories()) inventories.put(inv.id(), YogInventoryDef.of(inv));
        recipes = c.recipes();
        dimensions = c.dimensions();
        biomeFeatures = c.biomeFeatures();
        entities = c.entities();
        attributes = c.attributes();
        effects = c.effects();
        potions = c.potions();
        fonts = c.fonts();
        particles = c.particles();
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
        boolean none = blob == null || blob.length == 0;
        return new YogContent(none ? YogTypes.Content.fromData(Map.of()) : YogTypes.Content.fromBytes(blob));
    }

    /**
     * Tool {@code t}'s attack, what it doesn't give taken from the game's tool: {@code game} reads a game
     * item's attack damage and speed modifiers ({@code [damage, speed]}, null for no such item).
     */
    public static YogTypes.Tool resolved(YogTypes.Tool t, Function<String, float[]> game) {
        if (t.attackDamage() != null && t.attackSpeed() != null) return t;
        YogTypes.GameTool gameTool = t.gameTool();
        float materialDamage = gameTool == null ? 0 : gameTool.materialDamage();
        float[] a = gameTool == null ? null : game.apply(gameTool.item());
        if (a == null) {
            YogLog.warn("tool " + t.kind() + " of " + t.material().id() + ": no game tool to take its attack from");
            a = new float[] {materialDamage, 0};
        }
        float damage = t.attackDamage() == null ? a[0] - materialDamage : t.attackDamage();
        float speed = t.attackSpeed() == null ? a[1] : t.attackSpeed();
        return new YogTypes.Tool(t.kind(), damage, speed, t.material(), gameTool);
    }

    /** The game's base durability of an armor piece in each slot, the material's durability multiplies it. */
    private static final int HEAD_DURABILITY = 11;
    private static final int CHEST_DURABILITY = 16;
    private static final int LEGS_DURABILITY = 15;
    private static final int FEET_DURABILITY = 13;

    /** An armor piece's durability: its slot's base times the material's. */
    public static int armorDurability(YogTypes.Armor a) {
        int base = switch (a.slot()) {
            case YogTypes.ArmorSlot.CHEST -> CHEST_DURABILITY;
            case YogTypes.ArmorSlot.LEGS -> LEGS_DURABILITY;
            case YogTypes.ArmorSlot.FEET -> FEET_DURABILITY;
            default -> HEAD_DURABILITY;
        };
        return base * (int) a.material().durability();
    }

    /** An armor piece's defense points. */
    public static int armorDefense(YogTypes.Armor a) {
        YogTypes.ArmorDefense d = a.material().defense();
        return (int) switch (a.slot()) {
            case YogTypes.ArmorSlot.CHEST -> d.chest();
            case YogTypes.ArmorSlot.LEGS -> d.legs();
            case YogTypes.ArmorSlot.FEET -> d.feet();
            default -> d.head();
        };
    }

    /** Whether block {@code b} is a {@code YogStateBlock}: states of its own, or it takes items. */
    public static boolean stateful(YogTypes.Block b) {
        return b.facing() != null || b.attach() || b.waterlogged() || !b.properties().isEmpty() || b.acceptsItems();
    }

    /** The values of a yes/no property. */
    private static final List<String> YES_NO = List.of(String.valueOf(false), String.valueOf(true));

    /** Whether property {@code p} is a yes/no: {@code false, true}. */
    public static boolean yesNo(YogTypes.BlockProperty p) {
        return p.values().equals(YES_NO);
    }

    /** Whether entity type {@code e} is alive: living, or a mob. */
    public static boolean living(YogTypes.EntityDef e) {
        return e.kind().equals(YogTypes.EntityKind.LIVING) || mob(e);
    }

    public static boolean mob(YogTypes.EntityDef e) {
        return e.kind().equals(YogTypes.EntityKind.MOB);
    }

    public static boolean hanging(YogTypes.EntityDef e) {
        return e.kind().equals(YogTypes.EntityKind.HANGING);
    }

    /** The name a potion's translations go by: its own, else its id's path. */
    public static String potionName(YogTypes.PotionDef p) {
        return p.name() != null ? p.name() : YogIds.path(p.id());
    }

    /** A recipe's or dimension's definition as a JSON object (an empty one if it isn't an object). */
    public static com.google.gson.JsonObject json(Object v) {
        com.google.gson.JsonElement e = toGson(v);
        return e.isJsonObject() ? e.getAsJsonObject() : new com.google.gson.JsonObject();
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
        return com.google.gson.JsonParser.parseString(YogData.toJson(v));
    }
}
