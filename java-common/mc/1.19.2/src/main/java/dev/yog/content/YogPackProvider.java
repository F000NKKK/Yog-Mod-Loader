package dev.yog.content;

import dev.yog.NativeBridge;
import dev.yog.YogLog;

import dev.yog.util.YogImagePack;
import dev.yog.util.YogPackStager;
import java.io.IOException;
import java.nio.file.Path;
import java.util.function.Consumer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.FolderPackResources;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.RepositorySource;

/**
 * Exposes the {@code assets/} and {@code data/} bundled inside {@code .yog} mods
 * to the game as a single always-on pack — the same way a mod jar's resources
 * are exposed, but for our runtime-loaded mods.
 */
public class YogPackProvider implements RepositorySource {
    /** MC 1.20.x data layout: plural datapack dir names (loot_tables, tags/items, ...). */
    private static final boolean MC121_LAYOUT = false;
    /** The pack's pack.mcmeta, its format the one this game reads packs of its type in. */
    private static final String PACK_MCMETA = "{\"pack\":{\"pack_format\":%d,\"description\":\"Yog mods\"}}";

    private final PackType type;

    public YogPackProvider(PackType type) {
        this.type = type;
    }

    @Override
    public void loadPacks(Consumer<Pack> adder, Pack.PackConstructor constructor) {
        var game = net.minecraft.SharedConstants.getCurrentVersion();
        Path dir;
        try {
            dir = YogPackStager.stage(game.getName(), MC121_LAYOUT, PACK_MCMETA.formatted(type.getVersion(game)),
                    NativeBridge::nativeContent);
        } catch (IOException e) {
            YogLog.warn("failed to stage mod packs: " + e);
            return;
        }
        if (dir == null) {
            return;
        }
        // This version reads every sprite from its own file, not from atlas definitions.
        if (type == PackType.CLIENT_RESOURCES) YogAtlasCut.cut(dir);
        Pack pack = Pack.create(
                YogPackStager.PACK_ID,
                true,
                () -> YogImagePack.wrap(new FolderPackResources(dir.toFile()), PackResources.class),
                constructor,
                Pack.Position.TOP,
                PackSource.DEFAULT);
        if (pack != null) {
            adder.accept(pack);
        }
    }
}
