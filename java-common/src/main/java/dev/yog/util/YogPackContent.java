package dev.yog.util;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import dev.yog.YogLog;
import dev.yog.YogNames;
import dev.yog.YogNames.CallbackGenerator;
import dev.yog.YogNames.DimensionField;
import dev.yog.YogNames.GameDimension;
import dev.yog.data.YogContent;
import dev.yog.data.YogIds;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/**
 * What the runtime's content blob adds to the staged pack: the mods'
 * recipes, dimensions and fonts as the game's datapack and resource JSON.
 */
final class YogPackContent {
    private YogPackContent() {
    }

    /** Namespace for ids that don't name one. */
    private static final String DEFAULT_NAMESPACE = YogNames.Ns.YOG;
    /** A dimension's height when it doesn't say: the overworld's. */
    private static final int DEFAULT_MIN_Y = -64;
    private static final int DEFAULT_HEIGHT = 384;
    /** The light a monster spawns in, at most: none where mobs don't spawn. */
    private static final int MONSTER_LIGHT = 7;

    /** The mods' recipes as the game's, in this version's layout ({@code mc121}). */
    static void recipes(Path packRoot, List<YogContent.Recipe> recipes, boolean mc121) {
        for (YogContent.Recipe r : recipes) {
            try {
                String vanilla = YogDataLayout.yogRecipeToVanilla(r.data(), r.namespace(), mc121);
                if (vanilla == null) continue;
                write(dataFile(packRoot, r.namespace(), YogDataLayout.recipeDir(mc121), r.name()), vanilla);
            } catch (IOException e) {
                YogLog.warn("failed to write recipe " + YogIds.join(r.namespace(), r.name()) + ": " + e);
            }
        }
    }

    /** {@code data/<ns>/<dir>/<name>.json} of the pack. */
    private static Path dataFile(Path packRoot, String ns, String dir, String name) {
        return packRoot.resolve(YogNames.Pack.DATA).resolve(ns).resolve(dir).resolve(name + YogNames.Res.JSON);
    }

    private static void write(Path file, String text) throws IOException {
        Files.createDirectories(file.getParent());
        Files.writeString(file, text);
    }

    /**
     * The mods' dimensions (yog-cli's dimension data) as the game's
     * {@code dimension_type} and {@code dimension} JSON. A vanilla id
     * ({@code minecraft:overworld}) patches that dimension: the pack loads
     * at TOP priority. A dimension with a generator of Yog's names
     * {@code yog:callback_generator} (see {@code YogCallbackChunkGenerator});
     * one without, the game's noise generator.
     */
    static void dimensions(Path packRoot, List<YogContent.Dimension> dimensions) {
        Gson gson = new Gson();
        for (YogContent.Dimension d : dimensions) {
            String id = YogIds.qualifyIn(DEFAULT_NAMESPACE, d.id());
            int colon = id.indexOf(YogNames.Sep.NAMESPACE);
            String namespace = id.substring(0, colon), name = id.substring(colon + 1);
            JsonObject def = d.def();
            JsonObject dt = def.has(DimensionField.DIMENSION_TYPE) ? def.getAsJsonObject(DimensionField.DIMENSION_TYPE) : new JsonObject();
            JsonObject dimension = new JsonObject();
            dimension.addProperty(YogNames.Word.TYPE, id);
            dimension.add("generator", generator(def, dt));
            try {
                write(dataFile(packRoot, namespace, GameDimension.DIMENSION_TYPE, name), gson.toJson(dimensionType(def, dt)));
                write(dataFile(packRoot, namespace, GameDimension.DIMENSION, name), gson.toJson(dimension));
            } catch (IOException e) {
                YogLog.warn("failed to inject dimension '" + id + "': " + e);
            }
        }
    }

    /** The 26.x sky of a dimension whose effects are {@code effects}. */
    private static String skybox(String effects) {
        return switch (effects) {
            case GameDimension.EFFECTS_END -> "end";
            case GameDimension.EFFECTS_NETHER -> YogNames.Word.NONE;
            default -> "overworld";
        };
    }

    /** The game's dimension type of dimension {@code def} (its own type {@code dt}). */
    private static JsonObject dimensionType(JsonObject def, JsonObject dt) {
        JsonObject t = new JsonObject();
        t.addProperty("ultrawarm", jbool(dt, "ultrawarm", false));
        t.addProperty("natural", jbool(dt, "natural", true));
        t.addProperty("coordinate_scale", jnum(dt, "coordinate_scale", 1.0));
        t.addProperty("has_skylight", jbool(dt, "has_sky_light", true));
        t.addProperty("has_ceiling", jbool(dt, "has_ceiling", false));
        t.addProperty("ambient_light", jnum(dt, "ambient_light", 0.0));
        t.addProperty("bed_works", !jbool(dt, "beds_explode", false));
        t.addProperty("respawn_anchor_works", !jbool(dt, "respawn_anchors_explode", false));
        t.addProperty("min_y", (int) jnum(dt, "min_y", DEFAULT_MIN_Y));
        t.addProperty("height", (int) jnum(dt, "height", DEFAULT_HEIGHT));
        t.addProperty("logical_height", (int) jnum(dt, "logical_height", DEFAULT_HEIGHT));
        t.addProperty("infiniburn", GameDimension.INFINIBURN);
        String effects = jstr(dt, "effects", GameDimension.EFFECTS_OVERWORLD);
        t.addProperty("effects", effects);
        t.addProperty("piglin_safe", jbool(dt, "piglin_safe", false));
        boolean mobs = jbool(def, DimensionField.MOB_SPAWNING, true);
        t.addProperty("has_raids", mobs);
        t.addProperty("monster_spawn_block_light_limit", 0);
        t.addProperty("monster_spawn_light_level", mobs ? MONSTER_LIGHT : 0);
        // 26.x: these, and beds/anchors as attributes; older versions ignore them.
        t.addProperty("has_ender_dragon_fight", false);
        t.addProperty("skybox", skybox(effects));
        t.add("attributes", attributes(dt));
        return t;
    }

