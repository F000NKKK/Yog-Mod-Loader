package dev.yog.data;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Supplier;

/**
 * Frames: Yog Data packed for the bus (the Rust crate {@code yog-data},
 * module {@code frame}) — both sides know the schema, so nothing says what
 * a value is or what it's called. A record's fields go in order after its
 * length (a reader that knows fewer skips the rest, one that knows more
 * takes defaults); strings go in a table at the end, each once. The
 * generated records pack and unpack themselves through this.
 */
public final class YogFrame {
    private YogFrame() {}

    /** A varint byte's seven bits, the bit saying more follow, and their shift. */
    private static final int BITS = 0x7F;
    private static final int MORE = 0x80;
    private static final int SHIFT = 7;
    /** The longest varint, in bits. */
    private static final int VARINT_MAX_SHIFT = 64;
    /** A record's length reserved before its fields: one byte, the common case. */
    private static final int ONE_BYTE_MAX = 0x7F;
    /** Bits in a byte, a {@code long}, and a 32-bit float's bytes, a 64-bit one's. */
    private static final int BYTE_BITS = 8;
    private static final int LONG_BITS = 64;
    private static final int F32_BYTES = 4;
    private static final int F64_BYTES = 8;
    /** Nested deeper than this is malformed. */
    private static final int MAX_DEPTH = YogData.MAX_DEPTH;
    /** What a writer reserves to start with. */
    private static final int START_BYTES = 256;

    /** {@code n} zigzagged: small negative numbers stay small. */
    static long zigzag(long n) {
        return (n << 1) ^ (n >> (LONG_BITS - 1));
    }

    static long unzigzag(long z) {
        return (z >>> 1) ^ -(z & 1);
    }

    /** A list of {@code item}s read one after another. */
    public static <T> List<T> list(Unpacker u, Supplier<T> item) {
        int n = u.count();
        List<T> out = new ArrayList<>(n);
        for (int k = 0; k < n; k++) out.add(item.get());
        return out;
    }

    /** This thread's packers, a stack: a call nested in a handler takes the next one. */
    private static final class Pool {
        private static final int START = 4;
        private Packer[] stack = new Packer[START];
        private int depth;
    }

    private static final ThreadLocal<Pool> POOL = ThreadLocal.withInitial(Pool::new);

    /** Writes a frame. */
    public static final class Packer {
        private byte[] out = new byte[START_BYTES];
        private int n;
        private byte[] table = new byte[START_BYTES];
        private int tableLen;
        private final Map<String, Integer> strings = new HashMap<>();
        /** Where in this thread's pool it was taken; -1 when it isn't pooled. */
        private int level = -1;

        /**
         * A packer of this thread's, emptied — no allocation once the thread
         * has made a few; give it back with {@link #release}.
         */
        public static Packer take() {
            Pool pool = POOL.get();
            if (pool.depth == pool.stack.length) pool.stack = Arrays.copyOf(pool.stack, pool.stack.length * 2);
            Packer p = pool.stack[pool.depth];
            if (p == null) {
                p = new Packer();
                pool.stack[pool.depth] = p;
            }
            p.reset();
            p.level = pool.depth++;
            return p;
        }

        /** Gives a {@link #take}n packer back, with every one taken after it. */
        public void release() {
            if (level >= 0) POOL.get().depth = level;
        }

        /** Empty again, its buffers kept. */
        public void reset() {
            n = 0;
            tableLen = 0;
            strings.clear();
        }

        /** Nothing packed. */
        public boolean isEmpty() {
            return n == 0 && tableLen == 0;
        }

        /** The frame's length: what {@link #finish} would make. */
        public int size() {
            return n + tableLen + varintLength(tableLen);
        }

        /** Writes the frame into {@code dst} at {@code at}, which has {@link #size} bytes free. */
        public void writeTo(ByteBuffer dst, int at) {
            dst.put(at, out, 0, n);
            dst.put(at + n, table, 0, tableLen);
            long len = tableLen;
            int end = at + size();
            // The table's length backwards: read from the frame's end.
            while (true) {
                int more = (len & ~(long) BITS) != 0 ? MORE : 0;
                dst.put(--end, (byte) ((len & BITS) | more));
                len >>>= SHIFT;
                if (more == 0) break;
            }
        }

