package dev.yog;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.block.Block;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.Registries;
import net.minecraft.registry.RegistryKey;
import net.minecraft.registry.RegistryKeys;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.network.ServerPlayerEntity;
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

    /** Give `count` of `itemId` to the named online player. */
    public static boolean giveItem(String player, String itemId, int count) {
        ServerPlayerEntity p = playerByName(player);
        Identifier id = Identifier.tryParse(itemId);
        if (p == null || id == null || count <= 0 || !Registries.ITEM.containsId(id)) {
            return false;
        }
        Item item = Registries.ITEM.get(id);
        p.giveItemStack(new ItemStack(item, count));
        return true;
    }

    /** Teleport the named online player within their current world. */
    public static boolean teleport(String player, double x, double y, double z) {
        ServerPlayerEntity p = playerByName(player);
        if (p == null) {
            return false;
        }
        p.teleport(p.getServerWorld(), x, y, z, p.getYaw(), p.getPitch());
        return true;
    }

    private static ServerPlayerEntity playerByName(String name) {
        MinecraftServer s = server;
        return s == null ? null : s.getPlayerManager().getPlayer(name);
    }

    private static ServerWorld worldFor(String dimension) {
        MinecraftServer s = server;
        Identifier id = Identifier.tryParse(dimension);
        if (s == null || id == null) {
            return null;
        }
        return s.getWorld(RegistryKey.of(RegistryKeys.WORLD, id));
    }

    /** Load the embedded native runtime and initialise it. Idempotent. */
    public static synchronized void ensureLoaded() {
        if (loaded) {
            return;
        }
        loadEmbeddedRuntime();
        nativeInit(modsDir());
        loaded = true;
    }

    /**
     * Extract the platform's runtime native from inside this jar and load it, so
     * players never deal with a loose .so/.dll. The jar bundles every supported
     * platform under {@code /natives/<os>-<arch>/}.
     */
    private static void loadEmbeddedRuntime() {
        String resource = "/natives/" + platformTag() + "/" + runtimeLibName();
        try (InputStream in = NativeBridge.class.getResourceAsStream(resource)) {
            if (in == null) {
                throw new IllegalStateException("embedded Yog runtime not found: " + resource);
            }
            Path tmp = Files.createTempFile("yog_runtime", "-" + runtimeLibName());
            Files.copy(in, tmp, StandardCopyOption.REPLACE_EXISTING);
            tmp.toFile().deleteOnExit();
            System.load(tmp.toAbsolutePath().toString());
        } catch (IOException e) {
            throw new RuntimeException("failed to load the Yog native runtime", e);
        }
    }

    /** Directory players drop `.yog` mods into: {@code <game dir>/yog-mods}. */
    private static String modsDir() {
        Path dir = FabricLoader.getInstance().getGameDir().resolve("yog-mods");
        try {
            Files.createDirectories(dir);
        } catch (IOException ignored) {
            // best effort; the runtime tolerates a missing directory
        }
        return dir.toAbsolutePath().toString();
    }

    /** e.g. {@code linux-x86_64} — must match the Rust runtime's platform tag. */
    private static String platformTag() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String arch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);
        String osTag = os.contains("win") ? "windows" : os.contains("mac") ? "macos" : "linux";
        String archTag = switch (arch) {
            case "amd64", "x86_64" -> "x86_64";
            case "aarch64", "arm64" -> "aarch64";
            default -> arch;
        };
        return osTag + "-" + archTag;
    }

    private static String runtimeLibName() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("win")) {
            return "yog_runtime.dll";
        }
        return os.contains("mac") ? "libyog_runtime.dylib" : "libyog_runtime.so";
    }

    // --- native entry points implemented in yog-runtime (Rust) ---

    public static native void nativeInit(String modsDir);

    public static native void nativeOnBlockBreak(
            String player, String block, int x, int y, int z);

    public static native void nativeOnChat(String player, String message);

    public static native void nativeOnPlayerJoin(String player, String uuid);

    public static native void nativeOnPlayerLeave(String player, String uuid);

    public static native void nativeOnUseItem(String player, String item);

    public static native void nativeOnTick();

    public static native void nativeOnServerStarted();

    public static native void nativeOnServerStopping();

    /** Names of mod-registered commands, one per line. */
    public static native String nativeCommandNames();

    /** Run a registered command; returns the reply (empty string if none). */
    public static native String nativeOnCommand(String name, String args, String source);
}
