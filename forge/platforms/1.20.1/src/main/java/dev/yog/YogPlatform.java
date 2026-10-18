package dev.yog;

import dev.yog.util.YogModInfo;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;

/**
 * What the shared host code ({@code java-common/mc/<version>}) asks of the
 * loader: Forge's answers. Every loader's host has a class of this name
 * with these methods.
 */
public final class YogPlatform {
    private YogPlatform() {}

    /** The loader's name, as the pack stager and overlays call it. */
    public static final String LOADER = "forge";

    public static Path gameDir() {
        return net.minecraftforge.fml.loading.FMLPaths.GAMEDIR.get();
    }

    public static boolean isModLoaded(String id) {
        return net.minecraftforge.fml.ModList.get() != null && net.minecraftforge.fml.ModList.get().isLoaded(id);
    }

    public static List<YogModInfo> mods() {
        List<YogModInfo> out = new ArrayList<>();
        for (var m : net.minecraftforge.fml.ModList.get().getMods()) {
            out.add(new YogModInfo(m.getModId(), m.getDisplayName(), m.getVersion().toString(), "", m.getDescription()));
        }
        return out;
    }

    /** Whether {@code e} is a part of a bigger entity (the loader's multipart entities). */
    public static boolean isPart(Entity e) {
        return e instanceof net.minecraftforge.entity.PartEntity<?>;
    }

    public static void sendToClient(ServerPlayer p, String channel, byte[] data) {
        YogNetworkBridge.sendToPlayer(p, channel, data);
    }

    /** Raw bytes to everyone who sees {@code e}, and to it if a player. */
    public static void sendToTracking(Entity e, String channel, byte[] data) {
        YogNetworkBridge.sendToTracking(e, channel, data);
    }

    public static void sendToServer(String channel, byte[] data) {
        YogNetworkBridge.sendToServer(channel, data);
    }
}
