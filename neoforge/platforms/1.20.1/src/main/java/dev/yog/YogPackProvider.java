package dev.yog;

import dev.yog.util.YogPackStager;
import java.io.IOException;
import java.nio.file.Path;
import java.util.function.Consumer;
import net.minecraft.network.chat.Component;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PathPackResources;
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
    /** MC 1.20.x data layout: plural datapack dir names (loot_tables, tags/items, ...). */
    private static final boolean MC121_LAYOUT = false;
    /** pack_format 15 = MC 1.20.1, valid for both resources and data. */
    private static final String PACK_MCMETA = "{\"pack\":{\"pack_format\":15,\"description\":\"Yog mods\"}}";

    private final PackType type;

    public YogPackProvider(PackType type) {
        this.type = type;
    }

    @Override
    public void loadPacks(Consumer<Pack> adder) {
        Path dir;
        try {
            dir = YogPackStager.stage(FMLPaths.GAMEDIR.get(), "neoforge", "1.20.1", MC121_LAYOUT, PACK_MCMETA,
                    NativeBridge::nativeRecipeJsons, NativeBridge::nativeDimensionJsons);
        } catch (IOException e) {
            System.err.println("[yog] failed to stage mod packs: " + e);
            return;
        }
        if (dir == null) {
            return;
        }
        Pack pack = Pack.readMetaAndCreate(
                YogPackStager.PACK_ID,
                Component.literal(YogPackStager.PACK_TITLE),
                true,
                name -> new PathPackResources(name, dir, true),
                type,
                Pack.Position.TOP,
                PackSource.DEFAULT);
        if (pack != null) {
            adder.accept(pack);
        }
    }
}