        private void put(int b) {
            if (n == out.length) out = Arrays.copyOf(out, out.length * 2);
            out[n++] = (byte) b;
        }

        private void putAll(byte[] b) {
            if (n + b.length > out.length) out = Arrays.copyOf(out, Math.max(out.length * 2, n + b.length));
            System.arraycopy(b, 0, out, n, b.length);
            n += b.length;
        }

        private void tablePut(int b) {
            if (tableLen == table.length) table = Arrays.copyOf(table, table.length * 2);
            table[tableLen++] = (byte) b;
        }

        public void putU(long v) {
            while ((v & ~(long) BITS) != 0) {
                put((int) (v & BITS) | MORE);
                v >>>= SHIFT;
            }
            put((int) v);
        }

        public void putI(long v) {
            putU(zigzag(v));
        }

        public void putBool(boolean b) {
            put(b ? 1 : 0);
        }

        private void putFixed(long bits, int bytes) {
            for (int k = 0; k < bytes; k++) put((int) (bits >>> (BYTE_BITS * k)));
        }

        public void putF64(double f) {
            putFixed(Double.doubleToRawLongBits(f), F64_BYTES);
        }

        public void putF32(float f) {
            putFixed(Float.floatToRawIntBits(f) & 0xFFFFFFFFL, F32_BYTES);
        }

        /** Floats: four bytes each when every one is a {@code float} exactly. */
        public void putF64s(List<? extends Number> items) {
            putU(items.size());
            boolean narrow = true;
            for (Number x : items) {
                double d = x.doubleValue();
                if (Double.doubleToRawLongBits((double) (float) d) != Double.doubleToRawLongBits(d)) {
                    narrow = false;
                    break;
                }
            }
            putBool(narrow);
            for (Number x : items) {
                if (narrow) putF32((float) x.doubleValue());
                else putF64(x.doubleValue());
            }
        }

        public void putF32s(List<? extends Number> items) {
            putU(items.size());
            for (Number x : items) putF32(x.floatValue());
        }

        /** Numbers bit-packed: as many bits each as the biggest needs (one at least). */
        public void putUints(List<? extends Number> items, boolean signed) {
            putU(items.size());
            long max = 0;
            for (Number x : items) {
                long v = signed ? zigzag(x.longValue()) : x.longValue();
                if (Long.compareUnsigned(v, max) > 0) max = v;
            }
            int bits = Math.max(1, LONG_BITS - Long.numberOfLeadingZeros(max));
            put(bits);
            long acc = 0;
            int have = 0;
            for (Number x : items) {
                long v = signed ? zigzag(x.longValue()) : x.longValue();
                int left = bits;
                while (left > 0) {
                    int take = Math.min(left, BYTE_BITS - have);
                    acc |= (v & ((1L << take) - 1)) << have;
                    have += take;
                    v >>>= take;
                    left -= take;
                    if (have == BYTE_BITS) {
                        put((int) acc);
                        acc = 0;
                        have = 0;
                    }
                }
            }
            if (have > 0) put((int) acc);
        }

        /** A string: its index in the table, which gets it the first time. */
        public void putStr(String s) {
            Integer i = strings.get(s);
            if (i == null) {
                i = strings.size();
                strings.put(s, i);
                byte[] utf8 = s.getBytes(StandardCharsets.UTF_8);
                long len = utf8.length;
                while ((len & ~(long) BITS) != 0) {
                    tablePut((int) (len & BITS) | MORE);
                    len >>>= SHIFT;
                }
                tablePut((int) len);
                for (byte b : utf8) tablePut(b);
            }
            putU(i);
        }

        /** A name that repeats: its atom's number ({@link dev.yog.YogAtoms}). */
        public void putAtom(String name) {
            putU(dev.yog.YogAtoms.number(name));
        }

        public void putBytes(byte[] b) {
            putU(b.length);
            putAll(b);
        }

        /** A value it doesn't pack, in its binary form. */
        public void putTagged(Object value) {
            putBytes(YogData.encodeBody(value));
        }

