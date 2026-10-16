package dev.yog;

import net.minecraft.client.MinecraftClient;

/** Hot reload's client side (dev builds only, like {@link YogDevReload}). */
public final class YogDevReloadClient {
    private YogDevReloadClient() {}

    /** After a hot reload: the client rereads its resources as F3+T does (textures, models, sounds, lang). */
    public static void reloadResources() {
        MinecraftClient mc = MinecraftClient.getInstance();
        mc.execute(mc::reloadResources);
    }
}
