package dev.yog.util;

import dev.yog.YogNames;
import dev.yog.YogNames.DimensionField;

import dev.yog.YogLog;

import com.google.gson.Gson;
import dev.yog.data.YogContent;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Comparator;
import java.util.function.Supplier;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * Stages the {@code assets/} and {@code data/} bundled inside {@code .yog}
 * mods into one directory that every loader's {@code YogPackProvider} exposes
 * as a single always-on pack. Pure Java (plus Gson, which every Minecraft
 * ships) — only the pack registration itself is loader/version specific.
 */
public final class YogPackStager {
    private YogPackStager() {
    }

    /** Pack id the staged directory is registered under. */
    public static final String PACK_ID = "yog_runtime";
    /** Display name of the staged pack. */
    public static final String PACK_TITLE = "Yog Mods";
    /** Staging directory, relative to the game directory. Wiped on every reload. */
    public static final String PACK_DIR = ".yog-pack";

    /** Namespace for dimension ids that don't specify one. */
    private static final String DEFAULT_NAMESPACE = dev.yog.YogNames.Ns.YOG;
    /** Chunk generator that forwards each chunk to the native per-chunk callback. */
    private static final String CALLBACK_GENERATOR = "yog:callback_generator";
    private static final int DEFAULT_MIN_Y = -64;
    private static final int DEFAULT_HEIGHT = 384;

