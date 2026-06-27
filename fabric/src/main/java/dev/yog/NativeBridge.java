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
import net.minecraft.entity.attribute.EntityAttribute;
import net.minecraft.entity.attribute.EntityAttributeInstance;
import net.minecraft.entity.boss.CommandBossBar;
import net.minecraft.block.entity.BlockEntity;
import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.ItemEntity;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerInventory;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.nbt.StringNbtReader;
import net.minecraft.entity.boss.BossBar;
import net.minecraft.entity.effect.StatusEffect;
import net.minecraft.loot.LootDataKey;
import net.minecraft.loot.LootDataType;
import net.minecraft.loot.LootTable;
import net.minecraft.loot.context.LootContextParameterSet;
import net.minecraft.entity.effect.StatusEffectInstance;
import net.minecraft.item.ItemStack;
import net.minecraft.network.packet.s2c.play.OverlayMessageS2CPacket;
import net.minecraft.network.packet.s2c.play.SubtitleS2CPacket;
import net.minecraft.network.packet.s2c.play.TitleFadeS2CPacket;
import net.minecraft.network.packet.s2c.play.TitleS2CPacket;
import net.minecraft.registry.tag.TagKey;
import net.minecraft.item.Item;
import net.minecraft.network.PacketByteBuf;
import net.minecraft.registry.Registries;
import net.minecraft.registry.RegistryKey;
import net.minecraft.registry.RegistryKeys;
import net.minecraft.entity.boss.BossBarManager;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.server.world.ServerWorld;
import net.minecraft.sound.SoundCategory;
import net.minecraft.sound.SoundEvent;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import org.lwjgl.glfw.GLFW;
import net.minecraft.world.GameMode;
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
        if (p == null) { System.out.println("[yog] giveItem: player not found: " + player); return false; }
        if (id == null) { System.out.println("[yog] giveItem: bad id: " + itemId); return false; }
        if (count <= 0) { System.out.println("[yog] giveItem: bad count: " + count); return false; }
        if (!Registries.ITEM.containsId(id)) {
            System.out.println("[yog] giveItem: item not registered: " + itemId);
            // List what IS registered for debugging
            if (itemId.startsWith("yog:")) {
                System.out.println("[yog] known yog: items: " +
                    Registries.ITEM.getIds().stream()
                        .filter(i -> i.getNamespace().equals("yog"))
                        .map(Identifier::toString).toList());
            }
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

    public static String entityVelocity(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) return null;
        net.minecraft.util.math.Vec3d v = e.getVelocity();
        return v.x + "\t" + v.y + "\t" + v.z;
    }

    public static boolean entitySetVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        e.setVelocity(vx, vy, vz);
        e.velocityModified = true;
        return true;
    }

    public static boolean entityAddVelocity(String uuid, double vx, double vy, double vz) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        e.addVelocity(vx, vy, vz);
        e.velocityModified = true;
        return true;
    }

    /** Score of {@code player} on {@code objective}, or {@code Integer.MIN_VALUE} if unknown. */
    public static int scoreboardGet(String objective, String player) {
        MinecraftServer s = server;
        if (s == null) return Integer.MIN_VALUE;
        net.minecraft.scoreboard.Scoreboard sb = s.getScoreboard();
        net.minecraft.scoreboard.ScoreboardObjective obj = sb.getNullableObjective(objective);
        if (obj == null) return Integer.MIN_VALUE;
        if (!sb.playerHasObjective(player, obj)) return 0;
        return sb.getPlayerScore(player, obj).getScore();
    }

    public static boolean scoreboardSet(String objective, String player, int score) {
        MinecraftServer s = server;
        if (s == null) return false;
        net.minecraft.scoreboard.Scoreboard sb = s.getScoreboard();
        net.minecraft.scoreboard.ScoreboardObjective obj = sb.getNullableObjective(objective);
        if (obj == null) return false;
        sb.getPlayerScore(player, obj).setScore(score);
        return true;
    }

    /** Returns new score, or {@code Integer.MIN_VALUE} if objective unknown. */
    public static int scoreboardAdd(String objective, String player, int delta) {
        MinecraftServer s = server;
        if (s == null) return Integer.MIN_VALUE;
        net.minecraft.scoreboard.Scoreboard sb = s.getScoreboard();
        net.minecraft.scoreboard.ScoreboardObjective obj = sb.getNullableObjective(objective);
        if (obj == null) return Integer.MIN_VALUE;
        net.minecraft.scoreboard.ScoreboardPlayerScore score = sb.getPlayerScore(player, obj);
        score.incrementScore(delta);
        return score.getScore();
    }

    public static boolean playSound(
            String dimension, double x, double y, double z,
            String soundId, float volume, float pitch) {
        ServerWorld w = worldFor(dimension);
        Identifier id = Identifier.tryParse(soundId);
        if (w == null || id == null) return false;
        w.playSound(null, x, y, z, SoundEvent.of(id), SoundCategory.MASTER, volume, pitch);
        return true;
    }

    public static boolean playSoundToPlayer(String playerName, String soundId, float volume, float pitch) {
        ServerPlayerEntity p = playerByName(playerName);
        Identifier id = Identifier.tryParse(soundId);
        if (p == null || id == null) return false;
        p.getServerWorld().playSound(
                null, p.getX(), p.getY(), p.getZ(),
                SoundEvent.of(id), SoundCategory.MASTER, volume, pitch);
        return true;
    }

    public static boolean sendTitle(
            String playerName, String title, String subtitle,
            int fadein, int stay, int fadeout) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return false;
        p.networkHandler.sendPacket(new TitleFadeS2CPacket(fadein, stay, fadeout));
        if (!title.isEmpty()) {
            p.networkHandler.sendPacket(new TitleS2CPacket(Text.literal(title)));
        }
        if (!subtitle.isEmpty()) {
            p.networkHandler.sendPacket(new SubtitleS2CPacket(Text.literal(subtitle)));
        }
        return true;
    }

    public static boolean sendActionbar(String playerName, String message) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return false;
        p.networkHandler.sendPacket(new OverlayMessageS2CPacket(Text.literal(message)));
        return true;
    }

    public static boolean kickPlayer(String playerName, String reason) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return false;
        p.networkHandler.disconnect(Text.literal(reason));
        return true;
    }

    public static boolean setGamemode(String playerName, String gamemode) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return false;
        GameMode mode = switch (gamemode.toLowerCase(Locale.ROOT)) {
            case "survival", "s", "0" -> GameMode.SURVIVAL;
            case "creative", "c", "1" -> GameMode.CREATIVE;
            case "adventure", "a", "2" -> GameMode.ADVENTURE;
            case "spectator", "sp", "3" -> GameMode.SPECTATOR;
            default -> null;
        };
        if (mode == null) return false;
        p.changeGameMode(mode);
        return true;
    }

    // ── boss bar ────────────────────────────────────────────────────────────

    public static boolean bossbarCreate(String id, String title, String color, String style) {
        MinecraftServer s = server;
        if (s == null) return false;
        Identifier bid = Identifier.tryParse(id);
        if (bid == null) return false;
        BossBarManager mgr = s.getBossBarManager();
        if (mgr.get(bid) != null) return false;
        CommandBossBar bar = mgr.add(bid, Text.literal(title));
        bar.setColor(parseBossBarColor(color));
        bar.setStyle(parseBossBarStyle(style));
        return true;
    }

    public static boolean bossbarRemove(String id) {
        MinecraftServer s = server;
        if (s == null) return false;
        Identifier bid = Identifier.tryParse(id);
        if (bid == null) return false;
        BossBarManager mgr = s.getBossBarManager();
        CommandBossBar bar = mgr.get(bid);
        if (bar == null) return false;
        mgr.remove(bar);
        return true;
    }

    public static boolean bossbarSetTitle(String id, String title) {
        CommandBossBar bar = getBossBar(id);
        if (bar == null) return false;
        bar.setName(Text.literal(title));
        return true;
    }

    public static boolean bossbarSetProgress(String id, float progress) {
        CommandBossBar bar = getBossBar(id);
        if (bar == null) return false;
        bar.setPercent(Math.max(0f, Math.min(1f, progress)));
        return true;
    }

    public static boolean bossbarSetColor(String id, String color) {
        CommandBossBar bar = getBossBar(id);
        if (bar == null) return false;
        bar.setColor(parseBossBarColor(color));
        return true;
    }

    public static boolean bossbarAddPlayer(String id, String playerName) {
        CommandBossBar bar = getBossBar(id);
        ServerPlayerEntity p = playerByName(playerName);
        if (bar == null || p == null) return false;
        bar.addPlayer(p);
        return true;
    }

    public static boolean bossbarRemovePlayer(String id, String playerName) {
        CommandBossBar bar = getBossBar(id);
        ServerPlayerEntity p = playerByName(playerName);
        if (bar == null || p == null) return false;
        bar.removePlayer(p);
        return true;
    }

    public static boolean bossbarSetVisible(String id, boolean visible) {
        CommandBossBar bar = getBossBar(id);
        if (bar == null) return false;
        bar.setVisible(visible);
        return true;
    }

    private static CommandBossBar getBossBar(String id) {
        MinecraftServer s = server;
        if (s == null) return null;
        Identifier bid = Identifier.tryParse(id);
        return bid == null ? null : s.getBossBarManager().get(bid);
    }

    private static BossBar.Color parseBossBarColor(String color) {
        return switch (color.toLowerCase(Locale.ROOT)) {
            case "pink"   -> BossBar.Color.PINK;
            case "blue"   -> BossBar.Color.BLUE;
            case "red"    -> BossBar.Color.RED;
            case "green"  -> BossBar.Color.GREEN;
            case "yellow" -> BossBar.Color.YELLOW;
            case "purple" -> BossBar.Color.PURPLE;
            default       -> BossBar.Color.WHITE;
        };
    }

    private static BossBar.Style parseBossBarStyle(String style) {
        return switch (style.toLowerCase(Locale.ROOT)) {
            case "notched_6"  -> BossBar.Style.NOTCHED_6;
            case "notched_10" -> BossBar.Style.NOTCHED_10;
            case "notched_12" -> BossBar.Style.NOTCHED_12;
            case "notched_20" -> BossBar.Style.NOTCHED_20;
            default           -> BossBar.Style.PROGRESS;
        };
    }

    public static String onlinePlayers() {
        MinecraftServer s = server;
        if (s == null) return null;
        StringBuilder sb = new StringBuilder();
        for (ServerPlayerEntity p : s.getPlayerManager().getPlayerList()) {
            if (sb.length() > 0) sb.append('\n');
            sb.append(p.getName().getString());
        }
        return sb.toString();
    }

    public static String getBlockNbt(String dimension, int x, int y, int z) {
        ServerWorld w = worldFor(dimension);
        if (w == null) return null;
        BlockEntity be = w.getBlockEntity(new BlockPos(x, y, z));
        if (be == null) return null;
        NbtCompound nbt = be.createNbt();
        return nbt.toString();
    }

    public static boolean setBlockNbt(String dimension, int x, int y, int z, String snbt) {
        ServerWorld w = worldFor(dimension);
        if (w == null) return false;
        BlockPos pos = new BlockPos(x, y, z);
        BlockEntity be = w.getBlockEntity(pos);
        if (be == null) return false;
        try {
            NbtCompound nbt = StringNbtReader.parse(snbt);
            be.readNbt(nbt);
            be.markDirty();
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    public static String playerInventory(String playerName) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return null;
        PlayerInventory inv = p.getInventory();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < inv.size(); i++) {
            net.minecraft.item.ItemStack stack = inv.getStack(i);
            if (!stack.isEmpty()) {
                if (sb.length() > 0) sb.append('\n');
                sb.append(i).append('\t')
                  .append(Registries.ITEM.getId(stack.getItem())).append('\t')
                  .append(stack.getCount());
            }
        }
        return sb.toString();
    }

    public static boolean playerSetSlot(String playerName, int slot, String itemId, int count) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return false;
        PlayerInventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.size()) return false;
        if (count <= 0) {
            inv.setStack(slot, net.minecraft.item.ItemStack.EMPTY);
            return true;
        }
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || !Registries.ITEM.containsId(id)) return false;
        inv.setStack(slot, new net.minecraft.item.ItemStack(Registries.ITEM.get(id), count));
        return true;
    }

    public static boolean teleportToDim(String playerName, String dimension, double x, double y, double z) {
        ServerPlayerEntity p = playerByName(playerName);
        ServerWorld w = worldFor(dimension);
        if (p == null || w == null) return false;
        p.teleport(w, x, y, z, p.getYaw(), p.getPitch());
        return true;
    }

    public static boolean entityTeleportToDim(String uuid, String dimension, double x, double y, double z) {
        Entity e = entityByUuid(uuid);
        ServerWorld w = worldFor(dimension);
        if (e == null || w == null) return false;
        if (e instanceof ServerPlayerEntity p) {
            p.teleport(w, x, y, z, p.getYaw(), p.getPitch());
        } else {
            e.teleport(x, y, z);
        }
        return true;
    }

    public static String gameDir() {
        MinecraftServer s = server;
        if (s == null) return null;
        return s.getRunDirectory().getAbsolutePath();
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

    public static String entityGetNbt(String uuid) {
        Entity e = entityByUuid(uuid);
        if (e == null) return null;
        NbtCompound nbt = new NbtCompound();
        e.writeNbt(nbt);
        return nbt.toString();
    }

    public static boolean entitySetNbt(String uuid, String snbt) {
        Entity e = entityByUuid(uuid);
        if (e == null) return false;
        try {
            NbtCompound nbt = StringNbtReader.parse(snbt);
            e.readNbt(nbt);
            return true;
        } catch (Exception ex) {
            return false;
        }
    }

    public static boolean spawnParticles(
            String dimension, double x, double y, double z,
            String particleTypeId, int count,
            double dx, double dy, double dz, double speed) {
        ServerWorld w = worldFor(dimension);
        if (w == null) return false;
        Identifier id = Identifier.tryParse(particleTypeId);
        if (id == null) return false;
        net.minecraft.particle.ParticleType<?> type = Registries.PARTICLE_TYPE.get(id);
        if (!(type instanceof net.minecraft.particle.ParticleEffect effect)) return false;
        w.spawnParticles(effect, x, y, z, count, dx, dy, dz, speed);
        return true;
    }

    public static double entityAttributeGet(String uuid, String attributeId) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return Double.NaN;
        Identifier id = Identifier.tryParse(attributeId);
        if (id == null || !Registries.ATTRIBUTE.containsId(id)) return Double.NaN;
        EntityAttribute attr = Registries.ATTRIBUTE.get(id);
        EntityAttributeInstance inst = le.getAttributeInstance(attr);
        return inst == null ? Double.NaN : inst.getBaseValue();
    }

    public static boolean entityAttributeSet(String uuid, String attributeId, double value) {
        Entity e = entityByUuid(uuid);
        if (!(e instanceof LivingEntity le)) return false;
        Identifier id = Identifier.tryParse(attributeId);
        if (id == null || !Registries.ATTRIBUTE.containsId(id)) return false;
        EntityAttribute attr = Registries.ATTRIBUTE.get(id);
        EntityAttributeInstance inst = le.getAttributeInstance(attr);
        if (inst == null) return false;
        inst.setBaseValue(value);
        return true;
    }

    // ── held item NBT (ABI minor 11) ─────────────────────────────────────────

    /** SNBT of the item in the player's main hand, or null if offline / holding air. */
    public static String getHeldItemNbt(String playerName) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return null;
        net.minecraft.item.ItemStack stack = p.getMainHandStack();
        if (stack.isEmpty()) return null;
        NbtCompound nbt = stack.hasNbt() ? stack.getNbt() : new NbtCompound();
        return nbt.toString();
    }

    /** Merge snbt into the NBT of the item in the player's main hand.
     *  Returns false if the player is offline or holding air. */
    public static boolean setHeldItemNbt(String playerName, String snbt) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return false;
        net.minecraft.item.ItemStack stack = p.getMainHandStack();
        if (stack.isEmpty()) return false;
        try {
            NbtCompound nbt = StringNbtReader.parse(snbt);
            stack.setNbt(nbt);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    // ── item stack query (ABI minor 12) ──────────────────────────────────────

    /** SNBT of the item in the player's off hand, or null if offline / holding air. */
    public static String getOffhandItemNbt(String playerName) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return null;
        net.minecraft.item.ItemStack stack = p.getOffHandStack();
        if (stack.isEmpty()) return null;
        NbtCompound nbt = stack.hasNbt() ? stack.getNbt() : new NbtCompound();
        return nbt.toString();
    }

    /** Merge snbt into the NBT of the player's off-hand item. */
    public static boolean setOffhandItemNbt(String playerName, String snbt) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return false;
        net.minecraft.item.ItemStack stack = p.getOffHandStack();
        if (stack.isEmpty()) return false;
        try {
            NbtCompound nbt = StringNbtReader.parse(snbt);
            stack.setNbt(nbt);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    /** Full item stack at inventory slot: "item_id\tcount\tnbt_snbt", or null if empty/offline. */
    public static String getSlotItem(String playerName, int slot) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return null;
        PlayerInventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.size()) return null;
        net.minecraft.item.ItemStack stack = inv.getStack(slot);
        if (stack.isEmpty()) return null;
        String itemId = Registries.ITEM.getId(stack.getItem()).toString();
        int count = stack.getCount();
        String nbt = stack.hasNbt() ? stack.getNbt().toString() : "{}";
        return itemId + "\t" + count + "\t" + nbt;
    }

    /** Replace inventory slot; snbt merged into new item's NBT (pass "" for no NBT). */
    public static boolean setSlotItem(String playerName, int slot, String itemId, int count, String snbt) {
        ServerPlayerEntity p = playerByName(playerName);
        if (p == null) return false;
        PlayerInventory inv = p.getInventory();
        if (slot < 0 || slot >= inv.size()) return false;
        if (count <= 0) {
            inv.setStack(slot, net.minecraft.item.ItemStack.EMPTY);
            return true;
        }
        Identifier id = Identifier.tryParse(itemId);
        if (id == null || !Registries.ITEM.containsId(id)) return false;
        net.minecraft.item.ItemStack stack = new net.minecraft.item.ItemStack(Registries.ITEM.get(id), count);
        if (snbt != null && !snbt.isEmpty()) {
            try {
                NbtCompound nbt = StringNbtReader.parse(snbt);
                stack.setNbt(nbt);
            } catch (Exception ignored) {}
        }
        inv.setStack(slot, stack);
        return true;
    }

    public static int worldEntityCount(String dimension, String entityTypeId) {
        ServerWorld w = worldFor(dimension);
        if (w == null) return -1;
        Identifier id = Identifier.tryParse(entityTypeId);
        if (id == null || !Registries.ENTITY_TYPE.containsId(id)) return -1;
        EntityType<?> targetType = Registries.ENTITY_TYPE.get(id);
        int count = 0;
        for (Entity e : w.iterateEntities()) {
            if (e.getType() == targetType) count++;
        }
        return count;
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

    /** Typed command schemas: `name\tschema` per line (only typed commands). */
    public static native String nativeTypedCommandSchemas();

    /** Cancel-check for block break (before). Returns true = allow, false = cancel. */
    public static native boolean nativeOnBlockBreakPre(String player, String block, int x, int y, int z);

    /** Cancel-check for chat message (before). Returns true = allow, false = cancel. */
    public static native boolean nativeOnChatPre(String player, String message);

    /** Recipe JSONs: `namespace\tname\tJSON` per line. */
    public static native String nativeRecipeJsons();

    /** Get the JSON of a registered book by its id (e.g. "yog:example_guide"). */
    public static native String nativeBookJson(String bookId);

    // ── UI system ──────────────────────────────────────────────────────────

    /** Open a Rust-mod-defined UI screen. Called from Java. */
    public static native void nativeUIShow(String uiId, int screenW, int screenH);
    /** Hide / close the UI. Rust should stop rendering. */
    public static native void nativeUIHide(String uiId);
    /** Forward a mouse click to Rust. button: 0=left, 1=right, 2=middle. */
    public static native void nativeUIClick(String uiId, float mx, float my, int button);
    /** Forward a key event. action: 0=release, 1=press. */
    public static native void nativeUIKey(String uiId, int keyCode, int scanCode, int modifiers, int action);

    // (no native entry points needed for #4 — all calls are Rust→Java via JNI)
    /** Run a registered command; returns the reply (empty string if none). */
    public static native String nativeOnCommand(String name, String args, String source, String uuid);

    /** Declared custom items as `id\tmax_stack` lines. */
    public static native String nativeItemDefs();

    /** Declared custom blocks as `id\thardness\tresistance` lines. */
    public static native String nativeBlockDefs();


    /** Get the JSON of a registered book by its id (e.g. "yog:example_guide"). */
    public static native String nativeBookJson(String bookId);

    // (no native entry points needed for #4 — all calls are Rust→Java via JNI)

    public static native void nativeOnPacket(String channel, String player, byte[] payload);

    public static native void nativeOnClientPacket(String channel, byte[] payload);

    /** Server-receiver packet channels, one per line. */
    public static native String nativePacketChannels();

    /** Client-receiver packet channels, one per line. */
    public static native String nativeClientPacketChannels();

    /** Entity loaded into world — Post phase (observe only). */
    public static native void nativeOnEntitySpawn(String entityType, String uuid, String dimension);

    /** Entity loaded into world — Pre phase; return false to discard (cancel) it. */
    public static native boolean nativeOnEntitySpawnPre(String entityType, String uuid, String dimension);

    /** Entity about to take damage — Pre phase; return false to cancel the damage. */
    public static native boolean nativeOnEntityDamagePre(String entityType, String uuid, float amount, String source);

    /** Player placed a block — Pre phase; return false to cancel placement. */
    public static native boolean nativeOnPlaceBlockPre(String player, String block, int x, int y, int z);

    /** Player placed a block — Post phase (observe only). */
    public static native void nativeOnPlaceBlock(String player, String block, int x, int y, int z);

    /** Player about to die — Pre phase; return false to cancel death. */
    public static native boolean nativeOnPlayerDeathPre(String player, String uuid, String source);

    /** Player has died — Post phase (observe only). */
    public static native void nativeOnPlayerDeath(String player, String uuid, String source);

    /** Player respawned (Post only; no cancellation). */
    public static native void nativeOnPlayerRespawn(String player, String uuid, boolean atAnchor);

    /** Player earned an advancement (Post only; no cancellation). */
    public static native void nativeOnAdvancement(String player, String uuid, String advancement);

    /** Player right-clicked an entity — Pre phase; return false to cancel. */
    public static native boolean nativeOnEntityInteractPre(
            String player, String playerUuid,
            String entityType, String entityUuid, String hand);

    /** Player right-clicked an entity — Post phase (observe only). */
    public static native void nativeOnEntityInteract(
            String player, String playerUuid,
            String entityType, String entityUuid, String hand);

    /** Player took a crafted item from a crafting table (Post only; no cancellation). */
    public static native void nativeOnItemCraft(
            String player, String playerUuid, String resultItem, int resultCount);

    /** Explosion about to detonate — Pre phase; return false to cancel block damage. */
    public static native boolean nativeOnExplosionPre(
            String dimension, double x, double y, double z, float power, String causeUuid);

    /** Explosion detonated — Post phase (observe only). */
    public static native void nativeOnExplosion(
            String dimension, double x, double y, double z, float power, String causeUuid);

    // ── ABI minor 9 ──────────────────────────────────────────────────────────

    /** Player about to pick up an item — Pre phase; return false to prevent pickup. */
    public static native boolean nativeOnItemPickupPre(
            String player, String playerUuid, String itemId, int itemCount, String entityUuid);

    /** Player picked up an item — Post phase (observe only). */
    public static native void nativeOnItemPickup(
            String player, String playerUuid, String itemId, int itemCount, String entityUuid);

    /** Player sent a movement packet — Post only (high frequency). */
    public static native void nativeOnPlayerMove(
            String player, String playerUuid, double x, double y, double z, float yaw, float pitch);

    /** Player about to open a container — Pre phase; return false to prevent opening. */
    public static native boolean nativeOnContainerOpenPre(String player, String playerUuid);

    /** Player opened a container — Post phase; containerType is the screen handler registry id. */
    public static native void nativeOnContainerOpen(
            String player, String playerUuid, String containerType);

    /** Player closed a container — Post phase. */
    public static native void nativeOnContainerClose(String player, String playerUuid);

    /** Projectile about to hit — Pre phase; return false to cancel the hit. */
    public static native boolean nativeOnProjectileHitPre(
            String projectileType, String projectileUuid, String shooterUuid,
            String hitType, String hitEntityUuid,
            double x, double y, double z, String dimension);

    /** Projectile hit — Post phase. */
    public static native void nativeOnProjectileHit(
            String projectileType, String projectileUuid, String shooterUuid,
            String hitType, String hitEntityUuid,
            double x, double y, double z, String dimension);

    // ── ABI minor 11 — held item NBT native declarations ─────────────────────

    // (no native entry points — Rust calls Java via JNI, not the other way around)

    // ── ABI minor 10 — client-side hooks ─────────────────────────────────────

    /** Client tick — fired every tick on the render thread. */
    public static native void nativeOnClientTick();

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

    /** Key pressed/released/repeated — return false to cancel (suppress Minecraft processing). */
    public static native boolean nativeOnKeyPress(int keyCode, int scanCode, int action, int modifiers);

    /** A GUI screen was opened; screenClass is the simple class name (e.g. "InventoryScreen"). */
    public static native void nativeOnScreenOpen(String screenClass);

    /** A GUI screen was closed; screenClass is the simple class name. */
    public static native void nativeOnScreenClose(String screenClass);
}
