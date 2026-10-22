package dev.yog;

import dev.yog.bus.YogMirrors;
import dev.yog.bus.YogTypes;
import dev.yog.data.YogFrame;
import dev.yog.mirror.YogColumn;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.Map;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.state.BlockState;

/**
 * The host's block mirror ({@code yog:mirror/yog/block/<dimension>}): the
 * blocks of the boxes its declarations asked for ({@link YogTypes.BlockScope}),
 * keyed by packed position in their dimension's key space, a block's state
 * written as an atom.
 */
final class YogHostBlocks {
    private YogHostBlocks() {}

    /** The most blocks a box may hold: a bigger one is skipped (logged). */
    private static final long MOST_BLOCKS = 1L << 18;

    /** Rows by key, a dimension's each. */
    private static final Map<String, Map<Long, Integer>> ROWS = new HashMap<>();
    /** A state's atom: states are the game's own objects, one a state. */
    private static final Map<BlockState, Integer> ATOMS = new IdentityHashMap<>();

    static int blocks(YogMessage in, YogBus.Out out) {
        if (YogHostMirrors.server() == null) return YogBus.OK;
        YogTypes.Write w = YogTypes.Write.unpack(in.frame());
        String dimension = w.mirror().substring(YogMirrors.Block.ADDRESS.length() + 1);
        ServerLevel level = NativeBridge.worldFor(dimension);
        if (level == null) return YogBus.OK;
        Map<Long, BlockPos> live = new HashMap<>();
        for (YogTypes.Scope s : w.scopes()) {
            YogTypes.BlockScope box = YogTypes.BlockScope.unpack(new YogFrame.Unpacker(s.bytes()));
            collect(box, live);
        }
        String space = YogMirrors.Block.SPACE + "/" + dimension;
        Map<Long, Integer> rows = ROWS.computeIfAbsent(dimension, d -> new HashMap<>());
        YogHostMirrors.keep(space, rows, live.keySet(), false, w.rows());
        YogHostMirrors.Due due = new YogHostMirrors.Due(w);
        YogColumn state = due.column(YogMirrors.Block.STATE);
        if (state != null) {
            for (Map.Entry<Long, BlockPos> b : live.entrySet()) {
                Integer row = rows.get(b.getKey());
                if (row != null) state.putInt(row, atom(level.getBlockState(b.getValue())));
            }
        }
        due.end();
        return YogBus.OK;
    }

    /** The positions of {@code box} into {@code live}, by packed position. */
    private static void collect(YogTypes.BlockScope box, Map<Long, BlockPos> live) {
        int x0 = Math.min(box.min().x(), box.max().x()), x1 = Math.max(box.min().x(), box.max().x());
        int y0 = Math.min(box.min().y(), box.max().y()), y1 = Math.max(box.min().y(), box.max().y());
        int z0 = Math.min(box.min().z(), box.max().z()), z1 = Math.max(box.min().z(), box.max().z());
        long volume = (long) (x1 - x0 + 1) * (y1 - y0 + 1) * (z1 - z0 + 1);
        if (volume > MOST_BLOCKS) {
            YogLog.warn("a block mirror's box of " + volume + " blocks is skipped: " + MOST_BLOCKS + " at most");
            return;
        }
        for (int x = x0; x <= x1; x++) {
            for (int y = y0; y <= y1; y++) {
                for (int z = z0; z <= z1; z++) {
                    BlockPos p = new BlockPos(x, y, z);
                    live.put(p.asLong(), p);
                }
            }
        }
    }

    private static int atom(BlockState state) {
        return ATOMS.computeIfAbsent(state, s -> YogAtoms.number(BlockStateParser.serialize(s)));
    }

    /** Every dimension's keys taken (another world). */
    static void forgetAll() {
        for (Map.Entry<String, Map<Long, Integer>> d : ROWS.entrySet()) {
            YogHostMirrors.forget(YogMirrors.Block.SPACE + "/" + d.getKey(), d.getValue());
        }
        ROWS.clear();
    }
}
