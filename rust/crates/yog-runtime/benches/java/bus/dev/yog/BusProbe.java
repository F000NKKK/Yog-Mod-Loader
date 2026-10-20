package dev.yog;

import dev.yog.bus.YogTypes;
import dev.yog.data.YogFrame;
import java.util.ArrayList;
import java.util.List;

/**
 * The bus benchmark's Java side: handlers answering the way the host's
 * generated {@code serve} does (a record read from the arena, a reply
 * packed into it), and Java calling a Rust provider in a loop.
 */
public final class BusProbe {
    private BusProbe() {}

    private static final int SLOTS = 41;
    private static final int AREA_SIDE = 32;

    /** Answers a point with itself moved: read, built, packed. */
    public static void providePoint(String address) {
        YogBus.provide(YogBus.resolve(address), (in, out) -> {
            YogTypes.Point p = YogTypes.Point.fromFrame(in);
            new YogTypes.Point(p.x() + 1, p.y(), p.z()).pack(out.frame());
            return YogBus.OK;
        });
    }

    /** Answers with an inventory's 41 slots. */
    public static void provideInventory(String address) {
        List<YogTypes.SlotStack> slots = new ArrayList<>();
        for (int k = 0; k < SLOTS; k++) slots.add(new YogTypes.SlotStack(k, new YogTypes.Stack("minecraft:item_" + k % 7, k % 64 + 1, "{}")));
        YogBus.provide(YogBus.resolve(address), (in, out) -> {
            YogFrame.Packer p = out.frame();
            int at = p.beginList(slots.size());
            for (YogTypes.SlotStack s : slots) s.pack(p);
            p.end(at);
            return YogBus.OK;
        });
    }

    /** Answers with a 32³ block area (three kinds). */
    public static void provideArea(String address) {
        int[] blocks = new int[AREA_SIDE * AREA_SIDE * AREA_SIDE];
        for (int k = 0; k < blocks.length; k++) blocks[k] = k % 3;
        List<YogTypes.BlockKind> palette = List.of(
                new YogTypes.BlockKind("minecraft:stone", true, false, false, false),
                new YogTypes.BlockKind("minecraft:air", false, false, false, false),
                new YogTypes.BlockKind("minecraft:dirt", true, false, false, false));
        YogTypes.BlockArea area = new YogTypes.BlockArea(new YogTypes.BlockPos(0, 0, 0), new YogTypes.BlockPos(AREA_SIDE - 1, AREA_SIDE - 1, AREA_SIDE - 1), palette, blocks);
        YogBus.provide(YogBus.resolve(address), (in, out) -> {
            area.pack(out.frame());
            return YogBus.OK;
        });
    }

    /** Calls {@code address} (a Rust provider) {@code n} times with a point, reading each reply: their x summed. */
    public static double callPoints(String address, long n) {
        int to = YogBus.resolve(address);
        double sum = 0;
        for (long k = 0; k < n; k++) {
            YogMessage r = YogBus.call(to, new YogTypes.Point(k, 64, -3).packedFrame());
            sum += YogTypes.Point.fromFrame(r).x();
        }
        return sum;
    }
}
