package dev.yog;

import net.minecraft.server.MinecraftServer;
import net.minecraft.text.Text;

/**
 * Bridge between the Fabric host and the native Yog runtime ({@code libyog_runtime}).
 * Calls into Rust go through the {@code native} methods; calls back from Rust into
 * the game (e.g. {@link #broadcast}) are static methods invoked over JNI.
 */
public final class NativeBridge {
    private static boolean loaded = false;
    private static volatile MinecraftServer server;

    private NativeBridge() {
    }

    /** Remember the running server so Rust-initiated actions can reach it. */
    public static void setServer(MinecraftServer s) {
        server = s;
    }

    // --- callbacks FROM Rust (invoked via JNI by yog-runtime) ---

    /** Broadcast a chat message to all players. Safe to call off-thread. */
    public static void broadcast(String message) {
        MinecraftServer s = server;
        if (s != null) {
            s.execute(() -> s.getPlayerManager().broadcast(Text.literal(message), false));
        }
    }

    /** Load the native runtime and initialise it. Idempotent. */
    public static synchronized void ensureLoaded() {
        if (loaded) {
            return;
        }
        // Resolves libyog_runtime.so / .dylib / yog_runtime.dll on java.library.path.
        System.loadLibrary("yog_runtime");
        nativeInit();
        loaded = true;
    }

    // --- native entry points implemented in yog-runtime (Rust) ---

    public static native void nativeInit();

    public static native void nativeOnBlockBreak(
            String player, String block, int x, int y, int z);

    public static native void nativeOnChat(String player, String message);

    public static native void nativeOnPlayerJoin(String player, String uuid);

    public static native void nativeOnPlayerLeave(String player, String uuid);

    public static native void nativeOnServerStarted();

    public static native void nativeOnServerStopping();
}
