package dev.yog;

import dev.yog.util.YogPackStager;
import java.io.IOException;
import java.nio.file.Path;
import java.util.function.Consumer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.SharedConstants;
import net.minecraft.network.chat.Component;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.PackSelectionConfig;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PathPackResources;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.RepositorySource;

/**
 * Exposes the {@code assets/} and {@code data/} bundled inside {@code .yog} mods
 * to the game as a single always-on pack — the same way Fabric exposes a mod
 * jar's resources, but for our runtime-loaded mods.
 */
public class YogPackProvider implements RepositorySource {
    /** MC 1.21+ data layout: singular datapack dir names, 1.21 recipe format. */
    private static final boolean MC121_LAYOUT = true;
    /**
     * The same directory backs both pack types, so its format range covers
     * this game's resource pack format and its data pack format (26.x
     * declares a range, {@code min_format}..{@code max_format}).
     */
    private static final String PACK_MCMETA = "{\"pack\":{\"min_format\":"
            + Math.min(SharedConstants.RESOURCE_PACK_FORMAT_MAJOR, SharedConstants.DATA_PACK_FORMAT_MAJOR)
            + ",\"max_format\":"
            + Math.max(SharedConstants.RESOURCE_PACK_FORMAT_MAJOR, SharedConstants.DATA_PACK_FORMAT_MAJOR)
            + ",\"description\":\"Yog mods\"}}";

    private final PackType type;

    public YogPackProvider(PackType type) {
        this.type = type;
    }

    @Override
    public void loadPacks(Consumer<Pack> adder) {
        Path dir;
        try {
            dir = YogPackStager.stage(FabricLoader.getInstance().getGameDir(), "fabric", SharedConstants.getCurrentVersion().name(), MC121_LAYOUT, PACK_MCMETA,
                    NativeBridge::nativeContent);
        } catch (IOException e) {
            YogLog.warn("failed to stage mod packs: " + e);
            return;
        }
        if (dir == null) {
            return;
        }
        PackLocationInfo info = new PackLocationInfo(
                YogPackStager.PACK_ID, Component.literal(YogPackStager.PACK_TITLE), PackSource.DEFAULT, java.util.Optional.empty());
        Pack.ResourcesSupplier factory = new Pack.ResourcesSupplier() {
            @Override
            public PackResources openPrimary(PackLocationInfo packInfo) {
                return new PathPackResources(packInfo, dir);
            }

            @Override
            public PackResources openFull(PackLocationInfo packInfo, Pack.Metadata metadata) {
                return openPrimary(packInfo);
            }
        };
        Pack profile = Pack.readMetaAndCreate(
                info, factory, type,
                new PackSelectionConfig(true, Pack.Position.TOP, false));
        if (profile != null) {
            adder.accept(profile);
        }
    }
}