        /** A record starts: where its length goes. */
        public int begin() {
            put(0);
            return n - 1;
        }

        /** The record begun at {@code at} ends: its length written, its fields moved if it needs more than a byte. */
        public void end(int at) {
            int len = n - at - 1;
            if (len <= ONE_BYTE_MAX) {
                out[at] = (byte) len;
                return;
            }
            byte[] v = varint(len);
            int extra = v.length - 1;
            if (n + extra > out.length) out = Arrays.copyOf(out, Math.max(out.length * 2, n + extra));
            System.arraycopy(out, at + 1, out, at + 1 + extra, len);
            System.arraycopy(v, 0, out, at, v.length);
            n += extra;
        }

        /** The frame: body, table, the table's length written backwards. */
        public byte[] finish() {
            byte[] frame = new byte[size()];
            writeTo(ByteBuffer.wrap(frame), 0);
            return frame;
        }

        private static int varintLength(long v) {
            int k = 1;
            while ((v & ~(long) BITS) != 0) {
                v >>>= SHIFT;
                k++;
            }
            return k;
        }

        private static byte[] varint(long v) {
            byte[] b = new byte[(LONG_BITS + SHIFT - 1) / SHIFT];
            int k = 0;
            while ((v & ~(long) BITS) != 0) {
                b[k++] = (byte) ((v & BITS) | MORE);
                v >>>= SHIFT;
            }
            b[k++] = (byte) v;
            return Arrays.copyOf(b, k);
        }
    }

    /**
     * Reads a frame where it is — an array, or the bus's arena (a direct
     * buffer): numbers read in place, nothing copied but strings and bytes
     * taken out. Throws {@link YogData.DecodeException} on bytes that aren't one.
     */
    public static final class Unpacker {
        private final ByteBuffer b;
        private int i;
        private final int bodyEnd;
        private final int frameEnd;
        private int tableAt;
        /** The strings read out of the table so far, made when the first is. */
        private List<String> strings;
        /** A string's bytes out of a direct buffer, kept for the next. */
        private byte[] scratch;
        private int depth;

        public Unpacker(byte[] bytes) {
            this(ByteBuffer.wrap(bytes), 0, bytes.length);
        }

        /** The frame in {@code buf} from {@code from} up to {@code to}. */
        public Unpacker(ByteBuffer buf, int from, int to) {
            this.b = buf;
            this.i = from;
            this.frameEnd = to;
            long tableLen = 0;
            int shift = 0;
            int at = to;
            while (true) {
                if (at == from || shift >= VARINT_MAX_SHIFT) throw new YogData.DecodeException("no frame trailer", at);
                int x = buf.get(--at) & 0xFF;
                tableLen |= (long) (x & BITS) << shift;
                if ((x & MORE) == 0) break;
                shift += SHIFT;
            }
            if (tableLen < 0 || tableLen > at - from) throw new YogData.DecodeException("table past the start", at);
            this.bodyEnd = at - (int) tableLen;
            this.tableAt = bodyEnd;
        }

        /** Throws unless the body is read through. */
        public void finish() {
            if (i != bodyEnd) throw new YogData.DecodeException("bytes after the value", i);
        }

        private int u8(int limit) {
            if (i >= limit) throw new YogData.DecodeException("unexpected end", i);
            return b.get(i++) & 0xFF;
        }

        private long varintAt(boolean table) {
            long v = 0;
            for (int shift = 0; shift < VARINT_MAX_SHIFT; shift += SHIFT) {
                int x;
                if (table) {
                    if (tableAt >= frameEnd) throw new YogData.DecodeException("unexpected end", tableAt);
                    x = b.get(tableAt++) & 0xFF;
                } else {
                    x = u8(bodyEnd);
                }
                v |= (long) (x & BITS) << shift;
                if ((x & MORE) == 0) return v;
            }
            throw new YogData.DecodeException("varint longer than 64 bits", i);
        }

        public long getU() {
            return varintAt(false);
        }

        public long getI() {
            return unzigzag(getU());
        }

        public boolean getBool() {
            int x = u8(bodyEnd);
            if (x > 1) throw new YogData.DecodeException("not a bool: " + x, i - 1);
            return x == 1;
        }

