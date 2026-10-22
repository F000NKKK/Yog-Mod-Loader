package dev.yog.mirror;

import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

/**
 * A mirror's column ({@code yog_abi::YogColumn}) written by the host, its
 * one writer: {@link #begin} makes its seqlock odd (readers retry), the
 * values go in, {@link #end} makes it even again. Its memory is looked at
 * again when it grew (its generation moved).
 */
public final class YogColumn {
    /** The header's layout ({@code YogColumn}: seq, width, capacity, generation, data). */
    private static final int SEQ = 0;
    private static final int WIDTH = 4;
    private static final int CAPACITY = 8;
    private static final int GENERATION = 12;
    private static final int DATA = 16;
    private static final int HEADER_BYTES = 24;
    /** Bits a word of a bit column holds. */
    private static final int WORD_BITS = Long.SIZE;
    private static final int WORD_SHIFT = 6;
    private static final int WORD_MASK = WORD_BITS - 1;
    /** Bytes of the numbers a row holds. */
    private static final int F64 = Double.BYTES;
    private static final int F32 = Float.BYTES;
    private static final int I64 = Long.BYTES;

    private static final VarHandle INT = MethodHandles.byteBufferViewVarHandle(int[].class, ByteOrder.LITTLE_ENDIAN);

    private final ByteBuffer header;
    private int width;
    private ByteBuffer data;
    private int capacity;
    private int generation = -1;
    private int began;

    private YogColumn(ByteBuffer header) {
        this.header = header;
        this.width = header.getInt(WIDTH);
    }

    /** The column whose header is at {@code address}; null for none. */
    public static YogColumn at(long address) {
        ByteBuffer h = YogMirrorMemory.buffer(address, HEADER_BYTES);
        return h == null ? null : new YogColumn(h);
    }

    /** Starts writing: readers retry until {@link #end}. */
    public void begin() {
        began = (int) INT.get(header, SEQ);
        INT.set(header, SEQ, began + 1);
        VarHandle.releaseFence();
        int g = (int) INT.getAcquire(header, GENERATION);
        // A column made again where a freed one was has another width.
        if (g != generation || width != header.getInt(WIDTH)) {
            width = header.getInt(WIDTH);
            capacity = header.getInt(CAPACITY);
            data = YogMirrorMemory.buffer(header.getLong(DATA), (long) capacity * width);
            generation = g;
        }
    }

    /** Ends the write {@link #begin} started. */
    public void end() {
        INT.setRelease(header, SEQ, began + 2);
    }

    private boolean has(int row) {
        return data != null && row >= 0 && row < capacity;
    }

    public void putDoubles(int row, double a, double b, double c) {
        if (!has(row)) return;
        int at = row * width;
        data.putDouble(at, a).putDouble(at + F64, b).putDouble(at + 2 * F64, c);
    }

    public void putFloats(int row, float a, float b) {
        if (!has(row)) return;
        int at = row * width;
        data.putFloat(at, a).putFloat(at + F32, b);
    }

    public void putFloat(int row, float v) {
        if (has(row)) data.putFloat(row * width, v);
    }

    public void putInt(int row, int v) {
        if (has(row)) data.putInt(row * width, v);
    }

    public void putLongs(int row, long a, long b) {
        if (!has(row)) return;
        int at = row * width;
        data.putLong(at, a).putLong(at + I64, b);
    }

    /** Sets bit {@code bit} of a bit column (a word a row). */
    public void putBit(int bit, boolean on) {
        int word = bit >>> WORD_SHIFT;
        if (!has(word)) return;
        int at = word * width;
        long mask = 1L << (bit & WORD_MASK);
        long w = data.getLong(at);
        data.putLong(at, on ? w | mask : w & ~mask);
    }
}
