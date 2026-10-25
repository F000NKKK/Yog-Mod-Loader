package dev.yog;

import dev.yog.actions.YogQueries;
import dev.yog.client.YogClientMirrors;
import dev.yog.content.blocks.YogHostBlocks;

import dev.yog.bus.YogMirrors;
import dev.yog.bus.YogServices;
import dev.yog.bus.YogTypes;
import dev.yog.mirror.YogColumn;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.phys.Vec3;

/**
 * The game's own mirrors ({@code yog_mirror::host}, {@link YogMirrors}):
 * the runtime calls each one's address after a server tick while anyone
 * reads it, with the fields due. Each turn keeps the key space's keys (an
 * entity's network id) in step with the world, then writes what's due.
 */
public final class YogHostMirrors {
    private YogHostMirrors() {}

    /** Rows by key, as the runtime gave them: entities', players'. */
    private static final Map<Long, Integer> ENTITY_ROWS = new HashMap<>();
    private static final Map<Long, Integer> PLAYER_ROWS = new HashMap<>();
    /** The columns seen, by header address. */
    private static final Map<Long, YogColumn> COLUMNS = new HashMap<>();
    /** The server the keys are of: another one starts them over. */
    private static MinecraftServer keyed;

    /** Answers the host mirrors' addresses; before the mods register. */
    public static void provide() {
        YogBus.provide(YogBus.resolve(YogMirrors.Entity.ADDRESS), YogHostMirrors::entities);
        YogBus.provide(YogBus.resolve(YogMirrors.Player.ADDRESS), YogHostMirrors::players);
        YogBus.provide(YogBus.resolve(YogMirrors.Block.ADDRESS), YogHostBlocks::blocks);
        if (YogPlatform.isClient()) YogClientMirrors.provide();
    }

    private static int entities(YogMessage in, YogBus.Out out) {
        MinecraftServer s = server();
        if (s == null) return YogBus.OK;
        YogTypes.Write w = YogTypes.Write.unpack(in.frame());
        Map<Long, Entity> live = new HashMap<>();
        for (ServerLevel level : s.getAllLevels()) {
            for (Entity e : level.getAllEntities()) live.put(key(e), e);
        }
        keep(YogMirrors.Entity.SPACE, ENTITY_ROWS, live.keySet(), false, w.rows());
        Due due = new Due(w);
        for (Map.Entry<Long, Entity> e : live.entrySet()) {
            Integer row = ENTITY_ROWS.get(e.getKey());
            if (row != null) write(due, row, e.getValue());
        }
        due.end();
        return YogBus.OK;
    }

    private static int players(YogMessage in, YogBus.Out out) {
        MinecraftServer s = server();
        if (s == null) return YogBus.OK;
        YogTypes.Write w = YogTypes.Write.unpack(in.frame());
        Map<Long, ServerPlayer> live = new HashMap<>();
        for (ServerPlayer p : s.getPlayerList().getPlayers()) live.put(key(p), p);
        keep(YogMirrors.Player.SPACE, PLAYER_ROWS, live.keySet(), true, w.rows());
        Due due = new Due(w);
        YogColumn name = due.column(YogMirrors.Player.NAME);
        for (Map.Entry<Long, ServerPlayer> p : live.entrySet()) {
            Integer row = PLAYER_ROWS.get(p.getKey());
            if (row != null && name != null) name.putInt(row, YogAtoms.number(p.getValue().getName().getString()));
        }
        due.end();
        return YogBus.OK;
    }

    /** An entity's key: its network id, unsigned. */
    private static long key(Entity e) {
        return Integer.toUnsignedLong(e.getId());
    }

    /** The server, the keys started over when it's another one (a new world). */
    public static MinecraftServer server() {
        MinecraftServer s = NativeBridge.currentServer();
        if (s != keyed) {
            forget(YogMirrors.Entity.SPACE, ENTITY_ROWS);
            forget(YogMirrors.Player.SPACE, PLAYER_ROWS);
            YogHostBlocks.forgetAll();
            keyed = s;
        }
        return s;
    }

