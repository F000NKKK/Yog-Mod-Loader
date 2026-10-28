package dev.yog.data;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import substrate.data.Binary;
import substrate.data.Bool;
import substrate.data.Bytes;
import substrate.data.Int;
import substrate.data.ListValue;
import substrate.data.MapValue;
import substrate.data.Null;
import substrate.data.Ref;
import substrate.data.Str;
import substrate.data.Text;
import substrate.data.Value;

/**
 * Yog Data as the hosts hold it: Substrate Data's binary form (its Java port,
 * {@code substrate.data}, reads and writes it) with plain Java values. Pure
 * Java, no Minecraft types — every host shares it.
 *
 * <p>Values decode to: {@code null}, {@link Boolean}, {@link Long},
 * {@link Double}, {@link String} (as spelled: plain text starting with
 * {@code @} has it doubled), {@link List}, {@link Map} (insertion ordered),
 * {@link Ref} and {@code byte[]}. {@link #encode} takes the same (plus
 * {@link Integer}, {@link Short}, {@link Byte}, {@link Float}) and writes
 * what the Rust encoder writes for the same value.
 */
public final class YogData {
    private YogData() {}

    /** Nested deeper than this is malformed. */
    public static final int MAX_DEPTH = Binary.MAX_DEPTH;

    /** What a typed value's spelling starts with ({@code @item:x}). */
    private static final String TYPED = dev.yog.YogNames.Sep.TYPED;

    /** Malformed binary data. */
    public static final class DecodeException extends RuntimeException {
        public final int offset;

        DecodeException(substrate.data.DecodeException cause) {
            super(cause.getMessage(), cause);
            this.offset = cause.offset();
        }

        DecodeException(String message, int offset) {
            super("byte " + offset + ": " + message);
            this.offset = offset;
        }
    }

    // ── The binary form ──────────────────────────────────────────────────

    /** Reads the binary form; throws {@link DecodeException} on malformed bytes. */
    public static Object decode(byte[] bytes) {
        try {
            return plain(Binary.decode(bytes));
        } catch (substrate.data.DecodeException e) {
            throw new DecodeException(e);
        }
    }

    /** A value's binary form without the header: what a frame holds of a value it doesn't pack ({@link YogFrame}). */
    public static Object decodeBody(byte[] bytes) {
        try {
            return plain(Binary.decodeBody(bytes));
        } catch (substrate.data.DecodeException e) {
            throw new DecodeException(e);
        }
    }

    /** {@code value} in the binary form. */
    public static byte[] encode(Object value) {
        return Binary.encode(value(value));
    }

    /** {@code value}'s binary form without the header ({@link #decodeBody}). */
    public static byte[] encodeBody(Object value) {
        return Binary.encodeBody(value(value));
    }

    /** {@code v} (Substrate's model) as a plain Java value. */
    private static Object plain(Value v) {
        if (v instanceof Bool b) return b.value();
        if (v instanceof Int i) return i.value();
        if (v instanceof substrate.data.Float f) return f.value();
        if (v instanceof Str s) return s.value();
        if (v instanceof Bytes b) return b.data();
        if (v instanceof Ref r) return r;
        if (v instanceof ListValue l) {
            List<Object> out = new ArrayList<>(l.items().size());
            for (Value item : l.items()) out.add(plain(item));
            return out;
        }
        if (v instanceof MapValue m) {
            Map<String, Object> out = new LinkedHashMap<>(m.entries().size() * 2);
            for (MapValue.Entry e : m.entries()) out.put(e.key(), plain(e.value()));
            return out;
        }
        return null;
    }

