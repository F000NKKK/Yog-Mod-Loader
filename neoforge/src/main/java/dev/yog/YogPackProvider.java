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
        ResourcePackProfile profile = ResourcePackProfile.create(
                "yog_runtime",
                Text.literal("Yog Mods"),
                true,
                name -> new DirectoryResourcePack(name, dir, true),
                type,
                ResourcePackProfile.InsertionPosition.TOP,
                ResourcePackSource.NONE);
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
}
