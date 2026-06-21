package dev.yog;

/**
 * Bridge between the Fabric host and the native Yog runtime ({@code libyog_runtime}).
 * Every call into Rust goes through here.
 */
public final class NativeBridge {
    private static boolean loaded = false;

    private NativeBridge() {
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
}
