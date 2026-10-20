package dev.yog;

import dev.yog.data.YogContent;
import static dev.yog.util.YogProps.flag01;
import static dev.yog.util.YogProps.tsvField;

import dev.yog.util.YogEntityFlags;
import dev.yog.util.YogNativeRuntime;
import java.util.Locale;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.commands.arguments.ParticleArgument;
import net.minecraft.commands.arguments.blocks.BlockInput;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.ClientboundSetActionBarTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetSubtitleTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitleTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitlesAnimationPacket;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.bossevents.CustomBossEvent;
import net.minecraft.server.bossevents.CustomBossEvents;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.world.level.storage.TagValueOutput;
import net.minecraft.world.level.storage.loot.parameters.LootContextParamSets;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.network.chat.ComponentSerialization;
import com.mojang.serialization.JsonOps;
import com.google.gson.JsonParser;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.sounds.SoundSource;
import net.minecraft.tags.TagKey;
import net.minecraft.world.BossEvent;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.BoneMealItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.LevelEvent;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.storage.loot.LootParams;
import net.minecraft.world.level.storage.loot.LootTable;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.shapes.CollisionContext;
import org.lwjgl.glfw.GLFW;
import com.mojang.brigadier.StringReader;

/**
 * Bridge between the host (any loader: {@link YogPlatform}) and the native Yog runtime ({@code libyog_runtime}).
 * Calls into Rust go through the {@code native} methods; calls back from Rust into
 * the game (e.g. {@link #broadcast}) are static methods invoked over JNI.
 */
public final class NativeBridge {
    private static boolean loaded = false;
    private static volatile MinecraftServer server;

    private NativeBridge() {
    }

    /** Remember the running server so Rust-initiated actions can reach it. */
    /** The running server, or null. */
    static MinecraftServer currentServer() {
        return server;
    }

    public static void setServer(MinecraftServer s) {
        server = s;
    }

    // --- callbacks FROM Rust (invoked via JNI by yog-runtime) ---

    /** Broadcast a chat message to all players. Safe to call off-thread. */
    public static void broadcast(String message) {
        MinecraftServer s = server;
        if (s != null) {
            s.execute(() -> s.getPlayerList().broadcastSystemMessage(Component.literal(message), false));
        }
    }

    /** Registry id of the block at (x,y,z) in `dimension`, or null. */
    public static String getBlock(String dimension, int x, int y, int z) {
        ServerLevel w = worldFor(dimension);
        if (w == null) {
            return null;
        }
        Block block = w.getBlockState(new BlockPos(x, y, z)).getBlock();
        return BuiltInRegistries.BLOCK.getKey(block).toString();
    }

