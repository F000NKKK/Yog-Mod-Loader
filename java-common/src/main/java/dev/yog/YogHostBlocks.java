package dev.yog;

import dev.yog.bus.YogMirrors;
import dev.yog.bus.YogTypes;
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

    /** The boxes, a dimension's each. */
    private static final Map<String, YogBlockBoxes> BOXES = new HashMap<>();
    /** A state's atom: states are the game's own objects, one a state. */
    private static final Map<BlockState, Integer> ATOMS = new IdentityHashMap<>();

    static int blocks(YogMessage in, YogBus.Out out) {
        if (YogHostMirrors.server() == null) return YogBus.OK;
        YogTypes.Write w = YogTypes.Write.unpack(in.frame());
        String dimension = w.mirror().substring(YogMirrors.Block.ADDRESS.length() + 1);
        ServerLevel level = NativeBridge.worldFor(dimension);
        if (level == null) return YogBus.OK;
        YogBlockBoxes boxes = BOXES.computeIfAbsent(w.spaceName(), s -> new YogBlockBoxes());
        boxes.update(w);
        YogHostMirrors.Due due = new YogHostMirrors.Due(w);
        YogColumn state = due.column(YogMirrors.Block.STATE);
        if (state != null) {
            long[] at = boxes.positions();
            int[] rows = boxes.rows();
            BlockPos.MutableBlockPos p = new BlockPos.MutableBlockPos();
            boxes.forEachDue(w, i -> state.putInt(rows[i], atom(level.getBlockState(p.set(at[i])))));
        }
        due.end();
        return YogBus.OK;
    }

    /** {@code state}'s atom ({@code id[props]}). */
    static int atom(BlockState state) {
        return ATOMS.computeIfAbsent(state, s -> YogAtoms.number(BlockStateParser.serialize(s)));
    }

    /** Every dimension's keys taken (another world). */
    static void forgetAll() {
        BOXES.forEach((space, boxes) -> boxes.forget(space));
        BOXES.clear();
    }
}
