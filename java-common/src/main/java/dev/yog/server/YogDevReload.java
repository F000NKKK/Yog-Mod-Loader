package dev.yog.server;

import dev.yog.NativeBridge;
import dev.yog.YogLog;

import net.minecraft.server.MinecraftServer;

/**
 * Hot reload's server side (dev builds only: release jars leave this class
 * out, as the runtime leaves out what calls it).
 */
public final class YogDevReload {
    private YogDevReload() {}

    /**
     * After a hot reload: the server rereads its data as {@code /reload}
     * does — packs scanned again (the mods' new {@code .yog}s staged), then
     * recipes, tags, loot. Returns once it's done.
     */
    public static void reloadData() {
        dev.yog.data.YogContent.invalidate();
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return;
        java.util.concurrent.CompletableFuture<Void> done = new java.util.concurrent.CompletableFuture<>();
        s.execute(() -> {
            s.getPackRepository().reload();
            s.reloadResources(s.getPackRepository().getSelectedIds()).whenComplete((v, e) -> done.complete(null));
        });
        try {
            done.get(120, java.util.concurrent.TimeUnit.SECONDS);
        } catch (Exception e) {
            YogLog.warn("reloading data after a hot reload: " + e);
        }
    }
}
