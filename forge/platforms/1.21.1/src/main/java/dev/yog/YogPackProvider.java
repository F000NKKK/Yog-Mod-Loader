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
import net.minecraft.network.chat.Component;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackSelectionConfig;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PathPackResources;
import net.minecraft.server.packs.PackResources;

import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.RepositorySource;
import net.minecraftforge.fml.loading.FMLPaths;

/**
 * Exposes the {@code assets/} and {@code data/} bundled inside {@code .yog} mods
 * to the game as a single always-on pack — the same way a mod jar's resources
 * are exposed, but for our runtime-loaded mods.
 */
public class YogPackProvider implements RepositorySource {
    private final PackType type;

    public YogPackProvider(PackType type) {
        this.type = type;
    }

    @Override
    public void loadPacks(Consumer<Pack> adder) {
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
        var loc = new PackLocationInfo(
                "yog_runtime",
                Component.literal("Yog Mods"),
                PackSource.DEFAULT,
                java.util.Optional.empty());
        Pack pack = Pack.readMetaAndCreate(
                loc,
                new Pack.ResourcesSupplier() {
            @Override public PackResources openPrimary(PackLocationInfo loc) { return new PathPackResources(loc, dir); }
            @Override public PackResources openFull(PackLocationInfo loc, Pack.Metadata meta) { return new PathPackResources(loc, dir); }
        },
                type,
                new PackSelectionConfig(true, Pack.Position.TOP, false));
        if (pack != null) {
            adder.accept(pack);
        }
    }

    /** Merge every .yog's assets/ and data/ into one pack directory. */
    private static synchronized Path extract() throws IOException {
        Path game = FMLPaths.GAMEDIR.get();
        Path mods = game.resolve("yog-mods");
        Path out  = game.resolve(".yog-pack");
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

        // pack_format 34 = MC 1.21.1, valid for both resources and data.
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
