package dev.yog;

import dev.yog.bus.YogMirrors;
import dev.yog.bus.YogTypes;
import dev.yog.mirror.YogColumn;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.Map;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.core.SectionPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.monster.Enemy;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec3;

/**
 * The client's mirrors ({@code yog_mirror::host::CLIENT_*}): the entities
 * and the blocks this client sees, written after a client tick while
 * anyone reads them. A block's flags are worked out once a state, its rows
 * once a change of the boxes ({@link YogBlockBoxes}): a turn reads the
 * states and writes.
 */
final class YogClientMirrors {
    private YogClientMirrors() {}

    /** Rows by key: entities' (their network id). */
    private static final Map<Long, Integer> ENTITY_ROWS = new HashMap<>();
    private static final YogBlockBoxes BOXES = new YogBlockBoxes();
    /** What a state is to a body, by state (the game's own objects, one a state). */
    private static final Map<BlockState, Flags> FLAGS = new IdentityHashMap<>();
    /** The level the keys are of: another one starts them over. */
    private static ClientLevel keyed;

    private record Flags(int atom, long traits, float hardness) {}

    /** Answers the client mirrors' addresses (a client only). */
    static void provide() {
        YogBus.provide(YogBus.resolve(YogMirrors.ClientEntity.ADDRESS), YogClientMirrors::entities);
        YogBus.provide(YogBus.resolve(YogMirrors.ClientBlock.ADDRESS), YogClientMirrors::blocks);
    }

    private static int entities(YogMessage in, YogBus.Out out) {
        ClientLevel level = level();
        if (level == null) return YogBus.OK;
        YogTypes.Write w = YogTypes.Write.unpack(in.frame());
        Map<Long, Entity> live = new HashMap<>();
        for (Entity e : level.entitiesForRendering()) live.put(Integer.toUnsignedLong(e.getId()), e);
        YogHostMirrors.keep(YogMirrors.ClientEntity.SPACE, ENTITY_ROWS, live.keySet(), false, w.rows());
        YogHostMirrors.Due due = new YogHostMirrors.Due(w);
        for (Map.Entry<Long, Entity> e : live.entrySet()) {
            Integer row = ENTITY_ROWS.get(e.getKey());
            if (row != null) write(due, row, e.getValue());
        }
        due.end();
        return YogBus.OK;
    }

    private static void write(YogHostMirrors.Due due, int row, Entity e) {
        YogColumn c;
        if ((c = due.column(YogMirrors.ClientEntity.POS)) != null) c.putDoubles(row, e.getX(), e.getY(), e.getZ());
        if ((c = due.column(YogMirrors.ClientEntity.VELOCITY)) != null) {
            Vec3 v = e.getDeltaMovement();
            c.putDoubles(row, v.x, v.y, v.z);
        }
        if ((c = due.column(YogMirrors.ClientEntity.ROTATION)) != null) c.putFloats(row, e.getYRot(), e.getXRot());
        if ((c = due.column(YogMirrors.ClientEntity.KIND)) != null) {
            c.putInt(row, YogAtoms.number(BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString()));
        }
        if ((c = due.column(YogMirrors.ClientEntity.HEALTH)) != null) c.putFloat(row, e instanceof LivingEntity l ? l.getHealth() : 0f);
        if ((c = due.column(YogMirrors.ClientEntity.UUID)) != null) {
            c.putLongs(row, e.getUUID().getMostSignificantBits(), e.getUUID().getLeastSignificantBits());
        }
        if ((c = due.column(YogMirrors.ClientEntity.EYE_HEIGHT)) != null) c.putFloat(row, e.getEyeHeight());
        if ((c = due.column(YogMirrors.ClientEntity.HOSTILE)) != null) c.putByte(row, e instanceof Enemy);
        if ((c = due.column(YogMirrors.ClientEntity.NAME)) != null) {
            c.putInt(row, e instanceof Player p ? YogAtoms.number(p.getName().getString()) : 0);
        }
    }

    private static int blocks(YogMessage in, YogBus.Out out) {
        ClientLevel level = level();
        if (level == null) return YogBus.OK;
        YogTypes.Write w = YogTypes.Write.unpack(in.frame());
        BOXES.update(w);
        YogHostMirrors.Due due = new YogHostMirrors.Due(w);
        Columns c = new Columns(due);
        long[] at = BOXES.positions();
        int[] rows = BOXES.rows();
        BlockPos.MutableBlockPos p = new BlockPos.MutableBlockPos();
        for (int i = 0; i < at.length; i++) {
            if (rows[i] == YogBlockBoxes.NO_ROW) continue;
            p.set(at[i]);
            // Not loaded yet: no state, nothing known.
            Flags f = level.hasChunk(SectionPos.blockToSectionCoord(p.getX()), SectionPos.blockToSectionCoord(p.getZ()))
                    ? flags(level, p, level.getBlockState(p)) : null;
            c.write(rows[i], f);
        }
        due.end();
        return YogBus.OK;
    }

    private static Flags flags(ClientLevel level, BlockPos at, BlockState st) {
        Flags known = FLAGS.get(st);
        if (known != null) return known;
        Flags f = new Flags(YogHostBlocks.atom(st), YogBlockTraits.of(level, at, st), YogBlockTraits.hardness(level, at, st));
        FLAGS.put(st, f);
        return f;
    }

    /** A turn's block columns (those due). */
    private static final class Columns {
        private final YogColumn state, traits, hardness;

        Columns(YogHostMirrors.Due due) {
            state = due.column(YogMirrors.ClientBlock.STATE);
            traits = due.column(YogMirrors.ClientBlock.TRAITS);
            hardness = due.column(YogMirrors.ClientBlock.HARDNESS);
        }

        /** {@code f} at {@code row}; none, when null. */
        void write(int row, Flags f) {
            if (state != null) state.putInt(row, f == null ? 0 : f.atom());
            if (traits != null) traits.putLong(row, f == null ? 0L : f.traits());
            if (hardness != null) hardness.putFloat(row, f == null ? -1f : f.hardness());
        }
    }

    /** The client's level, the keys started over when it's another one. */
    private static ClientLevel level() {
        ClientLevel level = Minecraft.getInstance().level;
        if (level != keyed) {
            YogHostMirrors.forget(YogMirrors.ClientEntity.SPACE, ENTITY_ROWS);
            BOXES.forget(YogMirrors.ClientBlock.SPACE);
            keyed = level;
        }
        return level;
    }
}