    public static void forget(String space, Map<Long, Integer> rows) {
        if (rows.isEmpty()) return;
        long[] take = rows.keySet().stream().mapToLong(Long::longValue).toArray();
        new YogServices.Mirror.Keys(space, List.of(), take).call();
        rows.clear();
    }

    /**
     * The space's keys made {@code live}'s: new ones put (a player's parent
     * key its entity's, the same id), gone ones taken; the mirror's rows
     * marked alike.
     */
    public static void keep(String space, Map<Long, Integer> rows, Set<Long> live, boolean parented, long rowsAt) {
        List<YogTypes.Key> put = new ArrayList<>();
        for (Long k : live) {
            if (!rows.containsKey(k)) put.add(new YogTypes.Key(k, parented ? k : null));
        }
        Set<Long> gone = new HashSet<>(rows.keySet());
        gone.removeAll(live);
        if (put.isEmpty() && gone.isEmpty()) return;
        long[] take = gone.stream().mapToLong(Long::longValue).toArray();
        long[] given = new YogServices.Mirror.Keys(space, put, take).call();
        YogColumn marked = columnAt(rowsAt);
        if (marked != null) marked.begin();
        for (Long k : gone) {
            Integer row = rows.remove(k);
            if (row != null && marked != null) marked.putBit(row, false);
        }
        for (int i = 0; i < put.size() && given != null && i < given.length; i++) {
            int row = (int) given[i];
            rows.put(put.get(i).key(), row);
            if (marked != null) marked.putBit(row, true);
        }
        if (marked != null) marked.end();
    }

    /** Entity {@code e}'s due fields at {@code row}. */
    private static void write(Due due, int row, Entity e) {
        YogColumn c;
        if ((c = due.column(YogMirrors.Entity.POS)) != null) c.putDoubles(row, e.getX(), e.getY(), e.getZ());
        if ((c = due.column(YogMirrors.Entity.VELOCITY)) != null) {
            Vec3 v = e.getDeltaMovement();
            c.putDoubles(row, v.x, v.y, v.z);
        }
        if ((c = due.column(YogMirrors.Entity.ROTATION)) != null) c.putFloats(row, e.getYRot(), e.getXRot());
        if ((c = due.column(YogMirrors.Entity.KIND)) != null) {
            c.putInt(row, YogAtoms.number(net.minecraft.world.entity.EntityType.getKey(e.getType()).toString()));
        }
        if ((c = due.column(YogMirrors.Entity.DIMENSION)) != null) c.putInt(row, YogAtoms.number(YogQueries.dimensionId(e)));
        if ((c = due.column(YogMirrors.Entity.HEALTH)) != null) c.putFloat(row, e instanceof LivingEntity l ? l.getHealth() : 0f);
        if ((c = due.column(YogMirrors.Entity.UUID)) != null) {
            c.putLongs(row, e.getUUID().getMostSignificantBits(), e.getUUID().getLeastSignificantBits());
        }
    }

    private static YogColumn columnAt(long at) {
        if (at == 0) return null;
        return COLUMNS.computeIfAbsent(at, YogColumn::at);
    }

    /** A turn's due fields: their columns, open for writing, by field index. */
    public static final class Due {
        private final YogColumn[] columns = new YogColumn[Long.SIZE];

        public Due(YogTypes.Write w) {
            int next = 0;
            for (int i = 0; i < Long.SIZE && next < w.columns().length; i++) {
                if ((w.fields() >>> i & 1) == 0) continue;
                YogColumn c = columnAt(w.columns()[next++]);
                if (c != null) c.begin();
                columns[i] = c;
            }
        }

        public YogColumn column(int field) {
            return columns[field];
        }

        public void end() {
            for (YogColumn c : columns) {
                if (c != null) c.end();
            }
        }
    }
}
