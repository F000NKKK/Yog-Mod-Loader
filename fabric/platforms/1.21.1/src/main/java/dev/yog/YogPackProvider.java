package dev.yog;

import dev.yog.util.YogPackStager;
import java.io.IOException;
import java.nio.file.Path;
import java.util.function.Consumer;
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
    /** MC 1.21+ data layout: singular datapack dir names, 1.21 recipe format. */
    private static final boolean MC121_LAYOUT = true;
    /**
     * MC 1.21.1: data pack format 48, resource pack format 34. The same
     * directory backs both pack types, so declare the main format for data
     * and a supported range that covers resources too.
     */
    private static final String PACK_MCMETA =
            "{\"pack\":{\"pack_format\":48,\"supported_formats\":[34,48],\"description\":\"Yog mods\"}}";

    private final ResourceType type;

    public YogPackProvider(ResourceType type) {
        this.type = type;
    }

    @Override
    public void register(Consumer<ResourcePackProfile> adder) {
        Path dir;
        try {
            dir = YogPackStager.stage(FabricLoader.getInstance().getGameDir(), "fabric", "1.21.1", MC121_LAYOUT, PACK_MCMETA,
                    NativeBridge::nativeRecipeJsons, NativeBridge::nativeDimensionJsons);
        } catch (IOException e) {
            System.err.println("[yog] failed to stage mod packs: " + e);
            return;
        }
        if (dir == null) {
            return;
        }
        ResourcePackInfo info = new ResourcePackInfo(
                YogPackStager.PACK_ID, Text.literal(YogPackStager.PACK_TITLE), ResourcePackSource.NONE, java.util.Optional.empty());
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
}
