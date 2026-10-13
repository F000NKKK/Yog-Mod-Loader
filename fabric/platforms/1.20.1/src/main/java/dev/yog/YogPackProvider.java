package dev.yog;

import dev.yog.util.YogPackStager;
import java.io.IOException;
import java.nio.file.Path;
import java.util.function.Consumer;
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
    /** MC 1.20.x data layout: plural datapack dir names (loot_tables, tags/items, ...). */
    private static final boolean MC121_LAYOUT = false;
    /** pack_format 15 = MC 1.20.1, valid for both resources and data. */
    private static final String PACK_MCMETA = "{\"pack\":{\"pack_format\":15,\"description\":\"Yog mods\"}}";

    private final ResourceType type;

    public YogPackProvider(ResourceType type) {
        this.type = type;
    }

    @Override
    public void register(Consumer<ResourcePackProfile> adder) {
        Path dir;
        try {
            dir = YogPackStager.stage(FabricLoader.getInstance().getGameDir(), MC121_LAYOUT, PACK_MCMETA,
                    NativeBridge::nativeRecipeJsons, NativeBridge::nativeDimensionJsons);
        } catch (IOException e) {
            System.err.println("[yog] failed to stage mod packs: " + e);
            return;
        }
        if (dir == null) {
            return;
        }
        ResourcePackProfile profile = ResourcePackProfile.create(
                YogPackStager.PACK_ID,
                Text.literal(YogPackStager.PACK_TITLE),
                true,
                name -> new DirectoryResourcePack(name, dir, true),
                type,
                ResourcePackProfile.InsertionPosition.TOP,
                ResourcePackSource.NONE);
        if (profile != null) {
            adder.accept(profile);
        }
    }
}
