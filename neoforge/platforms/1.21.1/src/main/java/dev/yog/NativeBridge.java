package dev.yog;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;
import java.util.UUID;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.world.effect.MobEffect;
// LootDataId removed in 1.21.1 — using RegistryKey<LootTable>
// LootDataType removed in 1.21.1
import net.minecraft.world.level.storage.loot.LootTable;
import net.minecraft.world.level.storage.loot.LootParams;
import net.minecraft.world.level.storage.loot.parameters.LootContextParamSets;
import net.minecraft.world.level.storage.loot.parameters.LootContextParams;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.item.ItemStack;
import net.minecraft.network.protocol.game.ClientboundSetActionBarTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetSubtitleTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitlesAnimationPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitleTextPacket;
import net.minecraft.tags.TagKey;
import net.minecraft.world.item.Item;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.core.Registry;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.sounds.SoundSource;
import net.minecraft.network.chat.Component;
import net.minecraft.core.BlockPos;
import org.lwjgl.glfw.GLFW;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.neoforged.fml.loading.FMLPaths;
import java.util.Map;

/**
 * Bridge between the NeoForge host and the native Yog runtime ({@code libyog_runtime}).
 * Calls into Rust go through the {@code native} methods; calls back from Rust into
 * the game are static methods invoked over JNI.
 */
public final class NativeBridge {
    private static boolean loaded = false;
    private static volatile MinecraftServer server;

    private NativeBridge() {}

    /** Remember the running server so Rust-initiated actions can reach it. */
    public static void setServer(MinecraftServer s) { server = s; }

    /** Broadcast a chat message to all players. */
    public static void broadcast(String message) {
        MinecraftServer s = server;
        if (s != null) {
            s.execute(() -> s.getPlayerList().broadcastSystemMessage(Component.literal(message), false));
        }
    }

