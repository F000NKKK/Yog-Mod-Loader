package dev.yog;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Comparator;
import java.util.function.Consumer;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.resource.DirectoryResourcePack;
import net.minecraft.resource.ResourcePack;
import net.minecraft.resource.ResourcePackInfo;
import net.minecraft.resource.ResourcePackPosition;
import net.minecraft.resource.ResourcePackProfile;
import net.minecraft.resource.ResourcePackProvider;
import net.minecraft.resource.ResourcePackSource;
import net.minecraft.resource.ResourceType;
import net.minecraft.text.Text;

/**
 * Exposes the {@code assets/} and {@code data/} bundled inside {@code .yog} mods
 * to the game as a single always-on pack — the same way Fabric exposes a mod
 * jar's resources, but for our runtime-loaded mods.
 */
public class YogPackProvider implements ResourcePackProvider {
    private final ResourceType type;

    public YogPackProvider(ResourceType type) {
        this.type = type;
    }

    @Override
    public void register(Consumer<ResourcePackProfile> adder) {
        Path dir;
        try {
            dir = extract();
        } catch (IOException e) {
            System.err.println("[yog] failed to stage mod packs: " + e);
            return;
        }
        if (dir == null) {
            return;
        }
        ResourcePackInfo info = new ResourcePackInfo(
                "yog_runtime", Text.literal("Yog Mods"), ResourcePackSource.NONE, java.util.Optional.empty());
        ResourcePackProfile.PackFactory factory = new ResourcePackProfile.PackFactory() {
            @Override
            public ResourcePack open(ResourcePackInfo packInfo) {
                return new DirectoryResourcePack(packInfo, dir);
            }

            @Override
            public ResourcePack openWithOverlays(ResourcePackInfo packInfo, ResourcePackProfile.Metadata metadata) {
                return open(packInfo);
            }
        };
        ResourcePackProfile profile = ResourcePackProfile.create(
                info, factory, type,
                new ResourcePackPosition(true, ResourcePackProfile.InsertionPosition.TOP, false));
        if (profile != null) {
            adder.accept(profile);
        }
    }

    /** Merge every .yog's assets/ and data/ into one pack directory. */
    private static synchronized Path extract() throws IOException {
        Path mods = FabricLoader.getInstance().getGameDir().resolve("yog-mods");
        Path out = FabricLoader.getInstance().getGameDir().resolve(".yog-pack");
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
                        Path target = out.resolve(n).normalize();
                        if (!target.startsWith(out)) {
                            continue; // zip-slip guard
                        }
                        Files.createDirectories(target.getParent());
                        try (InputStream in = zip.getInputStream(e)) {
                            Files.copy(in, target, StandardCopyOption.REPLACE_EXISTING);
                        }
                    }
                }
            }
        }

        // Inject mod-registered recipes as JSON files inside the data pack.
        injectRecipes(out);

        // Inject mod-declared dimensions as vanilla dimension_type/dimension JSON.
        injectDimensions(out);

        // pack_format 15 = MC 1.20.1, valid for both resources and data.
        Files.writeString(out.resolve("pack.mcmeta"),
                "{\"pack\":{\"pack_format\":15,\"description\":\"Yog mods\"}}");
        return out;
    }

    private static void injectRecipes(Path packRoot) {
        String lines = NativeBridge.nativeRecipeJsons();
        if (lines == null || lines.isBlank()) return;
        for (String line : lines.split("\n")) {
            if (line.isBlank()) continue;
            String[] parts = line.split("\t", 3);
            if (parts.length < 3) continue;
            String namespace = parts[0];
            String name     = parts[1];
            String json     = parts[2];
            try {
                Path target = packRoot
                        .resolve("data").resolve(namespace).resolve("recipes")
                        .resolve(name + ".json");
                Files.createDirectories(target.getParent());
                Files.writeString(target, json);
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
    private static void injectDimensions(Path packRoot) {
        String lines = NativeBridge.nativeDimensionJsons();
        if (lines == null || lines.isBlank()) return;
        com.google.gson.Gson gson = new com.google.gson.Gson();
        for (String line : lines.split("\n")) {
            if (line.isBlank()) continue;
            String[] parts = line.split("\t", 2);
            if (parts.length < 2) continue;
            String id = parts[0];
            try {
                int colon = id.indexOf(':');
                String namespace = colon > 0 ? id.substring(0, colon) : "yog";
                String name = colon > 0 ? id.substring(colon + 1) : id;

                com.google.gson.JsonObject def = gson.fromJson(parts[1], com.google.gson.JsonObject.class);
                com.google.gson.JsonObject dt = def.has("dimension_type")
                        ? def.getAsJsonObject("dimension_type") : new com.google.gson.JsonObject();

                com.google.gson.JsonObject dimensionType = new com.google.gson.JsonObject();
                dimensionType.addProperty("ultrawarm", jbool(dt, "ultrawarm", false));
                dimensionType.addProperty("natural", jbool(dt, "natural", true));
                dimensionType.addProperty("coordinate_scale", jnum(dt, "coordinate_scale", 1.0));
                dimensionType.addProperty("has_skylight", jbool(dt, "has_sky_light", true));
                dimensionType.addProperty("has_ceiling", jbool(dt, "has_ceiling", false));
                dimensionType.addProperty("ambient_light", jnum(dt, "ambient_light", 0.0));
                dimensionType.addProperty("bed_works", !jbool(dt, "beds_explode", false));
                dimensionType.addProperty("respawn_anchor_works", !jbool(dt, "respawn_anchors_explode", false));
                dimensionType.addProperty("min_y", (int) jnum(dt, "min_y", -64));
                dimensionType.addProperty("height", (int) jnum(dt, "height", 384));
                dimensionType.addProperty("logical_height", (int) jnum(dt, "logical_height", 384));
                dimensionType.addProperty("infiniburn", "#minecraft:infiniburn_overworld");
                dimensionType.addProperty("effects", jstr(dt, "effects", "minecraft:overworld"));
                dimensionType.addProperty("piglin_safe", jbool(dt, "piglin_safe", false));
                dimensionType.addProperty("has_raids", true);
                dimensionType.addProperty("monster_spawn_block_light_limit", 0);
                dimensionType.addProperty("monster_spawn_light_level", 7);

                com.google.gson.JsonObject biomeSource = new com.google.gson.JsonObject();
                biomeSource.addProperty("type", "minecraft:fixed");
                biomeSource.addProperty("biome", "minecraft:plains");
                com.google.gson.JsonObject generator = new com.google.gson.JsonObject();
                if (def.has("generator_type") && !def.get("generator_type").isJsonNull()) {
                    generator.addProperty("type", "yog:callback_generator");
                    generator.addProperty("generator_type_id", def.get("generator_type").getAsString());
                    generator.addProperty("min_y", (int) jnum(dt, "min_y", -64));
                    generator.addProperty("height", (int) jnum(dt, "height", 384));
                    generator.add("biome_source", biomeSource);
                } else {
                    generator.addProperty("type", "minecraft:noise");
                    generator.addProperty("settings", "minecraft:overworld");
                    generator.add("biome_source", biomeSource);
                }

                com.google.gson.JsonObject dimension = new com.google.gson.JsonObject();
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

    private static boolean jbool(com.google.gson.JsonObject o, String key, boolean def) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsBoolean() : def;
    }

    private static double jnum(com.google.gson.JsonObject o, String key, double def) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsDouble() : def;
    }

    private static String jstr(com.google.gson.JsonObject o, String key, String def) {
        return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : def;
    }
}