    /**
     * Set the block at (x,y,z) in `dimension` to `blockId`. Returns whether it
     * was applied. Must run on the server thread (Yog calls it from event
     * handlers, which already do).
     */
    public static boolean setBlock(String dimension, int x, int y, int z, String blockId) {
        ServerLevel w = worldFor(dimension);
        if (w != null && blockId != null && (blockId.indexOf('[') >= 0 || blockId.indexOf('{') >= 0)) {
            // Full /setblock syntax: id[prop=value,...]{nbt}
            try {
                BlockStateParser.BlockResult r =
                        BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, blockId, true);
                return new BlockInput(r.blockState(), r.properties().keySet(), r.nbt())
                        .place(w, new BlockPos(x, y, z), 3);
            } catch (Throwable ignored) {
                // fall back to the plain id lookup below
            }
        }
        Identifier id = Identifier.tryParse(blockId);
        if (w == null || id == null || !BuiltInRegistries.BLOCK.containsKey(id)) {
            return false;
        }
        Block block = BuiltInRegistries.BLOCK.getValue(id);
        return w.setBlockAndUpdate(new BlockPos(x, y, z), block.defaultBlockState());
    }

    /** Give `count` of `itemId` to the named online player. */
    public static boolean giveItem(String player, String itemId, int count) {
        ServerPlayer p = playerByName(player);
        Identifier id = Identifier.tryParse(itemId);
        if (p == null) { YogLog.warn("giveItem: player not found: " + player); return false; }
        if (id == null) { YogLog.warn("giveItem: bad id: " + itemId); return false; }
        if (count <= 0) { YogLog.warn("giveItem: bad count: " + count); return false; }
        if (!BuiltInRegistries.ITEM.containsKey(id)) {
            YogLog.info("giveItem: item not registered: " + itemId);
            // List what IS registered for debugging
            if (itemId.startsWith("yog:")) {
                YogLog.warn("known yog: items: " +
                    BuiltInRegistries.ITEM.keySet().stream()
                        .filter(i -> i.getNamespace().equals("yog"))
                        .map(Identifier::toString).toList());
            }
            return false;
        }
        Item item = BuiltInRegistries.ITEM.getValue(id);
        p.addItem(new ItemStack(item, count));
        return true;
    }

    /** Teleport the named online player within their current world. */
    /** Whether the online player carries command tag `tag` (`/tag`; saved with the player's own data). */
    public static boolean playerHasTag(String player, String tag) {
        ServerPlayer p = playerByName(player);
        return p != null && p.entityTags().contains(tag);
    }

    /** Adds command tag `tag` to the online player; false when offline or already tagged. */
    public static boolean addPlayerTag(String player, String tag) {
        ServerPlayer p = playerByName(player);
        return p != null && p.addTag(tag);
    }

    public static boolean teleport(String player, double x, double y, double z) {
        ServerPlayer p = playerByName(player);
        if (p == null) {
            return false;
        }
        p.teleportTo(p.level(), x, y, z, java.util.Set.of(), p.getYRot(), p.getXRot(), true);
        return true;
    }

    /** Send a raw-byte packet to a player on a channel (server -> client). */
    /** Send a raw-byte packet to everyone who sees entity {@code uuid}, and to it if a player. */
    public static boolean sendToTracking(String uuid, String channel, byte[] data) {
        Entity e = entityByUuid(uuid);
        if (e == null || Identifier.tryParse(channel) == null) return false;
        YogPlatform.sendToTracking(e, channel, data);
        return true;
    }

    public static boolean sendToPlayer(String player, String channel, byte[] data) {
        ServerPlayer p = playerByName(player);
        Identifier id = Identifier.tryParse(channel);
        if (p == null || id == null) {
            return false;
        }
        YogPlatform.sendToClient(p, channel, data);
        return true;
    }

    private static ServerPlayer playerByName(String name) {
        MinecraftServer s = server;
        return s == null ? null : s.getPlayerList().getPlayerByName(name);
    }

    // --- entity ops (universal, by UUID) ---

    public static boolean entityTeleport(String uuid, double x, double y, double z) {
        Entity e = entityByUuid(uuid);
        if (e == null) {
            return false;
        }
        if (e instanceof ServerPlayer p) {
            p.connection.teleport(x, y, z, p.getYRot(), p.getXRot());
        } else if (e.level() instanceof ServerLevel sw) {
            e.teleportTo(sw, x, y, z, java.util.Set.of(), e.getYRot(), e.getXRot(), true);
        }
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
        if (e instanceof LivingEntity le) {
            le.setHealth((float) health);
            return true;
        }
        return false;
    }

    /** Game time in ticks since world creation, or Long.MIN_VALUE if dimension unknown. */
    public static long worldTime(String dimension) {
        ServerLevel w = worldFor(dimension);
        return w == null ? Long.MIN_VALUE : w.getGameTime();
    }

    /** Set the time-of-day; returns false if the dimension is unknown. */
    public static boolean worldSetTime(String dimension, long time) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        // Since 26.1 time is a clock's, shared by the dimensions that keep it.
        var clock = w.dimensionType().defaultClock();
        if (clock.isEmpty()) return false;
        w.getServer().clockManager().setTotalTicks(clock.get(), time);
        return true;
    }

    /** Whether it is currently raining in the given dimension. */
    public static boolean worldIsRaining(String dimension) {
        ServerLevel w = worldFor(dimension);
        return w != null && w.isRaining();
    }

    /** Rain / clear spell length when the caller passes no duration (5 minutes). */
    private static final int DEFAULT_WEATHER_TICKS = 6000;

    /**
     * Start or stop rain. {@code durationTicks == 0} picks a server default.
     * Internally calls {@link MinecraftServer#setWeatherParameters} (weather is the server's since 26.1).
     * Signature: clearDuration, rainDuration, rain, thunder.
     */
    public static boolean worldSetWeather(String dimension, boolean raining, int durationTicks) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        int dur = durationTicks > 0 ? durationTicks : DEFAULT_WEATHER_TICKS;
        if (raining) {
            w.getServer().setWeatherParameters(0, dur, true, false);
        } else {
            w.getServer().setWeatherParameters(dur, 0, false, false);
        }
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
        e.hurtMarked = true;
        return true;
    }

    public static boolean entityAddVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        e.push(vx, vy, vz);
        e.hurtMarked = true;
        return true;
    }

    /** Score of {@code player} on {@code objective}, or {@code Integer.MIN_VALUE} if unknown. */
    public static int scoreboardGet(String objective, String player) {
        MinecraftServer s = server;
        if (s == null) return Integer.MIN_VALUE;
        net.minecraft.world.scores.Scoreboard sb = s.getScoreboard();
        net.minecraft.world.scores.Objective obj = sb.getObjective(objective);
        if (obj == null) return Integer.MIN_VALUE;
        net.minecraft.world.scores.ReadOnlyScoreInfo sc =
                sb.getPlayerScoreInfo(net.minecraft.world.scores.ScoreHolder.forNameOnly(player), obj);
        return sc == null ? 0 : sc.value();
    }

    public static boolean scoreboardSet(String objective, String player, int score) {
        MinecraftServer s = server;
        if (s == null) return false;
        net.minecraft.world.scores.Scoreboard sb = s.getScoreboard();
        net.minecraft.world.scores.Objective obj = sb.getObjective(objective);
        if (obj == null) return false;
        sb.getOrCreatePlayerScore(net.minecraft.world.scores.ScoreHolder.forNameOnly(player), obj).set(score);
        return true;
    }

    /** Returns new score, or {@code Integer.MIN_VALUE} if objective unknown. */
    public static int scoreboardAdd(String objective, String player, int delta) {
        MinecraftServer s = server;
        if (s == null) return Integer.MIN_VALUE;
        net.minecraft.world.scores.Scoreboard sb = s.getScoreboard();
        net.minecraft.world.scores.Objective obj = sb.getObjective(objective);
        if (obj == null) return Integer.MIN_VALUE;
        net.minecraft.world.scores.ScoreAccess score =
                sb.getOrCreatePlayerScore(net.minecraft.world.scores.ScoreHolder.forNameOnly(player), obj);
        score.add(delta);
        return score.get();
    }

    public static boolean playSound(
            String dimension, double x, double y, double z,
            String soundId, float volume, float pitch) {
        ServerLevel w = worldFor(dimension);
        Identifier id = Identifier.tryParse(soundId);
        if (w == null || id == null) return false;
        w.playSound(null, x, y, z, SoundEvent.createVariableRangeEvent(id), SoundSource.MASTER, volume, pitch);
        return true;
    }

    public static boolean playSoundToPlayer(String playerName, String soundId, float volume, float pitch) {
        ServerPlayer p = playerByName(playerName);
        Identifier id = Identifier.tryParse(soundId);
        if (p == null || id == null) return false;
        p.level().playSound(
                null, p.getX(), p.getY(), p.getZ(),
                SoundEvent.createVariableRangeEvent(id), SoundSource.MASTER, volume, pitch);
        return true;
    }

    public static boolean sendTitle(
            String playerName, String title, String subtitle,
            int fadein, int stay, int fadeout) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        p.connection.send(new ClientboundSetTitlesAnimationPacket(fadein, stay, fadeout));
        if (!title.isEmpty()) {
            p.connection.send(new ClientboundSetTitleTextPacket(Component.literal(title)));
        }
        if (!subtitle.isEmpty()) {
            p.connection.send(new ClientboundSetSubtitleTextPacket(Component.literal(subtitle)));
        }
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

    // ── boss bar ────────────────────────────────────────────────────────────

    public static boolean bossbarCreate(String id, String title, String color, String style) {
        MinecraftServer s = server;
        if (s == null) return false;
        Identifier bid = Identifier.tryParse(id);
        if (bid == null) return false;
        CustomBossEvents mgr = s.getCustomBossEvents();
        if (mgr.get(bid) != null) return false;
        CustomBossEvent bar = mgr.create(s.overworld().getRandom(), bid, Component.literal(title));
        bar.setColor(parseBossBarColor(color));
        bar.setOverlay(parseBossBarStyle(style));
        return true;
    }

    public static boolean bossbarRemove(String id) {
        MinecraftServer s = server;
        if (s == null) return false;
        Identifier bid = Identifier.tryParse(id);
        if (bid == null) return false;
        CustomBossEvents mgr = s.getCustomBossEvents();
        CustomBossEvent bar = mgr.get(bid);
        if (bar == null) return false;
        mgr.remove(bar);
        return true;
    }

    public static boolean bossbarSetTitle(String id, String title) {
        CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setName(Component.literal(title));
        return true;
    }

    public static boolean bossbarSetProgress(String id, float progress) {
        CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setProgress(Math.max(0f, Math.min(1f, progress)));
        return true;
    }

    public static boolean bossbarSetColor(String id, String color) {
        CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setColor(parseBossBarColor(color));
        return true;
    }

    public static boolean bossbarAddPlayer(String id, String playerName) {
        CustomBossEvent bar = getBossBar(id);
        ServerPlayer p = playerByName(playerName);
        if (bar == null || p == null) return false;
        bar.addPlayer(p);
        return true;
    }

    public static boolean bossbarRemovePlayer(String id, String playerName) {
        CustomBossEvent bar = getBossBar(id);
        ServerPlayer p = playerByName(playerName);
        if (bar == null || p == null) return false;
        bar.removePlayer(p);
        return true;
    }

    public static boolean bossbarSetVisible(String id, boolean visible) {
        CustomBossEvent bar = getBossBar(id);
        if (bar == null) return false;
        bar.setVisible(visible);
        return true;
    }

    private static CustomBossEvent getBossBar(String id) {
        MinecraftServer s = server;
        if (s == null) return null;
        Identifier bid = Identifier.tryParse(id);
        return bid == null ? null : s.getCustomBossEvents().get(bid);
    }

    private static BossEvent.BossBarColor parseBossBarColor(String color) {
        return switch (color.toLowerCase(Locale.ROOT)) {
            case "pink"   -> BossEvent.BossBarColor.PINK;
            case "blue"   -> BossEvent.BossBarColor.BLUE;
            case "red"    -> BossEvent.BossBarColor.RED;
            case "green"  -> BossEvent.BossBarColor.GREEN;
            case "yellow" -> BossEvent.BossBarColor.YELLOW;
            case "purple" -> BossEvent.BossBarColor.PURPLE;
            default       -> BossEvent.BossBarColor.WHITE;
        };
    }

    private static BossEvent.BossBarOverlay parseBossBarStyle(String style) {
        return switch (style.toLowerCase(Locale.ROOT)) {
            case "notched_6"  -> BossEvent.BossBarOverlay.NOTCHED_6;
            case "notched_10" -> BossEvent.BossBarOverlay.NOTCHED_10;
            case "notched_12" -> BossEvent.BossBarOverlay.NOTCHED_12;
            case "notched_20" -> BossEvent.BossBarOverlay.NOTCHED_20;
            default           -> BossEvent.BossBarOverlay.PROGRESS;
        };
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
        CompoundTag nbt = be.saveWithoutMetadata(w.registryAccess());
        return nbt.toString();
    }

    public static boolean setBlockNbt(String dimension, int x, int y, int z, String snbt) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        BlockPos pos = new BlockPos(x, y, z);
        BlockEntity be = w.getBlockEntity(pos);
        if (be == null) return false;
        try {
            CompoundTag nbt = TagParser.parseCompoundFully(snbt);
            be.loadWithComponents(TagValueInput.create(ProblemReporter.DISCARDING, w.registryAccess(), nbt));
            be.setChanged();
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    /** The block container at (dimension, x, y, z) — a chest, a Yog inventory block — or null. */
    private static net.minecraft.world.Container containerAt(String dimension, int x, int y, int z) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return null;
        return w.getBlockEntity(new net.minecraft.core.BlockPos(x, y, z)) instanceof net.minecraft.world.Container c ? c : null;
    }

    /** A container slot: "item\tcount\tsnbt", or null when empty or not a container. */
    public static String getInventorySlot(String dimension, int x, int y, int z, int slot) {
        net.minecraft.world.Container inv = containerAt(dimension, x, y, z);
        if (inv == null || slot < 0 || slot >= inv.getContainerSize()) return null;
        var stack = inv.getItem(slot);
        if (stack.isEmpty()) return null;
        return BuiltInRegistries.ITEM.getKey(stack.getItem()) + "\t" + stack.getCount() + "\t" + stackSnbt(stack);
    }

    /**
     * Sets a container slot. {@code itemId} may carry the item's SNBT after a
     * tab ("mymod:blueprint\t{...}"); count 0 empties the slot.
     */
    public static boolean setInventorySlot(String dimension, int x, int y, int z, int slot, String itemId, int count) {
        net.minecraft.world.Container inv = containerAt(dimension, x, y, z);
        if (inv == null || slot < 0 || slot >= inv.getContainerSize()) return false;
        if (count <= 0) {
            inv.setItem(slot, net.minecraft.world.item.ItemStack.EMPTY);
            inv.setChanged();
            return true;
        }
        String[] parts = itemId.split("\t", 2);
        String item = parts[0];
        String snbt = parts.length > 1 ? parts[1] : "";
        Identifier id = Identifier.tryParse(item);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        var stack = new net.minecraft.world.item.ItemStack(BuiltInRegistries.ITEM.getValue(id), count);
        if (!snbt.isEmpty()) {
            try {
                stack.set(net.minecraft.core.component.DataComponents.CUSTOM_DATA, net.minecraft.world.item.component.CustomData.of(TagParser.parseCompoundFully(snbt)));
            } catch (Exception e) {
                YogLog.warn("setInventorySlot: bad SNBT for " + item + ": " + e.getMessage());
                return false;
            }
        }
        inv.setItem(slot, stack);
        inv.setChanged();
        return true;
    }

    public static String playerInventory(String playerName) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return null;
        Inventory inv = p.getInventory();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < inv.getContainerSize(); i++) {
            net.minecraft.world.item.ItemStack stack = inv.getItem(i);
            if (!stack.isEmpty()) {
                if (sb.length() > 0) sb.append('\n');
                sb.append(i).append('\t')
                  .append(BuiltInRegistries.ITEM.getKey(stack.getItem())).append('\t')
                  .append(stack.getCount());
            }
        }
        return sb.toString();
    }

    public static boolean playerSetSlot(String playerName, int slot, String itemId, int count) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        Inventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.getContainerSize()) return false;
        if (count <= 0) {
            inv.setItem(slot, net.minecraft.world.item.ItemStack.EMPTY);
            return true;
        }
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        inv.setItem(slot, new net.minecraft.world.item.ItemStack(BuiltInRegistries.ITEM.getValue(id), count));
        return true;
    }

    public static boolean teleportToDim(String playerName, String dimension, double x, double y, double z) {
        ServerPlayer p = playerByName(playerName);
        ServerLevel w = worldFor(dimension);
        if (p == null) { YogLog.warn("teleportToDim: player not found: " + playerName); return false; }
        if (w == null) { YogLog.warn("teleportToDim: no dimension " + dimension); return false; }
        p.teleportTo(w, x, y, z, java.util.Set.of(), p.getYRot(), p.getXRot(), true);
        return true;
    }

    public static boolean entityTeleportToDim(String uuid, String dimension, double x, double y, double z) {
        Entity e = entityByUuid(uuid);
        ServerLevel w = worldFor(dimension);
        if (e == null || w == null) return false;
        if (e instanceof ServerPlayer p) {
            p.teleportTo(w, x, y, z, java.util.Set.of(), p.getYRot(), p.getXRot(), true);
        } else {
            e.teleportTo(w, x, y, z, java.util.Set.of(), e.getYRot(), e.getXRot(), true);
        }
        return true;
    }

    public static String gameDir() {
        MinecraftServer s = server;
        if (s == null) return null;
        return s.getServerDirectory().toAbsolutePath().toString();
    }

    /** The directory of the world being played (its save): where mods keep what belongs to it. */
    public static String worldDir() {
        MinecraftServer s = server;
        if (s == null) return null;
        return s.getWorldPath(net.minecraft.world.level.storage.LevelResource.ROOT).toAbsolutePath().normalize().toString();
    }

    /** Awards advancement {@code id} to the player {@code uuid}: every criterion it has left. */
    public static boolean advancementGrant(String uuid, String id) {
        MinecraftServer s = server;
        if (s == null || id == null || !(entityByUuid(uuid) instanceof net.minecraft.server.level.ServerPlayer p)) return false;
        net.minecraft.advancements.AdvancementHolder adv = s.getAdvancements().get(net.minecraft.resources.Identifier.tryParse(id));
        if (adv == null) return false;
        var progress = p.getAdvancements().getOrStartProgress(adv);
        java.util.List<String> left = new java.util.ArrayList<>();
        progress.getRemainingCriteria().forEach(left::add);
        for (String c : left) p.getAdvancements().award(adv, c);
        return true;
    }

    /** Whether the player {@code uuid} has advancement {@code id}. */
    public static boolean advancementDone(String uuid, String id) {
        MinecraftServer s = server;
        if (s == null || id == null || !(entityByUuid(uuid) instanceof net.minecraft.server.level.ServerPlayer p)) return false;
        net.minecraft.advancements.AdvancementHolder adv = s.getAdvancements().get(net.minecraft.resources.Identifier.tryParse(id));
        return adv != null && p.getAdvancements().getOrStartProgress(adv).isDone();
    }

    public static boolean entityAddEffect(
            String uuid, String effectId, int durationTicks, int amplifier, boolean showParticles) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        Identifier id = Identifier.tryParse(effectId);
        if (id == null || !BuiltInRegistries.MOB_EFFECT.containsKey(id)) return false;
        MobEffect effect = BuiltInRegistries.MOB_EFFECT.getValue(id);
        if (effect == null) return false;
        return le.addEffect(new MobEffectInstance(
                BuiltInRegistries.MOB_EFFECT.wrapAsHolder(effect), durationTicks, amplifier, false, showParticles));
    }

    public static boolean entityRemoveEffect(String uuid, String effectId) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        Identifier id = Identifier.tryParse(effectId);
        if (id == null || !BuiltInRegistries.MOB_EFFECT.containsKey(id)) return false;
        MobEffect effect = BuiltInRegistries.MOB_EFFECT.getValue(id);
        if (effect == null) return false;
        return le.removeEffect(BuiltInRegistries.MOB_EFFECT.wrapAsHolder(effect));
    }

    public static boolean entityClearEffects(String uuid) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        return le.removeAllEffects();
    }

    public static boolean dropLoot(String tableId, String dimension, double x, double y, double z) {
        MinecraftServer s = server;
        if (s == null) return false;
        Identifier id = Identifier.tryParse(tableId);
        ServerLevel world = worldFor(dimension);
        if (id == null || world == null) return false;
        LootTable table = s.reloadableRegistries().getLootTable(
                net.minecraft.resources.ResourceKey.create(net.minecraft.core.registries.Registries.LOOT_TABLE, id));
        if (table == null || table == LootTable.EMPTY) return false;
        LootParams params = new LootParams.Builder(world).create(LootContextParamSets.EMPTY);
        java.util.List<ItemStack> stacks = table.getRandomItems(params);
        for (ItemStack stack : stacks) {
            world.addFreshEntity(new ItemEntity(world, x, y, z, stack));
        }
        return !stacks.isEmpty();
    }

    public static boolean hasItemTag(String itemId, String tagId) {
        Identifier iid = Identifier.tryParse(itemId);
        Identifier tid = Identifier.tryParse(tagId);
        if (iid == null || tid == null || !BuiltInRegistries.ITEM.containsKey(iid)) return false;
        TagKey<Item> tag = TagKey.create(Registries.ITEM, tid);
        return BuiltInRegistries.ITEM.getValue(iid).getDefaultInstance().is(tag);
    }

    public static boolean hasBlockTag(String blockId, String tagId) {
        Identifier bid = Identifier.tryParse(blockId);
        Identifier tid = Identifier.tryParse(tagId);
        if (bid == null || tid == null || !BuiltInRegistries.BLOCK.containsKey(bid)) return false;
        TagKey<Block> tag = TagKey.create(Registries.BLOCK, tid);
        return BuiltInRegistries.BLOCK.getValue(bid).defaultBlockState().is(tag);
    }

    public static boolean entityKill(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) {
            return false;
        }
        if (!(e.level() instanceof ServerLevel w)) return false;
        e.kill(w);
        return true;
    }

    public static String entityGetNbt(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) return null;
        TagValueOutput out = TagValueOutput.createWithContext(ProblemReporter.DISCARDING, e.registryAccess());
        e.saveWithoutId(out);
        return out.buildResult().toString();
    }

    public static boolean entitySetNbt(String uuid, String snbt) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        try {
            CompoundTag nbt = TagParser.parseCompoundFully(snbt);
            e.load(TagValueInput.create(ProblemReporter.DISCARDING, e.registryAccess(), nbt));
            return true;
        } catch (Exception ex) {
            return false;
        }
    }

    public static boolean spawnParticles(
            String dimension, double x, double y, double z,
            String particleTypeId, int count,
            double dx, double dy, double dz, double speed) {
        try {
            ServerLevel w = worldFor(dimension);
            if (w == null) return false;
            if (particleTypeId.indexOf(' ') >= 0 || particleTypeId.indexOf('{') >= 0) {
                // Particle with options, /particle syntax (e.g. "minecraft:dust{color:[1,0,0],scale:1}")
                net.minecraft.core.particles.ParticleOptions parsed = ParticleArgument.readParticle(
                        new StringReader(particleTypeId), w.registryAccess());
                w.sendParticles(parsed, x, y, z, count, dx, dy, dz, speed);
                return true;
            }
            Identifier id = Identifier.tryParse(particleTypeId);
            if (id == null) return false;
            net.minecraft.core.particles.ParticleType<?> type = BuiltInRegistries.PARTICLE_TYPE.getValue(id);
            if (!(type instanceof net.minecraft.core.particles.ParticleOptions effect)) return false;
            w.sendParticles(effect, x, y, z, count, dx, dy, dz, speed);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    public static double entityAttributeGet(String uuid, String attributeId) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return Double.NaN;
        Identifier id = Identifier.tryParse(attributeId);
        if (id == null || !BuiltInRegistries.ATTRIBUTE.containsKey(id)) return Double.NaN;
        Attribute attr = BuiltInRegistries.ATTRIBUTE.getValue(id);
        if (attr == null) return Double.NaN;
        AttributeInstance inst = le.getAttribute(BuiltInRegistries.ATTRIBUTE.wrapAsHolder(attr));
        return inst == null ? Double.NaN : inst.getBaseValue();
    }

    /** An attribute's value: its base changed by every modifier (effects, equipment). */
    public static double entityAttributeValue(String uuid, String attributeId) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return Double.NaN;
        Identifier id = Identifier.tryParse(attributeId);
        if (id == null || !BuiltInRegistries.ATTRIBUTE.containsKey(id)) return Double.NaN;
        Attribute attr = BuiltInRegistries.ATTRIBUTE.getValue(id);
        if (attr == null) return Double.NaN;
        AttributeInstance inst = le.getAttribute(BuiltInRegistries.ATTRIBUTE.wrapAsHolder(attr));
        return inst == null ? Double.NaN : inst.getValue();
    }

    public static boolean entityAttributeSet(String uuid, String attributeId, double value) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        Identifier id = Identifier.tryParse(attributeId);
        if (id == null || !BuiltInRegistries.ATTRIBUTE.containsKey(id)) return false;
        Attribute attr = BuiltInRegistries.ATTRIBUTE.getValue(id);
        if (attr == null) return false;
        AttributeInstance inst = le.getAttribute(BuiltInRegistries.ATTRIBUTE.wrapAsHolder(attr));
        if (inst == null) return false;
        inst.setBaseValue(value);
        return true;
    }

    // ── held item NBT (ABI minor 11) ─────────────────────────────────────────

    /** SNBT of the item in the player's main hand, or null if offline / holding air. */
    public static String getHeldItemNbt(String playerName) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return null;
        net.minecraft.world.item.ItemStack stack = p.getMainHandItem();
        if (stack.isEmpty()) return null;
        return stack.getOrDefault(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                net.minecraft.world.item.component.CustomData.EMPTY).copyTag().toString();
    }

    /** Merge snbt into the NBT of the item in the player's main hand.
     *  Returns false if the player is offline or holding air. */
    public static boolean setHeldItemNbt(String playerName, String snbt) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        net.minecraft.world.item.ItemStack stack = p.getMainHandItem();
        if (stack.isEmpty()) return false;
        try {
            CompoundTag nbt = TagParser.parseCompoundFully(snbt);
            stack.set(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                    net.minecraft.world.item.component.CustomData.of(nbt));
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    // ── item stack query (ABI minor 12) ──────────────────────────────────────

    /** SNBT of the item in the player's off hand, or null if offline / holding air. */
    public static String getOffhandItemNbt(String playerName) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return null;
        net.minecraft.world.item.ItemStack stack = p.getOffhandItem();
        if (stack.isEmpty()) return null;
        return stack.getOrDefault(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                net.minecraft.world.item.component.CustomData.EMPTY).copyTag().toString();
    }

    /** Merge snbt into the NBT of the player's off-hand item. */
    public static boolean setOffhandItemNbt(String playerName, String snbt) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        net.minecraft.world.item.ItemStack stack = p.getOffhandItem();
        if (stack.isEmpty()) return false;
        try {
            CompoundTag nbt = TagParser.parseCompoundFully(snbt);
            stack.set(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                    net.minecraft.world.item.component.CustomData.of(nbt));
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    /** Full item stack at inventory slot: "item_id\tcount\tnbt_snbt", or null if empty/offline. */
    public static String getSlotItem(String playerName, int slot) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return null;
        Inventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.getContainerSize()) return null;
        net.minecraft.world.item.ItemStack stack = inv.getItem(slot);
        if (stack.isEmpty()) return null;
        String itemId = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        int count = stack.getCount();
        String nbt = stack.getOrDefault(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                net.minecraft.world.item.component.CustomData.EMPTY).copyTag().toString();
        return itemId + "\t" + count + "\t" + nbt;
    }

    /** Replace inventory slot; snbt merged into new item's NBT (pass "" for no NBT). */
    public static boolean setSlotItem(String playerName, int slot, String itemId, int count, String snbt) {
        ServerPlayer p = playerByName(playerName);
        if (p == null) return false;
        Inventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.getContainerSize()) return false;
        if (count <= 0) {
            inv.setItem(slot, net.minecraft.world.item.ItemStack.EMPTY);
            return true;
        }
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        net.minecraft.world.item.ItemStack stack = new net.minecraft.world.item.ItemStack(BuiltInRegistries.ITEM.getValue(id), count);
        if (snbt != null && !snbt.isEmpty()) {
            try {
                CompoundTag nbt = TagParser.parseCompoundFully(snbt);
                stack.set(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                        net.minecraft.world.item.component.CustomData.of(nbt));
            } catch (Exception ignored) {}
        }
        inv.setItem(slot, stack);
        return true;
    }

    public static int worldEntityCount(String dimension, String entityTypeId) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return -1;
        Identifier id = Identifier.tryParse(entityTypeId);
        if (id == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(id)) return -1;
        EntityType<?> targetType = BuiltInRegistries.ENTITY_TYPE.getValue(id);
        int count = 0;
        for (Entity e : w.getAllEntities()) {
            if (e.getType() == targetType) count++;
        }
        return count;
    }

    public static String spawnEntity(String typeId, String dimension, double x, double y, double z) {
        ServerLevel w = worldFor(dimension);
        Identifier id = Identifier.tryParse(typeId);
        if (w == null || id == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(id)) {
            return null;
        }
        EntityType<?> type = BuiltInRegistries.ENTITY_TYPE.getValue(id);
        Entity e = type.create(w, EntitySpawnReason.COMMAND);
        if (e == null) {
            return null;
        }
        e.snapTo(x, y, z, e.getYRot(), e.getXRot());
        w.addFreshEntity(e);
        return e.getStringUUID();
    }

    // ── world / entity queries & actions ────────────────────────────────────

    /** Entity by uuid, or null for null / "" / unknown. */
    private static Entity optionalEntity(String uuid) {
        return uuid == null || uuid.isEmpty() ? null : entityByUuid(uuid);
    }

    /** Item stack from id + count + optional custom-data snbt; null if the id is unknown. */
    static ItemStack buildStack(String itemId, int count, String snbt) {
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || count <= 0 || !BuiltInRegistries.ITEM.containsKey(id)) return null;
        ItemStack stack = new ItemStack(BuiltInRegistries.ITEM.getValue(id), count);
        if (snbt != null && !snbt.isEmpty()) {
            try {
                CompoundTag nbt = TagParser.parseCompoundFully(snbt);
                if (!nbt.isEmpty()) {
                    stack.set(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                            net.minecraft.world.item.component.CustomData.of(nbt));
                }
            } catch (Exception ignored) {}
        }
        return stack;
    }

    /** Custom-data snbt of a stack, same format as getSlotItem ("{}" if none). */
    static String stackSnbt(ItemStack stack) {
        return stack.getOrDefault(net.minecraft.core.component.DataComponents.CUSTOM_DATA,
                net.minecraft.world.item.component.CustomData.EMPTY).copyTag().toString();
    }

    /** {@code YogEntityFlags} bits of {@code e}. */
    public static int entityFlags(Entity e) {
        int flags = 0;
        if (e instanceof LivingEntity) flags |= YogEntityFlags.LIVING;
        if (e instanceof Player) flags |= YogEntityFlags.PLAYER;
        if (e instanceof net.minecraft.world.entity.animal.Animal
                || e instanceof net.minecraft.world.entity.animal.fish.WaterAnimal) flags |= YogEntityFlags.ANIMAL;
        if (e instanceof net.minecraft.world.entity.monster.Enemy) flags |= YogEntityFlags.MONSTER;
        if (e instanceof ItemEntity) flags |= YogEntityFlags.ITEM;
        if (e instanceof net.minecraft.world.entity.Mob) flags |= YogEntityFlags.MOB;
        if (e instanceof net.minecraft.world.entity.decoration.ArmorStand) flags |= YogEntityFlags.ARMOR_STAND;
        if (e.isAlive()) flags |= YogEntityFlags.ALIVE;
        if (e.isSpectator()) flags |= YogEntityFlags.SPECTATOR;
        if (e instanceof net.minecraft.world.entity.boss.enderdragon.EnderDragonPart) flags |= YogEntityFlags.PART;
        if (e instanceof net.minecraft.world.entity.npc.villager.Villager) flags |= YogEntityFlags.VILLAGER;
        if (e.isShiftKeyDown()) flags |= YogEntityFlags.SNEAKING;
        if (e.onGround()) flags |= YogEntityFlags.ON_GROUND;
        if (e.horizontalCollision) flags |= YogEntityFlags.HORIZONTAL_COLLISION;
        if (e instanceof net.minecraft.world.entity.LivingEntity gliding && gliding.isFallFlying()) flags |= YogEntityFlags.GLIDING;
        return flags;
    }

    /** "type\tdim\tx\ty\tz\teyeHeight\twidth\theight\tflags\tname", or null. */
    public static String entityInfo(String uuid) {
        try {
            Entity e = entityByUuid(uuid);
            if (e == null) return null;
            int flags = entityFlags(e);
            return BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()) + "\t"
                    + e.level().dimension().identifier() + "\t"
                    + e.getX() + "\t" + e.getY() + "\t" + e.getZ() + "\t"
                    + e.getEyeHeight() + "\t" + e.getBbWidth() + "\t" + e.getBbHeight() + "\t"
                    + flags + "\t" + tsvField(e.getName().getString());
        } catch (Throwable t) {
            return null;
        }
    }

    /** UUIDs (newline-separated) of entities intersecting the box; null if dim unknown. */
    public static String entitiesInBox(String dim, double x0, double y0, double z0, double x1, double y1, double z1) {
        try {
            ServerLevel w = worldFor(dim);
            if (w == null) return null;
            StringBuilder sb = new StringBuilder();
            for (Entity e : w.getEntities((Entity) null, new AABB(x0, y0, z0, x1, y1, z1), o -> true)) {
                if (sb.length() > 0) sb.append('\n');
                sb.append(e.getStringUUID());
            }
            return sb.toString();
        } catch (Throwable t) {
            return null;
        }
    }

    /** "x\ty\tz\tface\thx\thy\thz" of the first block hit, or null on miss. */
    public static String raycastBlock(String dim, double fx, double fy, double fz, double tx, double ty, double tz) {
        try {
            ServerLevel w = worldFor(dim);
            if (w == null) return null;
            BlockHitResult hit = w.clip(new ClipContext(new Vec3(fx, fy, fz), new Vec3(tx, ty, tz),
                    ClipContext.Block.COLLIDER, ClipContext.Fluid.NONE, CollisionContext.empty()));
            if (hit == null || hit.getType() == HitResult.Type.MISS) return null;
            BlockPos p = hit.getBlockPos();
            Vec3 h = hit.getLocation();
            return p.getX() + "\t" + p.getY() + "\t" + p.getZ() + "\t" + hit.getDirection().get3DDataValue()
                    + "\t" + h.x + "\t" + h.y + "\t" + h.z;
        } catch (Throwable t) {
            return null;
        }
    }

    /** UUID of the nearest entity hit along from→to (ProjectileUtil semantics), or null. */
    public static String raycastEntity(String dim, double fx, double fy, double fz,
            double tx, double ty, double tz, String excludeUuid) {
        try {
            ServerLevel w = worldFor(dim);
            if (w == null) return null;
            Entity ex = optionalEntity(excludeUuid);
            Vec3 from = new Vec3(fx, fy, fz);
            Vec3 to = new Vec3(tx, ty, tz);
            AABB area = new AABB(fx, fy, fz, tx, ty, tz).inflate(1.0);
            Entity best = null;
            double bestDist = Double.MAX_VALUE;
            for (Entity c : w.getEntities(ex, area, o -> !o.isSpectator() && o.isPickable())) {
                if (ex != null && c.getRootVehicle() == ex.getRootVehicle()) continue;
                AABB box = c.getBoundingBox().inflate(c.getPickRadius());
                if (box.contains(from)) {
                    if (bestDist > 0.0) {
                        best = c;
                        bestDist = 0.0;
                    }
                    continue;
                }
                java.util.Optional<Vec3> clip = box.clip(from, to);
                if (clip.isPresent()) {
                    double d = from.distanceToSqr(clip.get());
                    if (d < bestDist) {
                        best = c;
                        bestDist = d;
                    }
                }
            }
            return best == null ? null : best.getStringUUID();
        } catch (Throwable t) {
            return null;
        }
    }

    /** "state\thardness\treplaceable\tair\tfluid\tsource\tredstone\tinWorld", or null. */
    public static String blockInfo(String dim, int x, int y, int z) {
        try {
            ServerLevel w = worldFor(dim);
            if (w == null) return null;
            BlockPos pos = new BlockPos(x, y, z);
            BlockState st = w.getBlockState(pos);
            FluidState fs = st.getFluidState();
            String fluid = fs.isEmpty() ? "" : BuiltInRegistries.FLUID.getKey(fs.getType()).toString();
            boolean inWorld = w.isInWorldBounds(pos) && w.getWorldBorder().isWithinBounds(pos);
            return BlockStateParser.serialize(st) + "\t"
                    + st.getDestroySpeed(w, pos) + "\t"
                    + flag01(st.canBeReplaced()) + "\t"
                    + flag01(st.isAir()) + "\t"
                    + fluid + "\t"
                    + flag01(!fs.isEmpty() && fs.isSource()) + "\t"
                    + w.getBestNeighborSignal(pos) + "\t"
                    + flag01(inWorld);
        } catch (Throwable t) {
            return null;
        }
    }

    /** Break the block (dropping items if `drop`); false for air / unbreakable blocks. */
    public static boolean breakBlock(String dim, int x, int y, int z, boolean drop, String breakerUuid) {
        try {
            ServerLevel w = worldFor(dim);
            if (w == null) return false;
            BlockPos pos = new BlockPos(x, y, z);
            BlockState st = w.getBlockState(pos);
            if (st.isAir() || st.getDestroySpeed(w, pos) < 0) return false;
            return w.destroyBlock(pos, drop, optionalEntity(breakerUuid));
        } catch (Throwable t) {
            return false;
        }
    }

    /** Place the BlockItem from the player's inventory slot at a replaceable position. */
    public static boolean placeBlockFromSlot(String player, int slot, String dim, int x, int y, int z) {
        try {
            ServerPlayer p = playerByName(player);
            ServerLevel w = worldFor(dim);
            if (p == null || w == null || p.level() != w) return false;
            Inventory inv = p.getInventory();
            if (slot < 0 || slot >= inv.getContainerSize()) return false;
            ItemStack stack = inv.getItem(slot);
            if (stack.isEmpty() || !(stack.getItem() instanceof BlockItem bi)) return false;
            BlockPos pos = new BlockPos(x, y, z);
            if (!w.getBlockState(pos).canBeReplaced()) return false;
            BlockHitResult hit = new BlockHitResult(Vec3.atCenterOf(pos), Direction.UP, pos, false);
            InteractionResult r = bi.place(new BlockPlaceContext(p, InteractionHand.MAIN_HAND, stack, hit));
            return r.consumesAction();
        } catch (Throwable t) {
            return false;
        }
    }

    /** Apply bone meal at the position (crops / saplings / grass, or water plants). */
    public static boolean bonemeal(String dim, int x, int y, int z) {
        try {
            ServerLevel w = worldFor(dim);
            if (w == null) return false;
            BlockPos pos = new BlockPos(x, y, z);
            boolean ok = BoneMealItem.growCrop(new ItemStack(Items.BONE_MEAL), w, pos)
                    || BoneMealItem.growWaterPlant(new ItemStack(Items.BONE_MEAL), w, pos, null);
            if (ok) w.levelEvent(LevelEvent.PARTICLES_AND_SOUND_PLANT_GROWTH, pos, 15);
            return ok;
        } catch (Throwable t) {
            return false;
        }
    }

    public static boolean explode(String dim, double x, double y, double z, float power, boolean fire, String sourceUuid) {
        try {
            ServerLevel w = worldFor(dim);
            if (w == null) return false;
            w.explode(optionalEntity(sourceUuid), x, y, z, power, fire, Level.ExplosionInteraction.TNT);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    /** A JSON text component (the /tellraw format), or null when it isn't one. */
    private static Component textFromJson(String json, net.minecraft.core.HolderLookup.Provider registries) {
        return ComponentSerialization.CODEC
                .parse(registries.createSerializationContext(JsonOps.INSTANCE), JsonParser.parseString(json))
                .result().orElse(null);
    }

    /** A system chat message from a JSON text component (the /tellraw format). */
    public static boolean tellraw(String player, String json) {
        try {
            ServerPlayer p = playerByName(player);
            if (p == null) return false;
            Component text = textFromJson(json, p.registryAccess());
            if (text == null) return false;
            p.sendSystemMessage(text);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    /** Damage an entity with a damage type id (empty: from the attacker), caused by attacker (uuid or ""). */
    public static boolean entityDamage(String uuid, float amount, String damageType, String attackerUuid) {
        try {
            Entity e = entityByUuid(uuid);
            if (e == null) return false;
            Entity src = optionalEntity(attackerUuid);
            DamageSource ds = null;
            Identifier tid = damageType == null || damageType.isEmpty() ? null : Identifier.tryParse(damageType);
            if (tid != null) {
                var type = e.level().registryAccess().lookupOrThrow(Registries.DAMAGE_TYPE)
                        .get(ResourceKey.create(Registries.DAMAGE_TYPE, tid));
                if (type.isPresent()) ds = new DamageSource(type.get(), src);
            }
            if (ds == null) {
                if (tid == null && src instanceof Player sp) {
                    ds = e.damageSources().playerAttack(sp);
                } else if (tid == null && src instanceof LivingEntity sl) {
                    ds = e.damageSources().mobAttack(sl);
                } else {
                    ds = e.damageSources().magic();
                }
            }
            return e.level() instanceof ServerLevel w && e.hurtServer(w, ds, amount);
        } catch (Throwable t) {
            return false;
        }
    }

    public static boolean entitySetFireTicks(String uuid, int ticks) {
        try {
            Entity e = entityByUuid(uuid);
            if (e == null) return false;
            e.setRemainingFireTicks(ticks);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    public static boolean playerAddXp(String player, int amount) {
        try {
            ServerPlayer p = playerByName(player);
            if (p == null) return false;
            p.giveExperiencePoints(amount);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    /** Grant / revoke flight; revoking leaves creative / spectator players untouched. */
    public static boolean playerSetMayFly(String player, boolean mayFly) {
        try {
            ServerPlayer p = playerByName(player);
            if (p == null) return false;
            GameType gm = p.gameMode.getGameModeForPlayer();
            if (!mayFly && (gm.isCreative() || gm == GameType.SPECTATOR)) return true;
            p.getAbilities().mayfly = mayFly;
            if (!mayFly) p.getAbilities().flying = false;
            p.onUpdateAbilities();
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    /** "uuid\tgamemode\tselectedSlot\txpLevel\txpTotal\tair\tmayFly\tflying", or null. */
    public static String playerInfo(String player) {
        try {
            ServerPlayer p = playerByName(player);
            if (p == null) return null;
            return p.getStringUUID() + "\t"
                    + p.gameMode.getGameModeForPlayer().getName() + "\t"
                    + p.getInventory().getSelectedSlot() + "\t"
                    + p.experienceLevel + "\t"
                    + p.totalExperience + "\t"
                    + p.getAirSupply() + "\t"
                    + flag01(p.getAbilities().mayfly) + "\t"
                    + flag01(p.getAbilities().flying);
        } catch (Throwable t) {
            return null;
        }
    }

    /** Spawn an item entity with the given velocity; returns its uuid or null. */
    public static String spawnItem(String dim, double x, double y, double z,
            double vx, double vy, double vz, String itemId, int count, String snbt) {
        try {
            ServerLevel w = worldFor(dim);
            if (w == null) return null;
            ItemStack stack = buildStack(itemId, count, snbt);
            if (stack == null || stack.isEmpty()) return null;
            ItemEntity ie = new ItemEntity(w, x, y, z, stack);
            ie.setDeltaMovement(vx, vy, vz);
            if (!w.addFreshEntity(ie)) return null;
            return ie.getStringUUID();
        } catch (Throwable t) {
            return null;
        }
    }

    /** "itemId\tcount\tsnbt" of an item entity's stack, or null. */
    public static String itemEntityStack(String uuid) {
        try {
            if (!(entityByUuid(uuid) instanceof ItemEntity ie)) return null;
            ItemStack stack = ie.getItem();
            return BuiltInRegistries.ITEM.getKey(stack.getItem()) + "\t" + stack.getCount() + "\t" + stackSnbt(stack);
        } catch (Throwable t) {
            return null;
        }
    }

    /** Replace an item entity's stack; count <= 0 or empty id discards the entity. */
    public static boolean itemEntitySetStack(String uuid, String itemId, int count, String snbt) {
        try {
            if (!(entityByUuid(uuid) instanceof ItemEntity ie)) return false;
            if (count <= 0 || itemId == null || itemId.isEmpty()) {
                ie.discard();
                return true;
            }
            ItemStack stack = buildStack(itemId, count, snbt);
            if (stack == null) return false;
            ie.setItem(stack);
            return true;
        } catch (Throwable t) {
            return false;
        }
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
        for (ServerLevel w : s.getAllLevels()) {
            Entity e = w.getEntity(uuid);
            if (e != null) {
                return e;
            }
        }
        return null;
    }

    private static ServerLevel worldFor(String dimension) {
        MinecraftServer s = server;
        Identifier id = Identifier.tryParse(dimension);
        if (s == null || id == null) {
            return null;
        }
        return s.getLevel(ResourceKey.create(Registries.DIMENSION, id));
    }

    // ── ABI minor 3 ──────────────────────────────────────────────────────────

    /** Puts {@code itemId} on cooldown for {@code ticks} for an online player. */
    public static boolean setItemCooldown(String player, String itemId, int ticks) {
        ServerPlayer p = playerByName(player);
        Identifier id = Identifier.tryParse(itemId);
        if (p == null || id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
        p.getCooldowns().addCooldown(BuiltInRegistries.ITEM.getValue(id).getDefaultInstance(), ticks);
        return true;
    }

    /** The UUID of what an entity rides, or null. */
    public static String entityVehicle(String uuid) {
        Entity e = entityByUuid(uuid);
        Entity v = e == null ? null : e.getVehicle();
        return v == null ? null : v.getStringUUID();
    }

    /** The UUIDs of what rides an entity directly, one a line, or null. */
    public static String entityPassengers(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) return null;
        StringBuilder out = new StringBuilder();
        for (Entity p : e.getPassengers()) out.append(p.getStringUUID()).append('\n');
        return out.toString();
    }

    /** Seats {@code rider} on {@code vehicle}; {@code force}: even one that wouldn't take it. */
    public static boolean entityMount(String rider, String vehicle, boolean force) {
        Entity r = entityByUuid(rider), v = entityByUuid(vehicle);
        return r != null && v != null && r.startRiding(v, force, true);
    }

    /** Gets an entity off what it rides. */
    /** An entity's own hitbox (blocks), or vanilla's again (width &lt; 0). */
    public static boolean entitySetDimensions(String uuid, float width, float height, float eye) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        dev.yog.util.YogDimensions.set(false, e.getUUID(), width, height, eye);
        e.refreshDimensions();
        return true;
    }

    /** Sets how far the entity has fallen (fall damage counts it on landing). */
    public static boolean entitySetFallDistance(String uuid, float distance) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        e.fallDistance = distance;
        return true;
    }

    public static boolean entityDismount(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null || !e.isPassenger()) return false;
        e.stopRiding();
        return true;
    }

    /** The strongest signal the block gives off (dust: its power), or -1 if it isn't a source. */
    public static int redstoneSignal(String dimension, int x, int y, int z) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return -1;
        net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(x, y, z);
        net.minecraft.world.level.block.state.BlockState state = w.getBlockState(pos);
        if (state.getBlock() instanceof net.minecraft.world.level.block.RedStoneWireBlock) {
            return state.getValue(net.minecraft.world.level.block.RedStoneWireBlock.POWER);
        }
        if (!state.isSignalSource()) return -1;
        int best = 0;
        for (net.minecraft.core.Direction d : net.minecraft.core.Direction.values()) {
            best = Math.max(best, state.getSignal(w, pos, d));
        }
        return best;
    }

    /** The redstone power a block receives from its neighbours (0–15), or -1. */
    public static int redstonePower(String dimension, int x, int y, int z) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return -1;
        net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(x, y, z);
        return w.getBestNeighborSignal(pos);
    }

    /** Power a Yog block declared as a redstone source gives off, by "dimension|x|y|z". */
    public static final java.util.Map<String, Integer> REDSTONE_OUTPUT = new java.util.concurrent.ConcurrentHashMap<>();

    public static String redstoneKey(String dimension, int x, int y, int z) {
        return dimension + "|" + x + "|" + y + "|" + z;
    }

    /** Sets the power (0–15) a redstone-source Yog block at a position gives off. */
    public static boolean setRedstoneOutput(String dimension, int x, int y, int z, int level) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        String key = redstoneKey(dimension, x, y, z);
        int clamped = Math.max(0, Math.min(15, level));
        // Zeros are kept too: the first level set always updates the
        // neighbours — a lamp saved lit goes out when its source starts at 0.
        Integer before = REDSTONE_OUTPUT.put(key, clamped);
        if (before == null || before != clamped) {
            net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(x, y, z);
            w.updateNeighborsAt(pos, w.getBlockState(pos).getBlock(), null);
        }
        return true;
    }

    /** The full state of the block at a position: "minecraft:repeater[delay=2,facing=east,...]". */
    public static String getBlockState(String dimension, int x, int y, int z) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return null;
        net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(x, y, z);
        return net.minecraft.commands.arguments.blocks.BlockStateParser.serialize(w.getBlockState(pos));
    }

    /** Places a block in a full state ("minecraft:repeater[delay=2]"); false if it doesn't parse. */
    public static boolean setBlockState(String dimension, int x, int y, int z, String state) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        try {
            var parsed = net.minecraft.commands.arguments.blocks.BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, state, false).blockState();
            net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(x, y, z);
            return w.setBlockAndUpdate(pos, parsed);
        } catch (Exception e) {
            YogLog.warn("setBlockState: " + state + ": " + e.getMessage());
            return false;
        }
    }

    /** Sets every block of the box to {@code state} (no neighbour updates); how many changed, or -1. */
    public static int fillRegion(String dimension, int x1, int y1, int z1, int x2, int y2, int z2, String state) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return -1;
        try {
            var parsed = net.minecraft.commands.arguments.blocks.BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, state, false).blockState();
            net.minecraft.core.BlockPos from = new net.minecraft.core.BlockPos(x1, y1, z1), to = new net.minecraft.core.BlockPos(x2, y2, z2);
            int changed = 0;
            for (net.minecraft.core.BlockPos pos : net.minecraft.core.BlockPos.betweenClosed(from, to)) {
                if (w.setBlock(pos, parsed, 2)) changed++;
            }
            return changed;
        } catch (Exception e) {
            YogLog.warn("fillRegion: " + state + ": " + e.getMessage());
            return -1;
        }
    }

    /** The box's non-air blocks: "x y z state" lines. */
    public static String scanRegion(String dimension, int x1, int y1, int z1, int x2, int y2, int z2) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return null;
        net.minecraft.core.BlockPos from = new net.minecraft.core.BlockPos(x1, y1, z1), to = new net.minecraft.core.BlockPos(x2, y2, z2);
        StringBuilder sb = new StringBuilder();
        for (net.minecraft.core.BlockPos pos : net.minecraft.core.BlockPos.betweenClosed(from, to)) {
            var st = w.getBlockState(pos);
            if (st.isAir()) continue;
            if (sb.length() > 0) sb.append('\n');
            sb.append(pos.getX()).append(' ').append(pos.getY()).append(' ').append(pos.getZ()).append(' ').append(net.minecraft.commands.arguments.blocks.BlockStateParser.serialize(st));
        }
        return sb.toString();
    }

    public static boolean worldIsThundering(String dimension) {
        ServerLevel w = worldFor(dimension);
        return w != null && w.isThundering();
    }

    /** Starts (or stops) a thunderstorm for {@code durationTicks} (0 = a default). */
    public static boolean worldSetThunder(String dimension, boolean thundering, int durationTicks) {
        ServerLevel w = worldFor(dimension);
        if (w == null) return false;
        int dur = durationTicks > 0 ? durationTicks : DEFAULT_WEATHER_TICKS;
        w.getServer().setWeatherParameters(thundering ? 0 : dur, thundering ? dur : 0, thundering, thundering);
        return true;
    }

    /** Load the embedded native runtime and initialise it. Idempotent. */
    public static synchronized void ensureLoaded() {
        if (loaded) {
            return;
        }
        YogNativeRuntime.loadEmbedded();
        nativeInit(YogNativeRuntime.modsDir(YogPlatform.gameDir()));
        loaded = true;
    }

    // --- native entry points implemented in yog-runtime (Rust) ---

    public static native void nativeInit(String modsDir);

    /** Names of mod-registered commands, one per line. */
    public static native String nativeCommandNames();

    /** Typed command schemas: `name\tschema` per line (only typed commands). */
    public static native String nativeTypedCommandSchemas();

    /** Recipe JSONs: `namespace\tname\tJSON` per line. */

    /**
     * Ask the mod-registered generator type {@code generatorTypeId} to fill
     * chunk ({@code chunkX}, {@code chunkZ}). Blocks until the native
     * callback returns; every block it places arrives via a nested call
     * back into {@link YogCallbackChunkGenerator#setBlockInGeneratingChunk}
     * on this same thread (see {@code yog-dimensions}' {@code ChunkWriter}).
     */
    public static native void nativeGenerateChunk(
            String generatorTypeId, int chunkX, int chunkZ, int minY, int height);

    /** Open a Rust-mod-defined UI. parentId="" for top-level, modal blocks clicks outside, pauseGame pauses MC. */
    public static native void nativeUIShow(String uiId, String parentId, boolean modal, boolean pauseGame, int screenW, int screenH);
    /** Hide / close the UI. */
    public static native void nativeUIHide(String uiId);
    /** Forward a mouse click to Rust. button: 0=left, 1=right, 2=middle. */
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

    /**
     * Close the Yog UI screen showing uiId ("" = any Yog UI screen) — called
     * from Rust. Always queued, so the mod's event handler finishes first.
     */
    public static void closeUI(String uiId) {
        try {
            net.minecraft.client.Minecraft mc = net.minecraft.client.Minecraft.getInstance();
            mc.execute(() -> YogUIScreen.closeIfOpen(uiId));
        } catch (Throwable t) {
            // dedicated server — no client classes; ignore
        }
    }

    /** Mouse dragged over a Yog UI screen (any button held). */
    public static native void nativeUIDrag(String uiId, float mx, float my);
    /** The cursor moved over a Yog UI screen (`move:X:Y`). */
    public static native void nativeUIMove(String uiId, float mx, float my);

    /** Mouse button released over a Yog UI screen. */
    public static native void nativeUIRelease(String uiId, float mx, float my);
    /** Forward a key event (action: 0=release, 1=press, 2=repeat); true when the mod used it. */
    public static native boolean nativeUIKey(String uiId, int keyCode, int scanCode, int modifiers, int action);
    /** Forward a typed character (one UTF-16 unit); true when the mod used it. */
    public static native boolean nativeUIChar(String uiId, char chr, int modifiers);
    /** Render a Rust UI immediately (called from Screen.render, not HUD). */
    public static native void nativeUIRender(String uiId, int screenW, int screenH);
    public static native boolean nativeIsUIActive(String uiId);
    public static native String nativeMenuEntries();
    /** The mods' key bindings (ABI 3.10): {@code id\tname key\tdefault key\tmod name} a line. */
    public static native String nativeKeyBindings();
    /** Run a registered command; returns the reply (empty string if none). */
    public static native String nativeOnCommand(String name, String args, String source, String uuid, String dimension);

    /** Everything registered — items, blocks, inventories — as one Yog Data blob ({@code YogContent}). */
    public static native byte[] nativeContent();

    // (no native entry points needed for #4 — all calls are Rust→Java via JNI)

    public static native void nativeOnPacket(String channel, String player, byte[] payload);

    public static native void nativeOnClientPacket(String channel, byte[] payload);

    /** Server-receiver packet channels, one per line. */
    public static native String nativePacketChannels();

    /** Client-receiver packet channels, one per line. */
    public static native String nativeClientPacketChannels();

    // ── ABI minor 9 ──────────────────────────────────────────────────────────

    // ── ABI minor 11 — held item NBT native declarations ─────────────────────

    // (no native entry points — Rust calls Java via JNI, not the other way around)

    // ── ABI minor 10 — client-side hooks ─────────────────────────────────────

    // ── ABI minor 14 — low-level GL pipeline ─────────────────────────────────

    /** Initialize the glow GL context on the render thread.  Call once after the GL context exists. */
    public static native void nativeGlInit();

    /** HUD render — fired every frame; deltaTick is partial-tick interpolation. */
    public static native void nativeOnHudRender(
            float deltaTick, int screenW, int screenH, float scaleFactor,
            float playerX, float playerY, float playerZ);

    /** World render — fired at WorldRenderEvents.LAST with full camera matrices. */
    public static native void nativeOnWorldRender(
            float deltaTick, int screenW, int screenH, float scaleFactor,
            float[] viewProj, float camX, float camY, float camZ,
            float playerX, float playerY, float playerZ);

    /** Called from Rust during nativeGlInit to resolve GL function pointers. */
    public static long glProcAddress(String name) {
        return GLFW.glfwGetProcAddress(name);
    }

    // ── translations (consumed by the runtime's translate ABI) ─────────────

    /**
     * The text of translation key {@code key} in the game's current
     * language (lang files of every resource pack, .yog mods' included), or
     * null when no language defines it. Format specifiers are left for the
     * caller (the runtime fills them in).
     */
    public static String translate(String key) {
        net.minecraft.locale.Language lang = net.minecraft.locale.Language.getInstance();
        return lang.has(key) ? lang.getOrDefault(key) : null;
    }

    // ── platform mod listing (consumed by the runtime's mods_list ABI) ──────

    /** TSV lines: id \t name \t version \t authors \t description. */
    public static String listPlatformMods() {
        StringBuilder sb = new StringBuilder();
        for (dev.yog.util.YogModInfo m : YogPlatform.mods()) {
            if (sb.length() > 0) sb.append('\n');
            sb.append(tsvField(m.id())).append('\t')
              .append(tsvField(m.name())).append('\t')
              .append(tsvField(m.version())).append('\t')
              .append(tsvField(m.authors())).append('\t')
              .append(tsvField(m.description()));
        }
        return sb.toString();
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
        return net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(stack.getItem()).toString() + "\t" + stack.getCount();
    }

    public static String getSlotPos(int index) {
        if (activeInventoryMenu == null || index < 0 || index >= activeInventoryMenu.slots.size()) return null;
        net.minecraft.world.inventory.Slot slot = activeInventoryMenu.slots.get(index);
        return slot.x + "\t" + slot.y;
    }

    // ── ABI minor 2 ──────────────────────────────────────────────────────────

    /** An action-bar message from a JSON text component. */
    public static boolean actionbarJson(String player, String json) {
        try {
            ServerPlayer p = playerByName(player);
            if (p == null) return false;
            Component text = textFromJson(json, p.registryAccess());
            if (text == null) return false;
            p.connection.send(new ClientboundSetActionBarTextPacket(text));
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    /** Use a fresh single item on a block face, as the named player (or nobody). */
    public static boolean useItemOnBlock(String dim, int x, int y, int z, int face, String itemId, String player) {
        try {
            ServerLevel w = worldFor(dim);
            Identifier id = Identifier.tryParse(itemId);
            if (w == null || id == null || !BuiltInRegistries.ITEM.containsKey(id) || face < 0 || face > 5) return false;
            ServerPlayer p = player.isEmpty() ? null : playerByName(player);
            BlockPos pos = new BlockPos(x, y, z);
            ItemStack stack = new ItemStack(BuiltInRegistries.ITEM.getValue(id));
            BlockHitResult hit = new BlockHitResult(Vec3.atCenterOf(pos), Direction.from3DDataValue(face), pos, false);
            net.minecraft.world.item.context.UseOnContext ctx =
                    new net.minecraft.world.item.context.UseOnContext(w, p, InteractionHand.MAIN_HAND, stack, hit) {};
            return stack.getItem().useOn(ctx).consumesAction();
        } catch (Throwable t) {
            return false;
        }
    }

    /** Empty a bucket item's fluid at the position, the way a player would. */
    public static boolean pourFluid(String dim, int x, int y, int z, String bucketId) {
        try {
            ServerLevel w = worldFor(dim);
            Identifier id = Identifier.tryParse(bucketId);
            if (w == null || id == null || !BuiltInRegistries.ITEM.containsKey(id)) return false;
            if (!(BuiltInRegistries.ITEM.getValue(id) instanceof net.minecraft.world.item.BucketItem bucket)) return false;
            BlockPos pos = new BlockPos(x, y, z);
            BlockHitResult hit = new BlockHitResult(Vec3.atCenterOf(pos), Direction.UP, pos, false);
            return bucket.emptyContents(null, w, pos, hit);
        } catch (Throwable t) {
            return false;
        }
    }

    public static boolean hasEntityTag(String entityType, String tagId) {
        Identifier eid = Identifier.tryParse(entityType);
        Identifier tid = Identifier.tryParse(tagId);
        if (eid == null || tid == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(eid)) return false;
        return BuiltInRegistries.ENTITY_TYPE.get(eid).map(h -> h.is(TagKey.create(Registries.ENTITY_TYPE, tid))).orElse(false);
    }

    /** The block id a block item places, or null. */
    public static String itemBlock(String itemId) {
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || !BuiltInRegistries.ITEM.containsKey(id)) return null;
        if (!(BuiltInRegistries.ITEM.getValue(id) instanceof BlockItem bi)) return null;
        return BuiltInRegistries.BLOCK.getKey(bi.getBlock()).toString();
    }

    public static boolean placeFeature(String dim, int x, int y, int z, String featureId) {
        try {
            ServerLevel w = worldFor(dim);
            Identifier id = Identifier.tryParse(featureId);
            if (w == null || id == null) return false;
            net.minecraft.world.level.levelgen.feature.ConfiguredFeature<?, ?> f =
                    w.registryAccess().lookupOrThrow(Registries.CONFIGURED_FEATURE).getValue(id);
            if (f == null) return false;
            return f.place(w, w.getChunkSource().getGenerator(), w.getRandom(), new BlockPos(x, y, z));
        } catch (Throwable t) {
            return false;
        }
    }

    // ── Pictures in item tooltips (ABI 2.2) ─────────────────────────────────

    /** {@code {w, h}} of the picture a mod gives a stack of {@code item} with custom data {@code snbt}, or null. */
    /** A Yog mod's name and tooltip lines for a stack — `name\nline…` — or null (ABI 2.4). */
    public static native String nativeItemText(String item, String snbt);
    /** yog-body: the ops for a tracked entity's body this frame, or null. */
    public static native byte[] nativeBodyPose(byte[] frame);

    /** Entity data: everything about an entity, for a player who starts seeing it; null for nothing. */
    public static native byte[] nativeEntityDataFor(String uuid);
    /** Entity data: what the entity keeps when saved; null for nothing. */
    public static native byte[] nativeEntityDataSave(String uuid);
    /** Entity data: what it kept, back as it loads. */
    public static native void nativeEntityDataLoad(String uuid, byte[] saved);
    /** Entity data: the entity left the server's memory. */
    public static native void nativeEntityDataForget(String uuid);
    /** Sets ({@code value}) or removes (null) the host's own value about an entity, sent like a mod's, not saved. */
    public static native void nativeEntityDataSet(String uuid, String key, byte[] value);
    /** Entity data: the client left its world. */
    public static native void nativeEntityDataClientReset();
    /** yog-body: {right, up, forward, wx, wy, wz, yaw, pitch, roll, fov} for the camera, or null. */
    public static native float[] nativeCamera(String uuid, float partial, double x, double y, double z, float yaw, float pitch, boolean detached, int flags);
    /** yog-body: the velocity to move the local player by instead of its travel step, or null. */
    public static native float[] nativeTravel(String uuid, float vx, float vy, float vz, float lx, float ly, float lz, float yaw, float pitch,
            float forward, float strafe, boolean jumping, boolean sneaking, float gravity, int flags);

    /** What machine {@code defId} at a block makes of {@code items} ({@code \n}-joined by slot): {@code recipe\nresult\ncount}, or null. */
    public static native String nativeMachineResult(String defId, String dimension, int x, int y, int z, String items);

    /** A player took machine {@code defId}'s result of {@code recipe}: the mods take their share. */
    public static native boolean nativeMachineCraft(String defId, String recipe, String dimension, int x, int y, int z, String items, String player);
    public static native float[] nativeTooltipSize(String item, String snbt);

    /** Draws that picture in {@code x, y, w, h} (GUI pixels). */
    public static native void nativeTooltipDraw(String item, String snbt, int x, int y, int w, int h, int screenW, int screenH);

    /** A stack's custom data as SNBT, what tooltip pictures are told. */
    public static String tooltipSnbt(net.minecraft.world.item.ItemStack stack) {
        return stackSnbt(stack);
    }

    // ── Other mods (ABI 2.3) ────────────────────────────────────────────────

    /** Whether Java mod {@code id} is loaded (Yog mods are the runtime's to know). */
    public static boolean isModLoaded(String id) {
        try {
            return YogPlatform.isModLoaded(id);
        } catch (Throwable t) {
            return false;
        }
    }

    static {
        // How java_call finds an entity by uuid, and an entity's uuid.
        dev.yog.data.YogJava.entityByUuid = NativeBridge::entityByUuid;
        dev.yog.data.YogJava.uuidOf = o -> o instanceof Entity e ? e.getStringUUID() : null;
        // Which vanilla items this version has, for content named by another version's ids.
        dev.yog.data.YogVersions.itemExists = id -> { var i = Identifier.tryParse(id); return i != null && BuiltInRegistries.ITEM.containsKey(i); };
    }

    /** A hopper or dropper offers {@code stack} to the Yog block at {@code pos} ({@code block.accepts_items}). */
    public static boolean itemInsert(net.minecraft.world.level.Level world, net.minecraft.core.BlockPos pos,
                                     ItemStack stack, boolean simulate) {
        String block = BuiltInRegistries.BLOCK.getKey(world.getBlockState(pos).getBlock()).toString();
        String item = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
        return nativeOnItemInsert(block, pos.getX(), pos.getY(), pos.getZ(),
                world.dimension().identifier().toString(), item, stack.getCount(), stackSnbt(stack), simulate);
    }

    public static native boolean nativeOnItemInsert(String block, int x, int y, int z, String dimension,
                                                    String item, int count, String nbt, boolean simulate);
}