    /** Registry id of the block at (x,y,z) in `dimension`, or null. */
    public static String getBlock(String dimension, int x, int y, int z) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return null;
        Block block = w.getBlockState(new BlockPos(x, y, z)).getBlock();
        return BuiltInRegistries.BLOCK.getKey(block).toString();
    }

    public static boolean setBlock(String dimension, int x, int y, int z, String blockId) {
        ServerLevel w = worldFor(dimension);
        ResourceLocation id = ResourceLocation.tryParse(blockId);
        if (w == null || id == null || !BuiltInRegistries.BLOCK.containsKey(id)) return false;
        Block block = BuiltInRegistries.BLOCK.get(id);
        return w.setBlockAndUpdate(new BlockPos(x, y, z), block.defaultBlockState());
    }

    public static boolean giveItem(String player, String itemId, int count) {
        ServerPlayer p = playerByName(player);
        ResourceLocation id = ResourceLocation.tryParse(itemId);
        if (p == null || id == null || count <= 0) return false;
        if (!BuiltInRegistries.ITEM.containsKey(id)) return false;
        Item item = BuiltInRegistries.ITEM.get(id);
        p.addItem(new ItemStack(item, count));
        return true;
    }

    public static boolean teleport(String player, double x, double y, double z) {
        ServerPlayer p = playerByName(player);
        if (p == null) return false;
        p.teleportTo(p.serverLevel(), x, y, z, p.getYRot(), p.getXRot());
        return true;
    }

    public static boolean sendToPlayer(String player, String channel, byte[] data) {
        ServerPlayer p = playerByName(player);
        if (p == null) return false;
        return YogNetworkBridge.sendToPlayer(p, channel, data);
    }

    private static ServerPlayer playerByName(String name) {
        MinecraftServer s = server;
        return s == null ? null : s.getPlayerList().getPlayerByName(name);
    }

    // ── Entity ops ──────────────────────────────────────────────────────

    public static boolean entityTeleport(String uuid, double x, double y, double z) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        if (e.level() instanceof ServerLevel sl) e.teleportTo(sl, x, y, z, java.util.Set.of(), e.getYRot(), e.getXRot());
        return true;
    }

    public static String entityPosition(String uuid) {
        Entity e = entityByUuid(uuid);
        return e == null ? null : e.getX() + "\t" + e.getY() + "\t" + e.getZ();
    }

    /** Yaw and pitch (degrees) of an entity by UUID: "yaw\tpitch", or null. */
    public static String entityRotation(String uuid) {
        Entity e = entityByUuid(uuid);
        return e == null ? null : e.getYRot() + "\t" + e.getXRot();
    }

    public static double entityHealth(String uuid) {
        Entity e = entityByUuid(uuid);
        return e instanceof LivingEntity le ? le.getHealth() : Double.NaN;
    }

    public static boolean entitySetHealth(String uuid, double health) {
        Entity e = entityByUuid(uuid);
        if (e instanceof LivingEntity le) { le.setHealth((float) health); return true; }
        return false;
    }

    public static long worldTime(String dimension) {
        ServerLevel w = worldFor(dimension);
        return w == null ? Long.MIN_VALUE : w.getDayTime();
    }

    public static boolean worldSetTime(String dimension, long time) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        w.setDayTime(time);
        return true;
    }

    public static boolean worldIsRaining(String dimension) {
        ServerLevel w = worldFor(dimension);
        return w != null && w.isRaining();
    }

    public static boolean worldSetWeather(String dimension, boolean raining, int durationTicks) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        int dur = durationTicks > 0 ? durationTicks : 6000;
        w.setWeatherParameters(raining ? 0 : dur, raining ? dur : 0, false, false);
        return true;
    }

    public static String entityVelocity(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) return null;
        net.minecraft.world.phys.Vec3 v = e.getDeltaMovement();
        return v.x + "\t" + v.y + "\t" + v.z;
    }

    public static boolean entitySetVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        e.setDeltaMovement(vx, vy, vz);
        e.hasImpulse = true;
        return true;
    }

    public static boolean entityAddVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        e.addDeltaMovement(new net.minecraft.world.phys.Vec3(vx, vy, vz));
        e.hasImpulse = true;
        return true;
    }

    public static int scoreboardGet(String objective, String player) { return 0; }
    public static boolean scoreboardSet(String objective, String player, int score) { return false; }
    public static int scoreboardAdd(String objective, String player, int delta) { return 0; }

    public static boolean playSound(String dimension, double x, double y, double z, String soundId, float volume, float pitch) {
        ServerLevel w = worldFor(dimension);
        ResourceLocation id = ResourceLocation.tryParse(soundId);
        if (w == null || id == null) return false;
        w.playSound(null, x, y, z, net.minecraft.core.registries.BuiltInRegistries.SOUND_EVENT.get(id), SoundSource.MASTER, volume, pitch);
        return true;
    }

    public static boolean sendTitle(String playerName, String title, String subtitle, int fadein, int stay, int fadeout) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        p.connection.send(new ClientboundSetTitlesAnimationPacket(fadein, stay, fadeout));
        if (!title.isEmpty()) p.connection.send(new ClientboundSetTitleTextPacket(Component.literal(title)));
        if (!subtitle.isEmpty()) p.connection.send(new ClientboundSetSubtitleTextPacket(Component.literal(subtitle)));
        return true;
    }

    public static boolean sendActionbar(String playerName, String message) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        p.connection.send(new ClientboundSetActionBarTextPacket(Component.literal(message)));
        return true;
    }

    public static boolean kickPlayer(String playerName, String reason) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        p.connection.disconnect(Component.literal(reason));
        return true;
    }

    public static boolean setGamemode(String playerName, String gamemode) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        GameType mode = switch (gamemode.toLowerCase(Locale.ROOT)) {
            case "survival", "s", "0" -> GameType.SURVIVAL;
            case "creative", "c", "1" -> GameType.CREATIVE;
            case "adventure", "a", "2" -> GameType.ADVENTURE;
            case "spectator", "sp", "3" -> GameType.SPECTATOR;
            default -> null;
        };
        if (mode == null) return false;
        p.setGameMode(mode);
        return true;
    }

    public static String onlinePlayers() {
        MinecraftServer s = server;
        if (s == null) return null;
        StringBuilder sb = new StringBuilder();
        for (ServerPlayer p : s.getPlayerList().getPlayers()) {
            if (sb.length() > 0) sb.append('\n');
            sb.append(p.getName().getString());
        }
        return sb.toString();
    }

    public static String getBlockNbt(String dimension, int x, int y, int z) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return null;
        BlockEntity be = w.getBlockEntity(new BlockPos(x, y, z));
        if (be == null) return null;
        CompoundTag nbt = be.saveWithFullMetadata(w.registryAccess());
        return nbt.toString();
    }

    public static boolean setBlockNbt(String dimension, int x, int y, int z, String snbt) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        BlockEntity be = w.getBlockEntity(new BlockPos(x, y, z));
        if (be == null) return false;
        try { CompoundTag nbt = TagParser.parseTag(snbt); be.loadWithComponents(nbt, w.registryAccess()); be.setChanged(); return true; }
        catch (Exception e) { return false; }
    }

    // yog-inventory (phase 2 stub): no BlockEntity backing exists yet
    // (see rust/crates/yog-inventory/DESIGN.md, phase 3) — always report
    // "no such inventory" until that lands.
    public static String getInventorySlot(String dimension, int x, int y, int z, int slot) {
        return null;
    }

    public static boolean setInventorySlot(String dimension, int x, int y, int z, int slot, String itemId, int count) {
        return false;
    }

    public static String playerInventory(String playerName) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return null;
        Inventory inv = p.getInventory();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < inv.items.size(); i++) {
            ItemStack stack = inv.items.get(i);
            if (!stack.isEmpty()) {
                if (sb.length() > 0) sb.append('\n');
                sb.append(i).append('\t').append(BuiltInRegistries.ITEM.getKey(stack.getItem())).append('\t').append(stack.getCount());
            }
        }
        return sb.toString();
    }

    public static boolean playerSetSlot(String playerName, int slot, String itemId, int count) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        Inventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.items.size()) return false;
        if (count <= 0) { inv.items.set(slot, ItemStack.EMPTY); return true; }
        ResourceLocation id = ResourceLocation.tryParse(itemId);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        inv.items.set(slot, new ItemStack(BuiltInRegistries.ITEM.get(id), count));
        return true;
    }

    public static boolean teleportToDim(String playerName, String dimension, double x, double y, double z) {
        ServerPlayer p = playerByName(playerName);
        ServerLevel w = worldFor(dimension);
        if (p == null || w == null) return false;
        p.teleportTo(w, x, y, z, p.getYRot(), p.getXRot());
        return true;
    }

    public static boolean entityTeleportToDim(String uuid, String dimension, double x, double y, double z) {
        Entity e = entityByUuid(uuid);
        ServerLevel w = worldFor(dimension);
        if (e == null || w == null) return false;
        e.teleportTo(w, x, y, z, java.util.Set.of(), e.getYRot(), e.getXRot());
        return true;
    }

    public static String gameDir() {
        MinecraftServer s = server;
        if (s == null) return net.neoforged.fml.loading.FMLPaths.GAMEDIR.get().toAbsolutePath().toString();
        return s.getServerDirectory().toAbsolutePath().toString();
    }

    public static boolean entityAddEffect(String uuid, String effectId, int durationTicks, int amplifier, boolean showParticles) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        ResourceLocation id = ResourceLocation.tryParse(effectId);
        if (id == null || !BuiltInRegistries.MOB_EFFECT.containsKey(id)) return false;
        MobEffect effect = BuiltInRegistries.MOB_EFFECT.get(id);
        return le.addEffect(new MobEffectInstance(BuiltInRegistries.MOB_EFFECT.wrapAsHolder(effect), durationTicks, amplifier, false, showParticles));
    }

    public static boolean entityKill(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        e.kill();
        return true;
    }

    public static String entityGetNbt(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) return null;
        CompoundTag nbt = new CompoundTag();
        e.save(nbt);
        return nbt.toString();
    }

    public static boolean entitySetNbt(String uuid, String snbt) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        try { CompoundTag nbt = TagParser.parseTag(snbt); e.load(nbt); return true; }
        catch (Exception ex) { return false; }
    }

    public static int worldEntityCount(String dimension, String entityTypeId) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return -1;
        ResourceLocation id = ResourceLocation.tryParse(entityTypeId);
        if (id == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(id)) return -1;
        EntityType<?> targetType = BuiltInRegistries.ENTITY_TYPE.get(id);
        int count = 0;
        for (Entity e : w.getEntities().getAll()) {
            if (e.getType() == targetType) count++;
        }
        return count;
    }

    public static String spawnEntity(String typeId, String dimension, double x, double y, double z) {
        ServerLevel w = worldFor(dimension);
        ResourceLocation id = ResourceLocation.tryParse(typeId);
        if (w == null || id == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(id)) return null;
        EntityType<?> type = BuiltInRegistries.ENTITY_TYPE.get(id);
        Entity e = type.create(w);
        if (e == null) return null;
        e.moveTo(x, y, z, e.getYRot(), e.getXRot());
        w.addFreshEntity(e);
        return e.getStringUUID();
    }

    private static Entity entityByUuid(String uuidStr) {
        MinecraftServer s = server;
        if (s == null) return null;
        for (ServerLevel w : s.getAllLevels()) {
            Entity e = w.getEntity(UUID.fromString(uuidStr));
            if (e != null) return e;
        }
        return null;
    }

    private static ServerLevel worldFor(String dimension) {
        MinecraftServer s = server;
        ResourceLocation id = ResourceLocation.tryParse(dimension);
        if (s == null || id == null) return null;
        return s.getLevel(ResourceKey.create(Registries.DIMENSION, id));
    }

    /** Load the embedded native runtime and initialise it. Idempotent. */
    public static synchronized void ensureLoaded() {
        if (loaded) return;
        loadEmbeddedRuntime();
        nativeInit(modsDir());
        loaded = true;
    }

    private static void loadEmbeddedRuntime() {
        String resource = "/natives/" + platformTag() + "/" + runtimeLibName();
        try (InputStream in = NativeBridge.class.getResourceAsStream(resource)) {
            if (in == null) throw new IllegalStateException("embedded Yog runtime not found: " + resource);
            Path tmp = Files.createTempFile("yog_runtime", "-" + runtimeLibName());
            Files.copy(in, tmp, StandardCopyOption.REPLACE_EXISTING);
            tmp.toFile().deleteOnExit();
            System.load(tmp.toAbsolutePath().toString());
        } catch (IOException e) {
            throw new RuntimeException("failed to load the Yog native runtime", e);
        }
    }

    private static String modsDir() {
        Path dir = FMLPaths.GAMEDIR.get().resolve("yog-mods");
        try { Files.createDirectories(dir); } catch (IOException ignored) {}
        return dir.toAbsolutePath().toString();
    }

    private static String platformTag() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String arch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);
        String osTag = os.contains("win") ? "windows" : os.contains("mac") ? "macos" : "linux";
        String archTag = switch (arch) { case "amd64", "x86_64" -> "x86_64"; case "aarch64", "arm64" -> "aarch64"; default -> arch; };
        return osTag + "-" + archTag;
    }

    private static String runtimeLibName() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("win")) return "yog_runtime.dll";
        return os.contains("mac") ? "libyog_runtime.dylib" : "libyog_runtime.so";
    }

    // ── native entry points ─────────────────────────────────────────────
    // (same as Fabric — JNI stubs implemented in Rust)

    public static native void nativeInit(String modsDir);
    public static native void nativeOnBlockBreak(String player, String block, int x, int y, int z);
    public static native void nativeOnChat(String player, String message);
    public static native void nativeOnPlayerJoin(String player, String uuid);
    public static native void nativeOnPlayerLeave(String player, String uuid);
    public static native void nativeOnUseItem(String player, String item, boolean sneaking);
    public static native void nativeOnUseBlock(String player, String block, int x, int y, int z);

    /** Cancel-check for item use (before). Returns true = allow, false = cancel. */
    public static native boolean nativeOnUseItemPre(String player, String item, boolean sneaking);

    /** Cancel-check for block use / right-click (before). Returns true = allow, false = cancel. */
    public static native boolean nativeOnUseBlockPre(
            String player, String block, int x, int y, int z);
    public static native void nativeOnAttackEntity(String player, String targetType, String targetUuid);
    public static native void nativeOnEntityDamage(String entityType, String uuid, float amount, String source);
    public static native void nativeOnEntityDeath(String entityType, String uuid, String source);
    public static native void nativeOnTick();
    public static native void nativeOnServerStarted(String worldDir);
    public static native void nativeOnServerStopping();
    public static native String nativeCommandNames();
    public static native String nativeTypedCommandSchemas();
    public static native boolean nativeOnBlockBreakPre(String player, String block, int x, int y, int z);
    public static native boolean nativeOnChatPre(String player, String message);
    public static native String nativeRecipeJsons();
    public static native String nativeBookJson(String bookId);
    public static native void nativeUIShow(String uiId, String parentId, boolean modal, boolean pauseGame, int screenW, int screenH);
    public static native void nativeUIHide(String uiId);
    public static native boolean nativeUIClick(String uiId, float mx, float my, int button);

    /** Mouse wheel over a Yog UI screen; dy is the vertical scroll amount. */
    public static native void nativeUIScroll(String uiId, float dy);

    /** Open a registered Yog UI (called from Rust; schedules onto the render thread). */
    public static void openUI(String uiId, boolean modal, boolean pause) {
        try {
            net.minecraft.client.Minecraft mc = net.minecraft.client.Minecraft.getInstance();
            mc.execute(() -> YogUIScreen.open(uiId, modal, pause));
        } catch (Throwable t) {
            // dedicated server — no client classes; ignore
        }
    }

    /** Mouse dragged over a Yog UI screen (any button held). */
    public static native void nativeUIDrag(String uiId, float mx, float my);

    /** Mouse button released over a Yog UI screen. */
    public static native void nativeUIRelease(String uiId, float mx, float my);
    public static native void nativeUIKey(String uiId, int keyCode, int scanCode, int modifiers, int action);
    public static native void nativeUIRender(String uiId, int screenW, int screenH);
    public static native boolean nativeIsUIActive(String uiId);
    public static native String nativeMenuEntries();
    public static native String nativeOnCommand(String name, String args, String source, String uuid);
    public static native String nativeItemDefs();
    public static native String nativeBlockDefs();

    /** Declared inventory-backed screens (yog-inventory) — see DESIGN.md. */
    public static native String nativeInventoryDefs();
    public static native void nativeOnPacket(String channel, String player, byte[] payload);
    public static native void nativeOnClientPacket(String channel, byte[] payload);
    public static native String nativePacketChannels();
    public static native String nativeClientPacketChannels();
    public static native void nativeOnEntitySpawn(String entityType, String uuid, String dimension);
    public static native boolean nativeOnEntitySpawnPre(String entityType, String uuid, String dimension);
    public static native boolean nativeOnEntityDamagePre(String entityType, String uuid, float amount, String source);
    public static native boolean nativeOnPlaceBlockPre(String player, String block, int x, int y, int z);
    public static native void nativeOnPlaceBlock(String player, String block, int x, int y, int z);
    public static native boolean nativeOnPlayerDeathPre(String player, String uuid, String source);
    public static native void nativeOnPlayerDeath(String player, String uuid, String source);
    public static native void nativeOnPlayerRespawn(String player, String uuid, boolean atAnchor);
    public static native void nativeOnAdvancement(String player, String uuid, String advancement);
    public static native boolean nativeOnEntityInteractPre(String player, String playerUuid, String entityType, String entityUuid, String hand);
    public static native void nativeOnEntityInteract(String player, String playerUuid, String entityType, String entityUuid, String hand);
    public static native void nativeOnItemCraft(String player, String playerUuid, String resultItem, int resultCount);
    public static native boolean nativeOnExplosionPre(String dimension, double x, double y, double z, float power, String causeUuid);
    public static native void nativeOnExplosion(String dimension, double x, double y, double z, float power, String causeUuid);
    public static native boolean nativeOnItemPickupPre(String player, String playerUuid, String itemId, int itemCount, String entityUuid);
    public static native void nativeOnItemPickup(String player, String playerUuid, String itemId, int itemCount, String entityUuid);
    public static native void nativeOnPlayerMove(String player, String playerUuid, double x, double y, double z, float yaw, float pitch);
    public static native boolean nativeOnContainerOpenPre(String player, String playerUuid);
    public static native void nativeOnContainerOpen(String player, String playerUuid, String containerType);
    public static native void nativeOnContainerClose(String player, String playerUuid);
    public static native boolean nativeOnProjectileHitPre(String projectileType, String projectileUuid, String shooterUuid, String hitType, String hitEntityUuid, double x, double y, double z, String dimension);
    public static native void nativeOnProjectileHit(String projectileType, String projectileUuid, String shooterUuid, String hitType, String hitEntityUuid, double x, double y, double z, String dimension);
    public static native void nativeOnClientTick();
    public static native void nativeGlInit();
    public static native void nativeOnHudRender(float deltaTick, int screenW, int screenH, float scaleFactor, float playerX, float playerY, float playerZ);
    public static native void nativeOnWorldRender(float deltaTick, int screenW, int screenH, float scaleFactor, float[] viewProj, float camX, float camY, float camZ, float playerX, float playerY, float playerZ);
    public static native void nativeOnScreenOpen(String screenClass);
    public static native void nativeOnScreenClose(String screenClass);
    public static native boolean nativeOnKeyPress(int keyCode, int scanCode, int action, int modifiers);

    /** Resolve an OpenGL function pointer by name — used by yog-runtime via JNI. */
    public static long glProcAddress(String name) {
        return GLFW.glfwGetProcAddress(name);
    }

    // ── platform mod listing (consumed by the runtime's mods_list ABI) ──────

    /** TSV lines: id \t name \t version \t authors \t description. */
    public static String listPlatformMods() {
        StringBuilder sb = new StringBuilder();
        for (var info : net.neoforged.fml.ModList.get().getMods()) {
            if (sb.length() > 0) sb.append('\n');
            sb.append(tsvField(info.getModId())).append('\t')
              .append(tsvField(info.getDisplayName())).append('\t')
              .append(tsvField(info.getVersion().toString())).append('\t')
              .append('\t')
              .append(tsvField(info.getDescription()));
        }
        return sb.toString();
    }

    private static String tsvField(String s) {
        return s == null ? "" : s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
    }

    public static YogInventoryMenu activeInventoryMenu;

    public static int getSlotCount() {
        return activeInventoryMenu != null ? activeInventoryMenu.slots.size() : 0;
    }

    public static String getSlotItem(int index) {
        if (activeInventoryMenu == null || index < 0 || index >= activeInventoryMenu.slots.size()) return null;
        net.minecraft.world.inventory.Slot slot = activeInventoryMenu.slots.get(index);
        net.minecraft.world.item.ItemStack stack = slot.getItem();
        if (stack.isEmpty()) return null;
        return net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(stack.getItem()) + "\t" + stack.getCount();
    }

    public static String getSlotPos(int index) {
        if (activeInventoryMenu == null || index < 0 || index >= activeInventoryMenu.slots.size()) return null;
        net.minecraft.world.inventory.Slot slot = activeInventoryMenu.slots.get(index);
        return slot.x + "\t" + slot.y;
    }
}
