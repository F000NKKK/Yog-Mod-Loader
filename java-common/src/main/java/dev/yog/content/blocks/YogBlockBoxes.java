package dev.yog.content.blocks;

import dev.yog.YogHostMirrors;
import dev.yog.YogLog;

import dev.yog.bus.YogTypes;
import dev.yog.data.YogFrame;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.IntConsumer;
import net.minecraft.core.BlockPos;

/**
 * A block mirror's boxes ({@link YogTypes.BlockScope}) as the positions
 * they hold and each one's row. The positions and the space's keys are
 * worked out again only when the boxes change: a turn with the same boxes
 * just walks the positions of the boxes due ({@link #forEachDue}) — each
 * box at its own declaration's rate.
 */
public final class YogBlockBoxes {
    /** The most blocks a mirror's boxes may hold: bigger boxes are skipped (logged). */
    private static final long MOST_BLOCKS = 1L << 18;
    /** Boxes that keep their own pace ({@code yog_mirror::SCOPE_PACES}); the rest are due every turn. */
    private static final int PACES = Long.SIZE;
    /** No row (the runtime gave the key none). */
    static final int NO_ROW = -1;

    /** The space's rows by packed position, as the runtime gave them. */
    private final Map<Long, Integer> byKey = new HashMap<>();
    private List<byte[]> scopes = List.of();
    private long[] positions = new long[0];
    private int[] rows = new int[0];
    /** Each box's positions, as indices into {@link #positions}. */
    private int[][] boxes = new int[0][];

    /** This turn's boxes made the mirror's: the space's keys and rows kept in step when they changed. */
    public void update(YogTypes.Write w) {
        List<byte[]> now = new ArrayList<>(w.scopes().size());
        for (YogTypes.Scope s : w.scopes()) now.add(s.bytes());
        if (same(now)) return;
        scopes = now;
        collect(now);
        Set<Long> live = new HashSet<>(positions.length * 2);
        for (long p : positions) live.add(p);
        YogHostMirrors.keep(w.spaceName(), byKey, live, false, w.rows());
        rows = new int[positions.length];
        for (int i = 0; i < positions.length; i++) rows[i] = byKey.getOrDefault(positions[i], NO_ROW);
    }

    /** The boxes' positions, packed ({@link BlockPos#asLong}). */
    public long[] positions() {
        return positions;
    }

    /** Each of {@link #positions}' rows ({@link #NO_ROW} when it has none). */
    public int[] rows() {
        return rows;
    }

    /** Each position (its index) of the boxes due in {@code w} — a position in two boxes maybe twice. */
    public void forEachDue(YogTypes.Write w, IntConsumer index) {
        for (int b = 0; b < boxes.length; b++) {
            if (b < PACES && (w.writing() >>> b & 1) == 0) continue;
            for (int i : boxes[b]) {
                if (rows[i] != NO_ROW) index.accept(i);
            }
        }
    }

    /** The keys given back ({@code space}'s): another world. */
    public void forget(String space) {
        YogHostMirrors.forget(space, byKey);
        scopes = List.of();
        positions = new long[0];
        rows = new int[0];
        boxes = new int[0][];
    }

    private boolean same(List<byte[]> now) {
        if (now.size() != scopes.size()) return false;
        for (int i = 0; i < now.size(); i++) {
            if (!Arrays.equals(now.get(i), scopes.get(i))) return false;
        }
        return true;
    }

    /** Every box's positions, each once, and each box's indices into them. */
    private void collect(List<byte[]> scopes) {
        Map<Long, Integer> index = new HashMap<>();
        List<Long> all = new ArrayList<>();
        boxes = new int[scopes.size()][];
        for (int b = 0; b < scopes.size(); b++) {
            YogTypes.BlockScope box = YogTypes.BlockScope.unpack(new YogFrame.Unpacker(scopes.get(b)));
            int x0 = Math.min(box.min().x(), box.max().x()), x1 = Math.max(box.min().x(), box.max().x());
            int y0 = Math.min(box.min().y(), box.max().y()), y1 = Math.max(box.min().y(), box.max().y());
            int z0 = Math.min(box.min().z(), box.max().z()), z1 = Math.max(box.min().z(), box.max().z());
            long volume = (long) (x1 - x0 + 1) * (y1 - y0 + 1) * (z1 - z0 + 1);
            if (all.size() + volume > MOST_BLOCKS) {
                YogLog.warn("a block mirror's box of " + volume + " blocks is skipped: " + MOST_BLOCKS + " at most");
                boxes[b] = new int[0];
                continue;
            }
            int[] mine = new int[(int) volume];
            int n = 0;
            for (int y = y0; y <= y1; y++) {
                for (int z = z0; z <= z1; z++) {
                    for (int x = x0; x <= x1; x++) {
                        long p = BlockPos.asLong(x, y, z);
                        Integer i = index.get(p);
                        if (i == null) {
                            i = all.size();
                            index.put(p, i);
                            all.add(p);
                        }
                        mine[n++] = i;
                    }
                }
            }
            boxes[b] = mine;
        }
        positions = all.stream().mapToLong(Long::longValue).toArray();
    }
}
