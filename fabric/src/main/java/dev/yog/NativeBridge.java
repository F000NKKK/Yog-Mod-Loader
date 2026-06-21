package dev.yog;

import net.minecraft.block.Block;
import net.minecraft.registry.Registries;
import net.minecraft.registry.RegistryKey;
import net.minecraft.registry.RegistryKeys;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.world.ServerWorld;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import net.minecraft.world.World;

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

    /** Registry id of the block at (x,y,z) in `dimension`, or null. */
    public static String getBlock(String dimension, int x, int y, int z) {
        ServerWorld w = worldFor(dimension);
        if (w == null) {
            return null;
        }
        Block block = w.getBlockState(new BlockPos(x, y, z)).getBlock();
        return Registries.BLOCK.getId(block).toString();
    }

    /**
     * Set the block at (x,y,z) in `dimension` to `blockId`. Returns whether it
     * was applied. Must run on the server thread (Yog calls it from event
     * handlers, which already do).
     */
    public static boolean setBlock(String dimension, int x, int y, int z, String blockId) {
        ServerWorld w = worldFor(dimension);
        Identifier id = Identifier.tryParse(blockId);
        if (w == null || id == null || !Registries.BLOCK.containsId(id)) {
            return false;
        }
        Block block = Registries.BLOCK.get(id);
        return w.setBlockState(new BlockPos(x, y, z), block.getDefaultState());
    }

    private static ServerWorld worldFor(String dimension) {
        MinecraftServer s = server;
        Identifier id = Identifier.tryParse(dimension);
        if (s == null || id == null) {
            return null;
        }
        return s.getWorld(RegistryKey.of(RegistryKeys.WORLD, id));
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

    /** Names of mod-registered commands, one per line. */
    public static native String nativeCommandNames();

    /** Run a registered command; returns the reply (empty string if none). */
    public static native String nativeOnCommand(String name, String args, String source);
}
