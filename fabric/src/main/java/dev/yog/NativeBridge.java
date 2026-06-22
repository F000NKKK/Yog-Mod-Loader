package dev.yog;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;
import net.fabricmc.loader.api.FabricLoader;
import java.util.UUID;
import net.fabricmc.fabric.api.networking.v1.PacketByteBufs;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.minecraft.block.Block;
import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.ItemEntity;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.effect.StatusEffect;
import net.minecraft.loot.LootDataKey;
import net.minecraft.loot.LootDataType;
import net.minecraft.loot.LootTable;
import net.minecraft.loot.context.LootContextParameterSet;
import net.minecraft.entity.effect.StatusEffectInstance;
import net.minecraft.item.ItemStack;
import net.minecraft.registry.tag.TagKey;
import net.minecraft.item.Item;
import net.minecraft.network.PacketByteBuf;
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

    /** Send a raw-byte packet to a player on a channel (server -> client). */
    public static boolean sendToPlayer(String player, String channel, byte[] data) {
        ServerPlayerEntity p = playerByName(player);
        Identifier id = Identifier.tryParse(channel);
        if (p == null || id == null) {
            return false;
        }
        PacketByteBuf buf = PacketByteBufs.create();
        buf.writeBytes(data);
        ServerPlayNetworking.send(p, id, buf);
        return true;
    }

    private static ServerPlayerEntity playerByName(String name) {
        MinecraftServer s = server;
        return s == null ? null : s.getPlayerManager().getPlayer(name);
    }

    // --- entity ops (universal, by UUID) ---

    public static boolean entityTeleport(String uuid, double x, double y, double z) {
        Entity e = entityByUuid(uuid);
        if (e == null) {
            return false;
        }
        if (e instanceof ServerPlayerEntity p) {
            p.networkHandler.requestTeleport(x, y, z, p.getYaw(), p.getPitch());
        } else {
            e.teleport(x, y, z);
        }
        return true;
    }

    public static String entityPosition(String uuid) {
        Entity e = entityByUuid(uuid);
        return e == null ? null : e.getX() + "\t" + e.getY() + "\t" + e.getZ();
    }

    public static double entityHealth(String uuid) {
        Entity e = entityByUuid(uuid);
        return e instanceof LivingEntity le ? le.getHealth() : Double.NaN;
    }

    public static boolean entitySetHealth(String uuid, double health) {
        Entity e = entityByUuid(uuid);
        if (e instanceof LivingEntity le) {
            le.setHealth((float) health);
            return true;
        }
        return false;
    }

    /** Game time in ticks since world creation, or Long.MIN_VALUE if dimension unknown. */
    public static long worldTime(String dimension) {
        ServerWorld w = worldFor(dimension);
        return w == null ? Long.MIN_VALUE : w.getTime();
    }

    /** Set the time-of-day; returns false if the dimension is unknown. */
    public static boolean worldSetTime(String dimension, long time) {
        ServerWorld w = worldFor(dimension);
        if (w == null) return false;
        w.setTimeOfDay(time);
        return true;
    }

    /** Whether it is currently raining in the given dimension. */
    public static boolean worldIsRaining(String dimension) {
        ServerWorld w = worldFor(dimension);
        return w != null && w.isRaining();
    }

    /**
     * Start or stop rain. {@code durationTicks == 0} picks a server default.
     * Internally calls {@link net.minecraft.server.world.ServerWorld#setWeather}.
     * Signature: clearDuration, rainDuration, rain, thunder.
     */
    public static boolean worldSetWeather(String dimension, boolean raining, int durationTicks) {
        ServerWorld w = worldFor(dimension);
        if (w == null) return false;
        int dur = durationTicks > 0 ? durationTicks : 6000;
        if (raining) {
            w.setWeather(0, dur, true, false);
        } else {
            w.setWeather(dur, 0, false, false);
        }
        return true;
    }

    public static boolean entityAddEffect(
            String uuid, String effectId, int durationTicks, int amplifier, boolean showParticles) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        Identifier id = Identifier.tryParse(effectId);
        if (id == null || !Registries.STATUS_EFFECT.containsId(id)) return false;
        StatusEffect effect = Registries.STATUS_EFFECT.get(id);
        return le.addStatusEffect(new StatusEffectInstance(effect, durationTicks, amplifier, false, showParticles));
    }

    public static boolean entityRemoveEffect(String uuid, String effectId) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        Identifier id = Identifier.tryParse(effectId);
        if (id == null || !Registries.STATUS_EFFECT.containsId(id)) return false;
        StatusEffect effect = Registries.STATUS_EFFECT.get(id);
        return le.removeStatusEffect(effect);
    }

    public static boolean entityClearEffects(String uuid) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        return le.clearStatusEffects();
    }

    public static boolean dropLoot(String tableId, String dimension, double x, double y, double z) {
        MinecraftServer s = server;
        if (s == null) return false;
        Identifier id = Identifier.tryParse(tableId);
        ServerWorld world = worldFor(dimension);
        if (id == null || world == null) return false;
        LootDataKey<LootTable> key = new LootDataKey<>(LootDataType.LOOT_TABLES, id);
        LootTable table = s.getLootManager().getElement(key);
        if (table == null || table == LootTable.EMPTY) return false;
        LootContextParameterSet params = new LootContextParameterSet(
                world, java.util.Map.of(), java.util.Map.of(), 0.0f);
        java.util.List<ItemStack> stacks = table.generateLoot(params);
        for (ItemStack stack : stacks) {
            world.spawnEntity(new ItemEntity(world, x, y, z, stack));
        }
        return !stacks.isEmpty();
    }

    public static boolean hasItemTag(String itemId, String tagId) {
        Identifier iid = Identifier.tryParse(itemId);
        Identifier tid = Identifier.tryParse(tagId);
        if (iid == null || tid == null || !Registries.ITEM.containsId(iid)) return false;
        TagKey<Item> tag = TagKey.of(RegistryKeys.ITEM, tid);
        return Registries.ITEM.get(iid).getDefaultStack().isIn(tag);
    }

    public static boolean hasBlockTag(String blockId, String tagId) {
        Identifier bid = Identifier.tryParse(blockId);
        Identifier tid = Identifier.tryParse(tagId);
        if (bid == null || tid == null || !Registries.BLOCK.containsId(bid)) return false;
        TagKey<Block> tag = TagKey.of(RegistryKeys.BLOCK, tid);
        return Registries.BLOCK.get(bid).getDefaultState().isIn(tag);
    }

    public static boolean entityKill(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) {
            return false;
        }
        e.kill();
        return true;
    }

    public static String spawnEntity(String typeId, String dimension, double x, double y, double z) {
        ServerWorld w = worldFor(dimension);
        Identifier id = Identifier.tryParse(typeId);
        if (w == null || id == null || !Registries.ENTITY_TYPE.containsId(id)) {
            return null;
        }
        EntityType<?> type = Registries.ENTITY_TYPE.get(id);
        Entity e = type.create(w);
        if (e == null) {
            return null;
        }
        e.refreshPositionAndAngles(x, y, z, e.getYaw(), e.getPitch());
        w.spawnEntity(e);
        return e.getUuidAsString();
    }

    private static Entity entityByUuid(String uuidStr) {
        MinecraftServer s = server;
        if (s == null) {
            return null;
        }
        UUID uuid;
        try {
            uuid = UUID.fromString(uuidStr);
        } catch (IllegalArgumentException ex) {
            return null;
        }
        for (ServerWorld w : s.getWorlds()) {
            Entity e = w.getEntity(uuid);
            if (e != null) {
                return e;
            }
        }
        return null;
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

    public static native void nativeOnUseBlock(
            String player, String block, int x, int y, int z);

    public static native void nativeOnAttackEntity(
            String player, String targetType, String targetUuid);

    public static native void nativeOnEntityDamage(
            String entityType, String uuid, float amount, String source);

    public static native void nativeOnEntityDeath(String entityType, String uuid, String source);

    public static native void nativeOnTick();

    public static native void nativeOnServerStarted();

    public static native void nativeOnServerStopping();

    /** Names of mod-registered commands, one per line. */
    public static native String nativeCommandNames();

    /** Run a registered command; returns the reply (empty string if none). */
    public static native String nativeOnCommand(String name, String args, String source, String uuid);

    /** Declared custom items as `id\tmax_stack` lines. */
    public static native String nativeItemDefs();

    /** Declared custom blocks as `id\thardness\tresistance` lines. */
    public static native String nativeBlockDefs();

    // (no native entry points needed for #4 — all calls are Rust→Java via JNI)

    public static native void nativeOnPacket(String channel, String player, byte[] payload);

    public static native void nativeOnClientPacket(String channel, byte[] payload);

    /** Server-receiver packet channels, one per line. */
    public static native String nativePacketChannels();

    /** Client-receiver packet channels, one per line. */
    public static native String nativeClientPacketChannels();
}
