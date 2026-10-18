package dev.yog.util;

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

        YogContent registered = YogContent.cached(content);
        // Mod-registered recipes as JSON files inside the data pack.
        injectRecipes(out, registered.recipes, mc121);

        // Mod-declared dimensions as vanilla dimension_type/dimension JSON.
        injectDimensions(out, registered.dimensions);

        Files.writeString(out.resolve("pack.mcmeta"), packMcmeta);
        return out;
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
    private static final String ATLAS_SOURCES = "sources";

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
    private static void injectDimensions(Path packRoot, java.util.List<YogContent.Dimension> dimensions) {
        Gson gson = new Gson();
        for (YogContent.Dimension d : dimensions) {
            String id = d.id();
            try {
                int colon = id.indexOf(':');
                String namespace = colon > 0 ? id.substring(0, colon) : DEFAULT_NAMESPACE;
                String name = colon > 0 ? id.substring(colon + 1) : id;

                JsonObject def = d.def();
                JsonObject dt = def.has("dimension_type") ? def.getAsJsonObject("dimension_type") : new JsonObject();

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
                dimensionType.addProperty("effects", jstr(dt, "effects", "minecraft:overworld"));
                dimensionType.addProperty("piglin_safe", jbool(dt, "piglin_safe", false));
                boolean mobs = jbool(def, "mob_spawning", true);
                dimensionType.addProperty("has_raids", mobs);
                dimensionType.addProperty("monster_spawn_block_light_limit", 0);
                dimensionType.addProperty("monster_spawn_light_level", mobs ? 7 : 0);

                JsonObject biomeSource = new JsonObject();
                biomeSource.addProperty("type", "minecraft:fixed");
                biomeSource.addProperty("biome", jstr(def, "biome", "minecraft:plains"));
                JsonObject generator = new JsonObject();
                if (def.has("generator_type") && !def.get("generator_type").isJsonNull()) {
                    generator.addProperty("type", CALLBACK_GENERATOR);
                    generator.addProperty("generator_type_id", def.get("generator_type").getAsString());
                    generator.addProperty("min_y", (int) jnum(dt, "min_y", DEFAULT_MIN_Y));
                    generator.addProperty("height", (int) jnum(dt, "height", DEFAULT_HEIGHT));
                    generator.addProperty("structures", jbool(def, "structures", true));
                    generator.addProperty("features", jbool(def, "features", true));
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