    /** 26.x's attributes of beds and respawn anchors that explode. */
    private static JsonObject attributes(JsonObject dt) {
        JsonObject attributes = new JsonObject();
        if (jbool(dt, "beds_explode", false)) {
            JsonObject bed = new JsonObject();
            bed.addProperty("can_set_spawn", "never");
            bed.addProperty("can_sleep", "never");
            bed.addProperty("explodes", true);
            attributes.add(GameDimension.BED_RULE, bed);
        }
        if (jbool(dt, "respawn_anchors_explode", false)) attributes.addProperty(GameDimension.ANCHORS_WORK, false);
        return attributes;
    }

    /** The dimension's generator: Yog's in code, or the game's noise. */
    private static JsonObject generator(JsonObject def, JsonObject dt) {
        JsonObject biomeSource = new JsonObject();
        biomeSource.addProperty(YogNames.Word.TYPE, GameDimension.FIXED_BIOMES);
        biomeSource.addProperty(YogNames.Kind.BIOME, jstr(def, DimensionField.BIOME, YogNames.Game.PLAINS));
        JsonObject generator = new JsonObject();
        if (def.has(DimensionField.GENERATOR_TYPE) && !def.get(DimensionField.GENERATOR_TYPE).isJsonNull()) {
            generator.addProperty(YogNames.Word.TYPE, CallbackGenerator.TYPE);
            generator.addProperty(CallbackGenerator.GENERATOR_TYPE_ID, def.get(DimensionField.GENERATOR_TYPE).getAsString());
            generator.addProperty(CallbackGenerator.MIN_Y, (int) jnum(dt, "min_y", DEFAULT_MIN_Y));
            generator.addProperty(CallbackGenerator.HEIGHT, (int) jnum(dt, "height", DEFAULT_HEIGHT));
            generator.addProperty(CallbackGenerator.STRUCTURES, jbool(def, DimensionField.STRUCTURES, true));
            generator.addProperty(CallbackGenerator.FEATURES, jbool(def, DimensionField.FEATURES, true));
            generator.addProperty(CallbackGenerator.MOB_SPAWNING, jbool(def, DimensionField.MOB_SPAWNING, true));
        } else {
            generator.addProperty(YogNames.Word.TYPE, GameDimension.NOISE);
            generator.addProperty("settings", GameDimension.OVERWORLD_NOISE);
        }
        generator.add(CallbackGenerator.BIOME_SOURCE, biomeSource);
        return generator;
    }

    /** A bitmap font provider's fields, as every version reads them. */
    private static final String FONT_PROVIDERS = "providers", FONT_BITMAP = "bitmap", FONT_FILE = "file",
            FONT_ASCENT = "ascent", FONT_HEIGHT = "height", FONT_CHARS = "chars";
    /** Where a font and its atlas go: {@code assets/<ns>/font/<path>.json}, {@code textures/font/<path>.png}. */
    private static final String FONT_DIR = YogNames.Kind.FONT, FONT_TEXTURE_DIR = YogNames.Res.TEXTURES + FONT_DIR;

    /**
     * The mods' Yog fonts as the game's (a bitmap font each, its atlas from
     * the runtime): the game's own text — chat, signs, tooltips — in one by
     * the same id Yog draws it by.
     */
    static void fonts(Path packRoot, List<YogContent.Font> fonts) {
        Gson gson = new Gson();
        for (YogContent.Font f : fonts) {
            String id = YogIds.qualifyIn(DEFAULT_NAMESPACE, f.id());
            int colon = id.indexOf(YogNames.Sep.NAMESPACE);
            String ns = id.substring(0, colon), path = id.substring(colon + 1);
            try {
                Path assets = packRoot.resolve(YogNames.Pack.ASSETS).resolve(ns);
                Path png = assets.resolve(FONT_TEXTURE_DIR).resolve(path + YogNames.Res.PNG);
                Files.createDirectories(png.getParent());
                Files.write(png, f.png());
                JsonObject provider = new JsonObject();
                provider.addProperty(YogNames.Word.TYPE, FONT_BITMAP);
                provider.addProperty(FONT_FILE, YogIds.join(ns, FONT_DIR + YogNames.Sep.PATH + path + YogNames.Res.PNG));
                provider.addProperty(FONT_ASCENT, f.ascent());
                provider.addProperty(FONT_HEIGHT, f.height());
                JsonArray chars = new JsonArray();
                f.rows().forEach(chars::add);
                provider.add(FONT_CHARS, chars);
                JsonArray providers = new JsonArray();
                providers.add(provider);
                JsonObject json = new JsonObject();
                json.add(FONT_PROVIDERS, providers);
                write(assets.resolve(FONT_DIR).resolve(path + YogNames.Res.JSON), gson.toJson(json));
            } catch (IOException e) {
                YogLog.warn("font " + f.id() + ": " + e);
            }
        }
    }

    private static boolean jbool(JsonObject o, String key, boolean def) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsBoolean() : def;
    }

    private static double jnum(JsonObject o, String key, double def) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsDouble() : def;
    }

    private static String jstr(JsonObject o, String key, String def) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : def;
    }
}