    /** Plain Java value {@code v} in Substrate's model. */
    private static Value value(Object v) {
        if (v == null) return Null.INSTANCE;
        if (v instanceof Boolean b) return new Bool(b);
        if (v instanceof Long || v instanceof Integer || v instanceof Short || v instanceof Byte) {
            return new Int(((Number) v).longValue());
        }
        if (v instanceof Double || v instanceof Float) return new substrate.data.Float(((Number) v).doubleValue());
        if (v instanceof String s) return new Str(s);
        if (v instanceof byte[] b) return new Bytes(b);
        if (v instanceof Ref r) return r;
        if (v instanceof List<?> list) {
            List<Value> items = new ArrayList<>(list.size());
            for (Object item : list) items.add(value(item));
            return new ListValue(items);
        }
        if (v instanceof Map<?, ?> map) {
            List<MapValue.Entry> entries = new ArrayList<>(map.size());
            for (Map.Entry<?, ?> e : map.entrySet()) entries.add(new MapValue.Entry((String) e.getKey(), value(e.getValue())));
            return new MapValue(entries);
        }
        throw new IllegalArgumentException("not a Yog Data value: " + v.getClass().getName());
    }

    // ── JSON ─────────────────────────────────────────────────────────────

    /** {@code v} as JSON text (Substrate's text form, compact): a reference as its spelling. */
    public static String toJson(Object v) {
        return Text.write(value(v));
    }

    // ── Reading values ───────────────────────────────────────────────────

    /** {@code v} as a map, or an empty one. */
    @SuppressWarnings("unchecked") // a decoded map: its keys are always strings
    public static Map<String, Object> map(Object v) {
        return v instanceof Map<?, ?> m ? (Map<String, Object>) m : Collections.emptyMap();
    }

    /** {@code v} as a list, or an empty one. */
    @SuppressWarnings("unchecked") // a decoded list: read-only, its items any value
    public static List<Object> list(Object v) {
        return v instanceof List<?> l ? (List<Object>) l : Collections.emptyList();
    }

    /** {@code v} as a string — a string's text (stored as spelled: a leading
     * {@code @} doubled), an image reference's source — or {@code def}. */
    public static String str(Object v, String def) {
        if (v instanceof String s) return s.startsWith(TYPED + TYPED) ? s.substring(TYPED.length()) : s;
        if (v instanceof Ref r && r.kind().equals(dev.yog.YogNames.Typed.IMAGE)) return r.id();
        return def;
    }

    /** The id a reference names ({@code @effect:mymod:clarity} → {@code mymod:clarity}), or {@code v} as a string. */
    public static String id(Object v) {
        return v instanceof Ref r ? r.id() : str(v, "");
    }

    public static long integer(Object v, long def) {
        return v instanceof Long n ? n : v instanceof Double d && d == Math.rint(d) ? d.longValue() : def;
    }

    public static double number(Object v, double def) {
        return v instanceof Number n ? n.doubleValue() : def;
    }

    public static boolean bool(Object v, boolean def) {
        return v instanceof Boolean b ? b : def;
    }

    // ── Number arrays (a record's list of numbers) as values and back ────

    public static List<Object> listOf(int[] a) {
        List<Object> out = new ArrayList<>(a.length);
        for (int x : a) out.add((long) x);
        return out;
    }

    public static List<Object> listOf(long[] a) {
        List<Object> out = new ArrayList<>(a.length);
        for (long x : a) out.add(x);
        return out;
    }

    public static List<Object> listOf(float[] a) {
        List<Object> out = new ArrayList<>(a.length);
        for (float x : a) out.add((double) x);
        return out;
    }

    public static List<Object> listOf(double[] a) {
        List<Object> out = new ArrayList<>(a.length);
        for (double x : a) out.add(x);
        return out;
    }

    /** {@code v} (a list of numbers) as an array; what isn't a number is 0. */
    public static int[] ints(Object v) {
        List<Object> l = list(v);
        int[] out = new int[l.size()];
        for (int k = 0; k < out.length; k++) out[k] = (int) integer(l.get(k), 0);
        return out;
    }

    public static long[] longs(Object v) {
        List<Object> l = list(v);
        long[] out = new long[l.size()];
        for (int k = 0; k < out.length; k++) out[k] = integer(l.get(k), 0);
        return out;
    }

    public static float[] floats(Object v) {
        List<Object> l = list(v);
        float[] out = new float[l.size()];
        for (int k = 0; k < out.length; k++) out[k] = (float) number(l.get(k), 0);
        return out;
    }

    public static double[] doubles(Object v) {
        List<Object> l = list(v);
        double[] out = new double[l.size()];
        for (int k = 0; k < out.length; k++) out[k] = number(l.get(k), 0);
        return out;
    }
}
