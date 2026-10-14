package dev.yog;

import dev.yog.util.YogPackStager;
import java.io.IOException;
import java.nio.file.Path;
import java.util.function.Consumer;
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
    /** MC 1.21+ data layout: singular datapack dir names, 1.21 recipe format. */
    private static final boolean MC121_LAYOUT = true;
    /**
     * MC 1.21.1: data pack format 48, resource pack format 34. The same
     * directory backs both pack types, so declare the main format for data
     * and a supported range that covers resources too.
     */
    private static final String PACK_MCMETA =
            "{\"pack\":{\"pack_format\":48,\"supported_formats\":[34,48],\"description\":\"Yog mods\"}}";

    private final PackType type;

    public YogPackProvider(PackType type) {
        this.type = type;
    }

    @Override
    public void loadPacks(Consumer<Pack> adder) {
        Path dir;
        try {
            dir = YogPackStager.stage(FMLPaths.GAMEDIR.get(), "forge", "1.21.1", MC121_LAYOUT, PACK_MCMETA,
                    NativeBridge::nativeContent);
        } catch (IOException e) {
            System.err.println("[yog] failed to stage mod packs: " + e);
            return;
        }
        if (dir == null) {
            return;
        }
        var loc = new PackLocationInfo(
                YogPackStager.PACK_ID,
                Component.literal(YogPackStager.PACK_TITLE),
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
}
