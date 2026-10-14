package dev.yog.util;

import com.google.gson.Gson;
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
    private static final String DEFAULT_NAMESPACE = "yog";
    /** Chunk generator that forwards each chunk to the native per-chunk callback. */
    private static final String CALLBACK_GENERATOR = "yog:callback_generator";
    private static final int DEFAULT_MIN_Y = -64;
    private static final int DEFAULT_HEIGHT = 384;

    /**
     * Merge every {@code .yog}'s {@code assets/} and {@code data/} into
     * {@code <gameDir>/.yog-pack}, then add the mod-registered recipes and
     * dimensions. Returns {@code null} when there is no mods directory.
     *
     * @param mc121 Minecraft 1.21+ data layout: singular datapack dir
     *     names and the 1.21 recipe result format (1.20-format recipes are
     *     upgraded on the fly)
     * @param packMcmeta contents of the generated {@code pack.mcmeta}
     * @param recipeJsons {@code namespace\tname\tjson} lines from the runtime
     * @param dimensionJsons {@code id\tYogDimensionDef json} lines from the runtime
     */
    public static synchronized Path stage(Path gameDir, boolean mc121, String packMcmeta,
                                          Supplier<String> recipeJsons, Supplier<String> dimensionJsons)
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

        try (var listing = Files.newDirectoryStream(mods, "*.yog")) {
            for (Path yog : listing) {
                try (ZipFile zip = new ZipFile(yog.toFile())) {
                    var entries = zip.entries();
                    while (entries.hasMoreElements()) {
                        ZipEntry e = entries.nextElement();
                        String n = e.getName();
                        if (e.isDirectory() || !(n.startsWith("assets/") || n.startsWith("data/"))) {
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
                            } else {
                                Files.copy(in, target, StandardCopyOption.REPLACE_EXISTING);
                            }
                        }
                    }
                }
            }
        }

        // Inject mod-registered recipes as JSON files inside the data pack.
        injectRecipes(out, recipeJsons.get(), mc121);

        // Inject mod-declared dimensions as vanilla dimension_type/dimension JSON.
        injectDimensions(out, dimensionJsons.get());

        Files.writeString(out.resolve("pack.mcmeta"), packMcmeta);
        return out;
    }

    private static void injectRecipes(Path packRoot, String lines, boolean mc121) {
        if (lines == null || lines.isBlank()) return;
        for (String line : lines.split("\n")) {
            if (line.isBlank()) continue;
            String[] parts = line.split("\t", 3);
            if (parts.length < 3) continue;
            String namespace = parts[0];
            String name     = parts[1];
            String json     = parts[2];
            try {
                String vanilla = YogDataLayout.yogRecipeToVanilla(json, namespace, mc121);
                if (vanilla == null) continue;
                Path target = packRoot
                        .resolve("data").resolve(namespace).resolve(YogDataLayout.recipeDir(mc121))
                        .resolve(name + ".json");
                Files.createDirectories(target.getParent());
                Files.writeString(target, vanilla);
            } catch (IOException e) {
                System.err.println("[yog] failed to write recipe " + namespace + ":" + name + ": " + e);
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
    private static void injectDimensions(Path packRoot, String lines) {
        if (lines == null || lines.isBlank()) return;
        Gson gson = new Gson();
        for (String line : lines.split("\n")) {
            if (line.isBlank()) continue;
            String[] parts = line.split("\t", 2);
            if (parts.length < 2) continue;
            String id = parts[0];
            try {
                int colon = id.indexOf(':');
                String namespace = colon > 0 ? id.substring(0, colon) : DEFAULT_NAMESPACE;
                String name = colon > 0 ? id.substring(colon + 1) : id;

                JsonObject def = gson.fromJson(parts[1], JsonObject.class);
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
                System.err.println("[yog] failed to inject dimension '" + id + "': " + e);
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
