package dev.yog.client;

import net.minecraft.client.Minecraft;

/** Hot reload's client side (dev builds only, like {@link YogDevReload}). */
public final class YogDevReloadClient {
    private YogDevReloadClient() {}

    /** After a hot reload: the client rereads its resources as F3+T does (textures, models, sounds, lang). */
    public static void reloadResources() {
        Minecraft mc = Minecraft.getInstance();
        mc.execute(mc::reloadResourcePacks);
    }
}