        private long fixed(int bytes) {
            if (i + bytes > bodyEnd) throw new YogData.DecodeException("unexpected end", i);
            long v = 0;
            for (int k = 0; k < bytes; k++) v |= (long) (b.get(i + k) & 0xFF) << (BYTE_BITS * k);
            i += bytes;
            return v;
        }

        public double getF64() {
            return Double.longBitsToDouble(fixed(F64_BYTES));
        }

        public float getF32() {
            return Float.intBitsToFloat((int) fixed(F32_BYTES));
        }

        /** A count of things a byte each at least: no more than the bytes left. */
        public int count() {
            long n = getU();
            if (n < 0 || n > bodyEnd - i) throw new YogData.DecodeException("count " + n + " past the end", i);
            return (int) n;
        }

        public List<Double> getF64s() {
            int n = count();
            boolean narrow = getBool();
            List<Double> out = new ArrayList<>(n);
            for (int k = 0; k < n; k++) out.add(narrow ? (double) getF32() : getF64());
            return out;
        }

        public List<Float> getF32s() {
            int n = count();
            List<Float> out = new ArrayList<>(n);
            for (int k = 0; k < n; k++) out.add(getF32());
            return out;
        }

        /** Bit-packed numbers ({@link Packer#putUints}). */
        public List<Long> getUints(boolean signed) {
            long n = getU();
            int bits = u8(bodyEnd);
            if (bits < 1 || bits > LONG_BITS || n < 0 || n > (long) (bodyEnd - i) * BYTE_BITS / bits) {
                throw new YogData.DecodeException("packed numbers past the end", i);
            }
            List<Long> out = new ArrayList<>((int) n);
            long acc = 0;
            int have = 0;
            for (long k = 0; k < n; k++) {
                long v = 0;
                int got = 0;
                while (got < bits) {
                    if (have == 0) {
                        acc = u8(bodyEnd);
                        have = BYTE_BITS;
                    }
                    int take = Math.min(bits - got, have);
                    v |= (acc & ((1L << take) - 1)) << got;
                    acc >>>= take;
                    have -= take;
                    got += take;
                }
                out.add(signed ? unzigzag(v) : v);
            }
            return out;
        }

        /** A string: the table read as far as it's needed. */
        public String getStr() {
            long index = getU();
            if (strings == null) strings = new ArrayList<>();
            while (strings.size() <= index) {
                long len = varintAt(true);
                if (len < 0 || len > frameEnd - tableAt) throw new YogData.DecodeException("string past the end", tableAt);
                strings.add(utf8(tableAt, (int) len));
                tableAt += (int) len;
            }
            return strings.get((int) index);
        }

        private String utf8(int at, int len) {
            if (b.hasArray()) return new String(b.array(), b.arrayOffset() + at, len, StandardCharsets.UTF_8);
            if (scratch == null || scratch.length < len) scratch = new byte[Math.max(len, START_BYTES)];
            b.get(at, scratch, 0, len);
            return new String(scratch, 0, len, StandardCharsets.UTF_8);
        }

        /** A name sent as its atom's number. */
        public String getAtom() {
            long n = getU();
            String name = n <= Integer.MAX_VALUE ? dev.yog.YogAtoms.name((int) n) : null;
            if (name == null) throw new YogData.DecodeException("no atom " + n, i);
            return name;
        }

        public byte[] getBytes() {
            int len = count();
            byte[] out = new byte[len];
            b.get(i, out);
            i += len;
            return out;
        }

        /** A value held in its binary form. */
        public Object getTagged() {
            return YogData.decodeBody(getBytes());
        }

        /** A record starts: where it ends. */
        public int begin() {
            int len = count();
            if (++depth > MAX_DEPTH) throw new YogData.DecodeException("nested deeper than " + MAX_DEPTH + " levels", i);
            return i + len;
        }

        public boolean more(int end) {
            return i < end;
        }

        /** The record ending at {@code end} is done: fields this reader doesn't know skipped. */
        public void leave(int end) {
            depth--;
            if (i > end) throw new YogData.DecodeException("a record read past its end", i);
            i = end;
        }
    }
}
