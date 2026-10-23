package dev.yog.actions;

import dev.yog.NativeBridge;
import dev.yog.YogLog;
import dev.yog.YogServiceHost;

import com.mojang.brigadier.exceptions.CommandSyntaxException;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.commands.arguments.blocks.BlockInput;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.Identifier;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.storage.loot.parameters.LootContextParamSets;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.storage.loot.LootParams;
import net.minecraft.world.level.storage.loot.LootTable;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;

/** The host's world actions: the methods {@code yog_event::world} names. */
public final class YogWorldActions {
    private YogWorldActions() {}

    /** Registry id of the block at (x,y,z) in `dimension`, or null. */
    @YogServiceHost
    public static String getBlock(String dimension, int x, int y, int z) {
        ServerLevel w = NativeBridge.worldFor(dimension);
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
    @YogServiceHost
    public static boolean setBlock(String dimension, int x, int y, int z, String blockId) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w != null && blockId != null && (blockId.indexOf('[') >= 0 || blockId.indexOf('{') >= 0)) {
            // Full /setblock syntax: id[prop=value,...]{nbt}
            try {
                BlockStateParser.BlockResult r =
                        BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, blockId, true);
                return new BlockInput(r.blockState(), r.properties().keySet(), r.nbt())
                        .place(w, new BlockPos(x, y, z), 3);
            } catch (CommandSyntaxException e) {
                YogLog.warn("set block " + blockId + ": " + e.getMessage() + " — placed by its id alone");
            }
        }
        Identifier id = Identifier.tryParse(blockId);
        if (w == null || id == null || !BuiltInRegistries.BLOCK.containsKey(id)) {
            return false;
        }
        Block block = BuiltInRegistries.BLOCK.getValue(id);
        return w.setBlockAndUpdate(new BlockPos(x, y, z), block.defaultBlockState());
    }

    /** Game time in ticks since world creation, or Long.MIN_VALUE if dimension unknown. */
    @YogServiceHost
    public static long worldTime(String dimension) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        return w == null ? Long.MIN_VALUE : w.getGameTime();
    }

    /** Set the time-of-day; returns false if the dimension is unknown. */
    @YogServiceHost
    public static boolean worldSetTime(String dimension, long time) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w == null) return false;
        // Since 26.1 time is a clock's, shared by the dimensions that keep it.
        var clock = w.dimensionType().defaultClock();
        if (clock.isEmpty()) return false;
        w.getServer().clockManager().setTotalTicks(clock.get(), time);
        return true;
    }

    /** Whether it is currently raining in the given dimension. */
    @YogServiceHost
    public static boolean worldIsRaining(String dimension) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        return w != null && w.isRaining();
    }

    /** Rain / clear spell length when the caller passes no duration (5 minutes). */
    private static final int DEFAULT_WEATHER_TICKS = 6000;

    /**
     * Start or stop rain. {@code durationTicks == 0} picks a server default.
     * Internally calls {@link MinecraftServer#setWeatherParameters} (weather is the server's since 26.1).
     * Signature: clearDuration, rainDuration, rain, thunder.
     */
    @YogServiceHost
    public static boolean worldSetWeather(String dimension, boolean raining, int durationTicks) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w == null) return false;
        int dur = durationTicks > 0 ? durationTicks : DEFAULT_WEATHER_TICKS;
        if (raining) {
            w.getServer().setWeatherParameters(0, dur, true, false);
        } else {
            w.getServer().setWeatherParameters(dur, 0, false, false);
        }
        return true;
    }

    @YogServiceHost
    public static boolean dropLoot(String tableId, String dimension, double x, double y, double z) {
        MinecraftServer s = NativeBridge.currentServer();
        if (s == null) return false;
        Identifier id = Identifier.tryParse(tableId);
        ServerLevel world = NativeBridge.worldFor(dimension);
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

    /** UUID of the nearest entity hit along from→to (ProjectileUtil semantics), or null. */
    @YogServiceHost
    public static String raycastEntity(String dim, double fx, double fy, double fz,
            double tx, double ty, double tz, String excludeUuid) {
        try {
            ServerLevel w = NativeBridge.worldFor(dim);
            if (w == null) return null;
            Entity ex = NativeBridge.optionalEntity(excludeUuid);
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

    @YogServiceHost
    public static boolean explode(String dim, double x, double y, double z, float power, boolean fire, String sourceUuid) {
        try {
            ServerLevel w = NativeBridge.worldFor(dim);
            if (w == null) return false;
            w.explode(NativeBridge.optionalEntity(sourceUuid), x, y, z, power, fire, Level.ExplosionInteraction.TNT);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }

    @YogServiceHost
    public static boolean worldIsThundering(String dimension) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        return w != null && w.isThundering();
    }

    /** Starts (or stops) a thunderstorm for {@code durationTicks} (0 = a default). */
    @YogServiceHost
    public static boolean worldSetThunder(String dimension, boolean thundering, int durationTicks) {
        ServerLevel w = NativeBridge.worldFor(dimension);
        if (w == null) return false;
        int dur = durationTicks > 0 ? durationTicks : DEFAULT_WEATHER_TICKS;
        w.getServer().setWeatherParameters(thundering ? 0 : dur, thundering ? dur : 0, thundering, thundering);
        return true;
    }

    @YogServiceHost
    public static boolean placeFeature(String dim, int x, int y, int z, String featureId) {
        try {
            ServerLevel w = NativeBridge.worldFor(dim);
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
}