    /**
     * Merge every {@code .yog}'s {@code assets/} and {@code data/} into
     * {@code <gameDir>/.yog-pack}, then add the mod-registered recipes and
     * dimensions. Returns {@code null} when there is no mods directory.
     *
     * <p>A {@code .yog} may also carry files for one loader or Minecraft
     * version only, under {@code when/<loader>/<version>/} ({@code any} for
     * either): {@code when/any/1.21.1/data/…}, {@code when/fabric/1.20.1/data/…}.
     * They're staged after the common ones, the more specific last. Tag
     * files ({@code data/<ns>/tags/…}) from several mods or overlays are
     * merged, not replaced.
     *
     * @param loader this host's loader: {@code fabric}, {@code forge}, {@code neoforge}
     * @param mcVersion this host's Minecraft version ({@code 1.21.1})
     * @param mc121 Minecraft 1.21+ data layout: singular datapack dir
     *     names and the 1.21 recipe result format (1.20-format recipes are
     *     upgraded on the fly)
     * @param packMcmeta contents of the generated {@code pack.mcmeta}
     * @param content the runtime's content blob ({@code NativeBridge.nativeContent()}):
     *     its recipes and dimensions become datapack JSON
     */
    public static synchronized Path stage(Path gameDir, String loader, String mcVersion, boolean mc121, String packMcmeta,
                                          Supplier<byte[]> content)
            throws IOException {
        Path mods = gameDir.resolve(YogNativeRuntime.MODS_DIR);
        Path out = gameDir.resolve(PACK_DIR);
        if (!Files.isDirectory(mods)) {
            return null;
        }

        if (Files.exists(out)) {
            try (var walk = Files.walk(out)) {
                walk.sorted(Comparator.reverseOrder()).forEach(p -> {
                    try {
                        Files.deleteIfExists(p);
                    } catch (IOException ignored) {
                    }
                });
            }
        }
        Files.createDirectories(out);
        dev.yog.data.YogTags.setHost(loader, mcVersion);
        YogDataLayout.setVersion(mcVersion);

        // The common files, then each overlay that applies, least specific first.
        // A version directory is the whole version or a release line of it: `1.21` covers 1.21.x.
        java.util.List<String> versions = new java.util.ArrayList<>(java.util.List.of(dev.yog.YogNames.Pack.ANY_VERSION));
        String[] parts = mcVersion.split("\\.");
        for (int i = 2; i <= parts.length; i++) versions.add(String.join(".", java.util.Arrays.copyOf(parts, i)));
        java.util.List<String> layers = new java.util.ArrayList<>(java.util.List.of(""));
        for (String v : versions) {
            layers.add(overlay(dev.yog.YogNames.Pack.ANY_LOADER, v));
            layers.add(overlay(loader, v));
        }
        for (String layer : layers) {
            try (var listing = Files.newDirectoryStream(mods, "*.yog")) {
                for (Path yog : listing) {
                    try (ZipFile zip = new ZipFile(yog.toFile())) {
                        var entries = zip.entries();
                        while (entries.hasMoreElements()) {
                            ZipEntry e = entries.nextElement();
                            String n = e.getName();
                            if (e.isDirectory() || !n.startsWith(layer)) {
                                continue;
                            }
                            n = n.substring(layer.length());
                            if (!(n.startsWith("assets/") || n.startsWith("data/"))) {
                                continue;
                            }
                            // Accept both pre-1.21 plural and 1.21+ singular datapack dir names.
                            n = YogDataLayout.remapDataPath(n, mc121);
                            Path target = out.resolve(n).normalize();
                            if (!target.startsWith(out)) {
                                continue; // zip-slip guard
                            }
                            Files.createDirectories(target.getParent());
                            try (InputStream in = zip.getInputStream(e)) {
                                if (mc121 && YogDataLayout.isRecipePath(n)) {
                                    // 1.20-format recipe results don't parse on 1.21.
                                    String json = new String(in.readAllBytes(), StandardCharsets.UTF_8);
                                    Files.writeString(target, YogDataLayout.upgradeRecipeJson(json));
                                } else if (isTagPath(n) && Files.exists(target)) {
                                    // Several mods (or overlays) adding to one tag: all of them.
                                    String json = new String(in.readAllBytes(), StandardCharsets.UTF_8);
                                    Files.writeString(target, mergeTags(Files.readString(target), json));
                                } else if (isAtlasPath(n) && Files.exists(target)) {
                                    // Several mods' textures in one atlas: every mod's sources.
                                    String json = new String(in.readAllBytes(), StandardCharsets.UTF_8);
                                    Files.writeString(target, mergeAtlases(Files.readString(target), json));
                                } else {
                                    Files.copy(in, target, StandardCopyOption.REPLACE_EXISTING);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Yog's common tags: what the mods put in them, to this host's tags.
        dev.yog.data.YogTags.stage(out, mc121);

        // Item textures in the items atlas, where this version has one.
        if (YogDataLayout.atLeast(mcVersion, ITEMS_ATLAS_SINCE)) splitItemsAtlas(out);

        YogContent registered = YogContent.cached(content);
        // Mod-registered recipes as JSON files inside the data pack.
        injectRecipes(out, registered.recipes, mc121);

        // Mod-declared dimensions as vanilla dimension_type/dimension JSON.
        injectDimensions(out, registered.dimensions);

        // The mods' Yog fonts, the game's too.
        injectFonts(out, registered.fonts);

        // The mods' key bindings' categories, named in the controls.
        injectKeyCategoryNames(out);

        // A name for each of the mods' item tags nothing names (recipe viewers show them).
        injectTagNames(out, mc121);

        Files.writeString(out.resolve("pack.mcmeta"), packMcmeta);
        return out;
    }

    /**
     * The names of the mods' key binding categories ({@link YogKeyBindings}),
     * in Yog's lang file of the game's default locale (merged with what a
     * mod put there): the controls show each mod's name over its keys.
     */
    private static void injectKeyCategoryNames(Path packRoot) throws IOException {
        injectNames(packRoot, YogKeyBindings.categoryNames());
    }

    /** The registry whose tags recipe viewers name, and the game's folder of them per layout. */
    private static final String ITEM_TAGS = "item", ITEM_TAGS_1_20 = "items";
    /** A tag's translation key: {@code tag.item.<ns>.<path with dots>}. */
    private static final String TAG_KEY = "tag." + ITEM_TAGS + ".";
    /** The lang file of Yog's common tags' names (the jar's, not the pack's). */
    private static final String COMMON_TAG_NAMES = "/assets/yog_tags/lang/" + dev.yog.YogNames.Game.LOCALE + dev.yog.YogNames.Res.JSON;

    /**
     * A name for every item tag in the pack none of the lang files names
     * (a mod's own tags): its path's words, last first ({@code ingots/iron}
     * → {@code Iron Ingots}) — recipe viewers show a tag by it.
     */
    private static void injectTagNames(Path packRoot, boolean mc121) throws IOException {
        java.util.Set<String> named = new java.util.HashSet<>(langKeys(packRoot));
        try (InputStream in = YogPackStager.class.getResourceAsStream(COMMON_TAG_NAMES)) {
            if (in != null) {
                JsonObject common = new Gson().fromJson(new String(in.readAllBytes(), StandardCharsets.UTF_8), JsonObject.class);
                if (common != null) named.addAll(common.keySet());
            }
        }
        java.util.Map<String, String> names = new java.util.TreeMap<>();
        Path data = packRoot.resolve(dev.yog.YogNames.Pack.DATA);
        if (!Files.isDirectory(data)) return;
        try (var namespaces = Files.newDirectoryStream(data)) {
            for (Path ns : namespaces) {
                Path tags = ns.resolve(dev.yog.YogNames.Pack.TAGS).resolve(mc121 ? ITEM_TAGS : ITEM_TAGS_1_20);
                if (!Files.isDirectory(tags)) continue;
                try (var walk = Files.walk(tags)) {
                    for (Path f : (Iterable<Path>) walk.filter(f -> f.toString().endsWith(dev.yog.YogNames.Res.JSON))::iterator) {
                        String rel = tags.relativize(f).toString().replace('\\', '/');
                        String path = rel.substring(0, rel.length() - dev.yog.YogNames.Res.JSON.length());
                        String key = TAG_KEY + ns.getFileName() + "." + path.replace('/', '.');
                        if (!named.contains(key)) names.put(key, tagName(path));
                    }
                }
            }
        }
        injectNames(packRoot, names);
    }

    /** {@code ingots/iron} → {@code Iron Ingots}: the path's words, the last folder first. */
    static String tagName(String path) {
        java.util.List<String> parts = new java.util.ArrayList<>(java.util.Arrays.asList(path.split("/")));
        java.util.Collections.reverse(parts);
        StringBuilder out = new StringBuilder();
        for (String part : parts) {
            for (String word : part.split("_")) {
                if (word.isEmpty()) continue;
                if (out.length() > 0) out.append(' ');
                out.append(Character.toUpperCase(word.charAt(0))).append(word.substring(1));
            }
        }
        return out.toString();
    }

    /** Every key the pack's lang files of the default locale name. */
    private static java.util.Set<String> langKeys(Path packRoot) throws IOException {
        java.util.Set<String> keys = new java.util.HashSet<>();
        Path assets = packRoot.resolve(dev.yog.YogNames.Pack.ASSETS);
        if (!Files.isDirectory(assets)) return keys;
        try (var namespaces = Files.newDirectoryStream(assets)) {
            for (Path ns : namespaces) {
                Path lang = ns.resolve(dev.yog.YogNames.Res.LANG + dev.yog.YogNames.Game.LOCALE + dev.yog.YogNames.Res.JSON);
                if (!Files.exists(lang)) continue;
                JsonObject entries = new Gson().fromJson(Files.readString(lang), JsonObject.class);
                if (entries != null) keys.addAll(entries.keySet());
            }
        }
        return keys;
    }

    /** {@code names} into Yog's lang file of the game's default locale, where nothing names them yet. */
    private static void injectNames(Path packRoot, java.util.Map<String, String> names) throws IOException {
        if (names.isEmpty()) return;
        Path lang = packRoot.resolve(dev.yog.YogNames.Pack.ASSETS).resolve(dev.yog.YogNames.Ns.YOG)
                .resolve(dev.yog.YogNames.Res.LANG + dev.yog.YogNames.Game.LOCALE + dev.yog.YogNames.Res.JSON);
        JsonObject entries = Files.exists(lang) ? new Gson().fromJson(Files.readString(lang), JsonObject.class) : new JsonObject();
        if (entries == null) entries = new JsonObject();
        for (java.util.Map.Entry<String, String> e : names.entrySet()) {
            if (!entries.has(e.getKey())) entries.addProperty(e.getKey(), e.getValue());
        }
        Files.createDirectories(lang.getParent());
        Files.writeString(lang, new Gson().toJson(entries));
    }

    /** {@code data/<ns>/tags/…} — a tag file. */
    static boolean isTagPath(String path) {
        String[] p = path.split("/", 4);
        return p.length == 4 && p[0].equals("data") && p[2].equals("tags");
    }

    /** {@code assets/<ns>/atlases/<atlas>.json} — an atlas's sources. */
    static boolean isAtlasPath(String path) {
        String[] p = path.split("/", 4);
        return p.length == 4 && p[0].equals(dev.yog.YogNames.Pack.ASSETS) && (p[2] + "/").equals(dev.yog.YogNames.Res.ATLASES);
    }

    /** Two atlas definitions as one: the sources of both, in order. */
    public static String mergeAtlases(String a, String b) {
        Gson gson = new Gson();
        JsonObject x = gson.fromJson(a, JsonObject.class);
        JsonObject y = gson.fromJson(b, JsonObject.class);
        if (x == null) return b;
        if (y == null) return a;
        com.google.gson.JsonArray sources = x.has(ATLAS_SOURCES) ? x.getAsJsonArray(ATLAS_SOURCES) : new com.google.gson.JsonArray();
        if (y.has(ATLAS_SOURCES)) sources.addAll(y.getAsJsonArray(ATLAS_SOURCES));
        x.add(ATLAS_SOURCES, sources);
        return gson.toJson(x);
    }

    /** An atlas definition's list of where its sprites come from. */
    private static final String ATLAS_SOURCES = YogNames.AtlasSource.SOURCES;
    /** The first version whose item textures have an atlas of their own ({@code minecraft:items}), not the blocks'. */
    private static final int[] ITEMS_ATLAS_SINCE = {1, 21, 11};
    /** The atlases' definitions: {@code assets/minecraft/atlases/<name>.json}. */
    private static final String BLOCKS_ATLAS = YogNames.AtlasSource.BLOCKS, ITEMS_ATLAS = YogNames.AtlasSource.ITEMS;
    /** The item textures' folder, which the items atlas takes whole: a sprite under it is an item's. */
    private static final String ITEM_SPRITES = YogNames.Res.ITEM_DIR;
    /** An atlas source's kind and fields: a whole texture, or sprites cut from one picture. */
    private static final String SOURCE_TYPE = YogNames.AtlasSource.TYPE, SINGLE = YogNames.AtlasSource.SINGLE,
            UNSTITCH = YogNames.AtlasSource.UNSTITCH, RESOURCE = YogNames.AtlasSource.RESOURCE,
            SPRITE = YogNames.AtlasSource.SPRITE, REGIONS = YogNames.AtlasSource.REGIONS;

    /**
     * The mods' item sprites out of the blocks atlas, into the items atlas
     * (1.21.11+): a texture of its own under {@code textures/item} is in the
     * items atlas already (its folder), so it's only dropped from the blocks
     * one — twice is a duplicate sprite; an item's region of a packed
     * picture moves to an {@code unstitch} of the same picture there.
     */
    private static void splitItemsAtlas(Path packRoot) throws IOException {
        Path atlases = packRoot.resolve(dev.yog.YogNames.Pack.ASSETS).resolve(dev.yog.YogNames.Ns.MINECRAFT).resolve(dev.yog.YogNames.Res.ATLASES);
        Path blocks = atlases.resolve(BLOCKS_ATLAS + dev.yog.YogNames.Res.JSON);
        if (!Files.exists(blocks)) return;
        Gson gson = new Gson();
        JsonObject atlas = gson.fromJson(Files.readString(blocks), JsonObject.class);
        if (atlas == null || !atlas.has(ATLAS_SOURCES)) return;
        com.google.gson.JsonArray kept = new com.google.gson.JsonArray(), moved = new com.google.gson.JsonArray();
        for (var element : atlas.getAsJsonArray(ATLAS_SOURCES)) {
            JsonObject source = element.getAsJsonObject();
            String type = source.has(SOURCE_TYPE) ? source.get(SOURCE_TYPE).getAsString() : "";
            if (type.endsWith(SINGLE) && isItemSprite(source.has(SPRITE) ? source.get(SPRITE) : source.get(RESOURCE))) continue;
            if (type.endsWith(UNSTITCH) && source.has(REGIONS)) {
                com.google.gson.JsonArray blockRegions = new com.google.gson.JsonArray(), itemRegions = new com.google.gson.JsonArray();
                for (var region : source.getAsJsonArray(REGIONS)) {
                    (isItemSprite(region.getAsJsonObject().get(SPRITE)) ? itemRegions : blockRegions).add(region);
                }
                if (!itemRegions.isEmpty()) {
                    JsonObject items = source.deepCopy();
                    items.add(REGIONS, itemRegions);
                    moved.add(items);
                }
                if (blockRegions.isEmpty()) continue;
                source.add(REGIONS, blockRegions);
            }
            kept.add(source);
        }
        atlas.add(ATLAS_SOURCES, kept);
        Files.writeString(blocks, gson.toJson(atlas));
        if (moved.isEmpty()) return;
        JsonObject items = new JsonObject();
        items.add(ATLAS_SOURCES, moved);
        Path itemsAtlas = atlases.resolve(ITEMS_ATLAS + dev.yog.YogNames.Res.JSON);
        String json = gson.toJson(items);
        Files.writeString(itemsAtlas, Files.exists(itemsAtlas) ? mergeAtlases(Files.readString(itemsAtlas), json) : json);
    }

    /** Whether a sprite's name ({@code ns:item/…}) is an item texture's. */
    private static boolean isItemSprite(com.google.gson.JsonElement name) {
        if (name == null) return false;
        String id = name.getAsString();
        return id.substring(id.indexOf(':') + 1).startsWith(ITEM_SPRITES);
    }

    /** Two tag files as one: the values of both (once each); {@code replace} if either says so. */
    public static String mergeTags(String a, String b) {
        Gson gson = new Gson();
        JsonObject x = gson.fromJson(a, JsonObject.class);
        JsonObject y = gson.fromJson(b, JsonObject.class);
        if (x == null) return b;
        if (y == null) return a;
        com.google.gson.JsonArray values = x.has("values") ? x.getAsJsonArray("values") : new com.google.gson.JsonArray();
        if (y.has("values")) {
            for (var v : y.getAsJsonArray("values")) {
                if (!values.contains(v)) values.add(v);
            }
        }
        x.add("values", values);
        if (y.has("replace") && y.get("replace").getAsBoolean()) x.addProperty("replace", true);
        return gson.toJson(x);
    }

    private static void injectRecipes(Path packRoot, java.util.List<YogContent.Recipe> recipes, boolean mc121) {
        for (YogContent.Recipe r : recipes) {
            String namespace = r.namespace();
            String name     = r.name();
            try {
                String vanilla = YogDataLayout.yogRecipeToVanilla(r.data(), namespace, mc121);
                if (vanilla == null) continue;
                Path target = packRoot
                        .resolve("data").resolve(namespace).resolve(YogDataLayout.recipeDir(mc121))
                        .resolve(name + ".json");
                Files.createDirectories(target.getParent());
                Files.writeString(target, vanilla);
            } catch (IOException e) {
                YogLog.warn("failed to write recipe " + namespace + ":" + name + ": " + e);
            }
        }
    }

    /**
     * Convert mod-declared {@code YogDimensionDef} JSON (our shape — see the
     * {@code yog-dimensions} Rust crate) into vanilla {@code dimension_type}
     * / {@code dimension} datapack JSON. Writing to a vanilla id (e.g.
     * {@code minecraft:overworld}) patches that dimension instead of adding
     * a new one, since this pack loads at TOP priority.
     *
     * <p>Custom generator types ({@code generator_type} in the def) reference
     * {@code yog:callback_generator} (see {@code YogCallbackChunkGenerator}),
     * which forwards each chunk to the native per-chunk callback. Dimensions
     * without a {@code generator_type} fall back to a plain vanilla noise
     * generator.
     */
    /** A dimension's look before 26.1, and the skies 26.x names for them. */
    private static final String EFFECTS_OVERWORLD = "minecraft:overworld";
    private static final String EFFECTS_END = "minecraft:the_end";
    private static final String EFFECTS_NETHER = "minecraft:the_nether";
    private static final String SKYBOX_OVERWORLD = "overworld", SKYBOX_END = "end", SKYBOX_NONE = "none";
    /** 26.x dimension attributes: whether beds work and explode, whether respawn anchors work. */
    private static final String ATTRIBUTE_BED_RULE = "minecraft:gameplay/bed_rule";
    private static final String ATTRIBUTE_ANCHORS_WORK = "minecraft:gameplay/respawn_anchor_works";

    /** The 26.x sky of a dimension whose effects are {@code effects}. */
    private static String skybox(String effects) {
        return switch (effects) {
            case EFFECTS_END -> SKYBOX_END;
            case EFFECTS_NETHER -> SKYBOX_NONE;
            default -> SKYBOX_OVERWORLD;
        };
    }

    /** A bitmap font provider's fields, as every version reads them. */
    private static final String FONT_PROVIDERS = "providers", FONT_TYPE = "type", FONT_BITMAP = "bitmap", FONT_FILE = "file",
            FONT_ASCENT = "ascent", FONT_HEIGHT = "height", FONT_CHARS = "chars";
    /** Where a font and its atlas go: {@code assets/<ns>/font/<path>.json}, {@code textures/font/<path>.png}. */
    private static final String FONT_DIR = "font", FONT_TEXTURE_DIR = "textures/font";

    /**
     * The mods' Yog fonts as the game's (a bitmap font each, its atlas from
     * the runtime): the game's own text — chat, signs, tooltips — in one by
     * the same id Yog draws it by.
     */
    private static void injectFonts(Path packRoot, java.util.List<YogContent.Font> fonts) {
        Gson gson = new Gson();
        for (YogContent.Font f : fonts) {
            int colon = f.id().indexOf(':');
            String ns = colon > 0 ? f.id().substring(0, colon) : DEFAULT_NAMESPACE;
            String path = f.id().substring(colon + 1);
            try {
                Path assets = packRoot.resolve(dev.yog.YogNames.Pack.ASSETS).resolve(ns);
                Path png = assets.resolve(FONT_TEXTURE_DIR).resolve(path + dev.yog.YogNames.Res.PNG);
                Files.createDirectories(png.getParent());
                Files.write(png, f.png());
                com.google.gson.JsonObject provider = new com.google.gson.JsonObject();
                provider.addProperty(FONT_TYPE, FONT_BITMAP);
                provider.addProperty(FONT_FILE, ns + ":" + FONT_DIR + "/" + path + dev.yog.YogNames.Res.PNG);
                provider.addProperty(FONT_ASCENT, f.ascent());
                provider.addProperty(FONT_HEIGHT, f.height());
                com.google.gson.JsonArray chars = new com.google.gson.JsonArray();
                f.rows().forEach(chars::add);
                provider.add(FONT_CHARS, chars);
                com.google.gson.JsonArray providers = new com.google.gson.JsonArray();
                providers.add(provider);
                com.google.gson.JsonObject json = new com.google.gson.JsonObject();
                json.add(FONT_PROVIDERS, providers);
                Path file = assets.resolve(FONT_DIR).resolve(path + dev.yog.YogNames.Res.JSON);
                Files.createDirectories(file.getParent());
                Files.writeString(file, gson.toJson(json));
            } catch (IOException e) {
                dev.yog.YogLog.warn("font " + f.id() + ": " + e);
            }
        }
    }

    private static void injectDimensions(Path packRoot, java.util.List<YogContent.Dimension> dimensions) {
        Gson gson = new Gson();
        for (YogContent.Dimension d : dimensions) {
            String id = d.id();
            try {
                int colon = id.indexOf(':');
                String namespace = colon > 0 ? id.substring(0, colon) : DEFAULT_NAMESPACE;
                String name = colon > 0 ? id.substring(colon + 1) : id;

                JsonObject def = d.def();
                JsonObject dt = def.has(DimensionField.DIMENSION_TYPE) ? def.getAsJsonObject(DimensionField.DIMENSION_TYPE) : new JsonObject();

                JsonObject dimensionType = new JsonObject();
                dimensionType.addProperty("ultrawarm", jbool(dt, "ultrawarm", false));
                dimensionType.addProperty("natural", jbool(dt, "natural", true));
                dimensionType.addProperty("coordinate_scale", jnum(dt, "coordinate_scale", 1.0));
                dimensionType.addProperty("has_skylight", jbool(dt, "has_sky_light", true));
                dimensionType.addProperty("has_ceiling", jbool(dt, "has_ceiling", false));
                dimensionType.addProperty("ambient_light", jnum(dt, "ambient_light", 0.0));
                dimensionType.addProperty("bed_works", !jbool(dt, "beds_explode", false));
                dimensionType.addProperty("respawn_anchor_works", !jbool(dt, "respawn_anchors_explode", false));
                dimensionType.addProperty("min_y", (int) jnum(dt, "min_y", DEFAULT_MIN_Y));
                dimensionType.addProperty("height", (int) jnum(dt, "height", DEFAULT_HEIGHT));
                dimensionType.addProperty("logical_height", (int) jnum(dt, "logical_height", DEFAULT_HEIGHT));
                dimensionType.addProperty("infiniburn", "#minecraft:infiniburn_overworld");
                dimensionType.addProperty("effects", jstr(dt, "effects", EFFECTS_OVERWORLD));
                dimensionType.addProperty("piglin_safe", jbool(dt, "piglin_safe", false));
                boolean mobs = jbool(def, DimensionField.MOB_SPAWNING, true);
                dimensionType.addProperty("has_raids", mobs);
                dimensionType.addProperty("monster_spawn_block_light_limit", 0);
                dimensionType.addProperty("monster_spawn_light_level", mobs ? 7 : 0);
                // 26.x: these, and beds/anchors as attributes; older versions ignore them.
                dimensionType.addProperty("has_ender_dragon_fight", false);
                dimensionType.addProperty("skybox", skybox(jstr(dt, "effects", EFFECTS_OVERWORLD)));
                JsonObject attributes = new JsonObject();
                if (jbool(dt, "beds_explode", false)) {
                    JsonObject bed = new JsonObject();
                    bed.addProperty("can_set_spawn", "never");
                    bed.addProperty("can_sleep", "never");
                    bed.addProperty("explodes", true);
                    attributes.add(ATTRIBUTE_BED_RULE, bed);
                }
                if (jbool(dt, "respawn_anchors_explode", false)) attributes.addProperty(ATTRIBUTE_ANCHORS_WORK, false);
                dimensionType.add("attributes", attributes);

                JsonObject biomeSource = new JsonObject();
                biomeSource.addProperty("type", "minecraft:fixed");
                biomeSource.addProperty("biome", jstr(def, DimensionField.BIOME, YogNames.Game.PLAINS));
                JsonObject generator = new JsonObject();
                if (def.has(DimensionField.GENERATOR_TYPE) && !def.get(DimensionField.GENERATOR_TYPE).isJsonNull()) {
                    generator.addProperty("type", CALLBACK_GENERATOR);
                    generator.addProperty("generator_type_id", def.get(DimensionField.GENERATOR_TYPE).getAsString());
                    generator.addProperty("min_y", (int) jnum(dt, "min_y", DEFAULT_MIN_Y));
                    generator.addProperty("height", (int) jnum(dt, "height", DEFAULT_HEIGHT));
                    generator.addProperty("structures", jbool(def, DimensionField.STRUCTURES, true));
                    generator.addProperty("features", jbool(def, DimensionField.FEATURES, true));
                    generator.addProperty("mob_spawning", mobs);
                    generator.add("biome_source", biomeSource);
                } else {
                    generator.addProperty("type", "minecraft:noise");
                    generator.addProperty("settings", "minecraft:overworld");
                    generator.add("biome_source", biomeSource);
                }

                JsonObject dimension = new JsonObject();
                dimension.addProperty("type", namespace + ":" + name);
                dimension.add("generator", generator);

                Path dtPath = packRoot.resolve("data").resolve(namespace).resolve("dimension_type").resolve(name + ".json");
                Path dPath = packRoot.resolve("data").resolve(namespace).resolve("dimension").resolve(name + ".json");
                Files.createDirectories(dtPath.getParent());
                Files.createDirectories(dPath.getParent());
                Files.writeString(dtPath, gson.toJson(dimensionType));
                Files.writeString(dPath, gson.toJson(dimension));
            } catch (Exception e) {
                YogLog.warn("failed to inject dimension '" + id + "': " + e);
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

    /** The overlay folder of {@code loader} on {@code version}: {@code when/<loader>/<version>/}. */
    private static String overlay(String loader, String version) {
        String sep = dev.yog.YogNames.Sep.PATH;
        return dev.yog.YogNames.Pack.OVERLAYS + sep + loader + sep + version + sep;
    }
}
