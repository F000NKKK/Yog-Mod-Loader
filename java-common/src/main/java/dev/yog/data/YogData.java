package dev.yog.data;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Yog Data's binary form (the Rust crate {@code yog-data}, module
 * {@code binary}): the one format content crosses between the native
 * runtime and the hosts in. Pure Java, no Minecraft types — every host
 * shares it.
 *
 * <p>Values decode to: {@code null}, {@link Boolean}, {@link Long},
 * {@link Double}, {@link String}, {@link List}, {@link Map} (insertion
 * ordered), and the typed records {@link Ref}, {@link Tag}, {@link Res},
 * {@link Text}, {@link Image}, {@link Color}, {@link Lang}. {@link #encode} takes the
 * same (plus {@link Integer}/{@link Float}) and writes exactly what the
 * Rust encoder writes for the same value.
 */
public final class YogData {
    private YogData() {}

    public static final byte[] MAGIC = {'Y', 'O', 'G', 'D'};
    public static final int VERSION = 1;
    /** Nested deeper than this is malformed. */
    public static final int MAX_DEPTH = 256;

    private static final int NULL = 0, FALSE = 1, TRUE = 2, INT = 3, FLOAT = 4, STR = 5, LIST = 6, MAP = 7,
            REF = 8, TAG = 9, RES = 10, TEXT = 11, IMAGE = 12, COLOR = 13, LANG = 14;

    /** Content of a kind ({@code item}, {@code block}, ...) by id. */
    public record Ref(String kind, String id) {}

    /** A tag of content of a kind. */
    public record Tag(String kind, String id) {}

    /** A mod resource, {@code @Set.key}. */
    public record Res(String set, String key) {}

    /** Yog text ({@code {b|bold}}). */
    public record Text(String text) {}

    /** An image source ({@code sprite:button}, {@code mymod:gui/logo}). */
    public record Image(String source) {}

    /** {@code 0xAARRGGBB}. */
    public record Color(int argb) {}

    /** A translation key of the game's or a mod's lang files ({@code @lang:gui.done}). */
    public record Lang(String key) {}

    /** Malformed binary data. */
    public static final class DecodeException extends RuntimeException {
        public final int offset;

        DecodeException(String message, int offset) {
            super("byte " + offset + ": " + message);
            this.offset = offset;
        }
    }

    // ── Decoding ─────────────────────────────────────────────────────────

    /** Reads the binary form; throws {@link DecodeException} on malformed bytes. */
    public static Object decode(byte[] bytes) {
        Reader r = new Reader(bytes);
        for (int i = 0; i < 4; i++) {
            if (bytes.length <= i || bytes[i] != MAGIC[i]) throw new DecodeException("not Yog Data (no `YOGD` header)", 0);
        }
        r.i = 4;
        int version = r.u8();
        if (version != VERSION) throw new DecodeException("Yog Data version " + version + ", this reads version " + VERSION, 4);
        int count = r.count(1);
        r.strings = new String[count];
        for (int s = 0; s < count; s++) {
            int len = r.count(1);
            r.strings[s] = new String(bytes, r.i, len, StandardCharsets.UTF_8);
            r.i += len;
        }
        Object v = r.value(0);
        if (r.i != bytes.length) throw new DecodeException("bytes after the value", r.i);
        return v;
    }

    private static final class Reader {
        final byte[] b;
        int i;
        String[] strings;

        Reader(byte[] b) {
            this.b = b;
        }

        int u8() {
            if (i >= b.length) throw new DecodeException("unexpected end", i);
            return b[i++] & 0xFF;
        }

        long varint() {
            long n = 0;
            for (int shift = 0; shift < 64; shift += 7) {
                int x = u8();
                n |= (long) (x & 0x7F) << shift;
                if ((x & 0x80) == 0) return n;
            }
            throw new DecodeException("varint longer than 64 bits", i);
        }

        int count(int min) {
            long n = varint();
            int left = b.length - i;
            if (n < 0 || n > left / Math.max(1, min)) throw new DecodeException("length " + n + " past the end (" + left + " bytes left)", i);
            return (int) n;
        }

        String string() {
            long s = varint();
            if (s < 0 || s >= strings.length) throw new DecodeException("string " + s + " isn't in the table", i);
            return strings[(int) s];
        }

        long fixed(int n) {
            if (i + n > b.length) throw new DecodeException("unexpected end", i);
            long v = 0;
            for (int k = 0; k < n; k++) v |= (long) (b[i + k] & 0xFF) << (8 * k);
            i += n;
            return v;
        }

        Object value(int depth) {
            if (depth > MAX_DEPTH) throw new DecodeException("nested deeper than " + MAX_DEPTH + " levels", i);
            int tag = u8();
            switch (tag) {
                case NULL: return null;
                case FALSE: return Boolean.FALSE;
                case TRUE: return Boolean.TRUE;
                case INT: {
                    long z = varint();
                    return (z >>> 1) ^ -(z & 1);
                }
                case FLOAT: return Double.longBitsToDouble(fixed(8));
                case STR: return string();
                case LIST: {
                    int n = count(1);
                    List<Object> list = new ArrayList<>(n);
                    for (int k = 0; k < n; k++) list.add(value(depth + 1));
                    return list;
                }
                case MAP: {
                    int n = count(2);
                    Map<String, Object> map = new LinkedHashMap<>(n * 2);
                    for (int k = 0; k < n; k++) {
                        String key = string();
                        map.put(key, value(depth + 1));
                    }
                    return map;
                }
                case REF: return new Ref(string(), string());
                case TAG: return new Tag(string(), string());
                case RES: return new Res(string(), string());
                case TEXT: return new Text(string());
                case IMAGE: return new Image(string());
                case COLOR: return new Color((int) fixed(4));
                case LANG: return new Lang(string());
                default: throw new DecodeException("unknown tag " + tag, i - 1);
            }
        }
    }

    // ── Encoding ─────────────────────────────────────────────────────────

    /** {@code value} in the binary form. */
    public static byte[] encode(Object value) {
        Writer w = new Writer();
        w.value(value);
        ByteArrayOutputStream out = new ByteArrayOutputStream(w.body.size() + 64);
        out.writeBytes(MAGIC);
        out.write(VERSION);
        varint(out, w.table.size());
        for (String s : w.table) {
            byte[] utf8 = s.getBytes(StandardCharsets.UTF_8);
            varint(out, utf8.length);
            out.writeBytes(utf8);
        }
        out.writeBytes(w.body.toByteArray());
        return out.toByteArray();
    }

    private static final class Writer {
        final ByteArrayOutputStream body = new ByteArrayOutputStream();
        final Map<String, Integer> index = new HashMap<>();
        final List<String> table = new ArrayList<>();

        void string(String s) {
            Integer i = index.get(s);
            if (i == null) {
                i = table.size();
                index.put(s, i);
                table.add(s);
            }
            varint(body, i);
        }

        void fixed(long v, int n) {
            for (int k = 0; k < n; k++) body.write((int) (v >>> (8 * k)) & 0xFF);
        }

        void value(Object v) {
            if (v == null) {
                body.write(NULL);
            } else if (v instanceof Boolean bool) {
                body.write(bool ? TRUE : FALSE);
            } else if (v instanceof Long || v instanceof Integer || v instanceof Short || v instanceof Byte) {
                long n = ((Number) v).longValue();
                body.write(INT);
                varint(body, (n << 1) ^ (n >> 63));
            } else if (v instanceof Double || v instanceof Float) {
                body.write(FLOAT);
                fixed(Double.doubleToRawLongBits(((Number) v).doubleValue()), 8);
            } else if (v instanceof String s) {
                body.write(STR);
                string(s);
            } else if (v instanceof List<?> list) {
                body.write(LIST);
                varint(body, list.size());
                for (Object item : list) value(item);
            } else if (v instanceof Map<?, ?> map) {
                body.write(MAP);
                varint(body, map.size());
                for (Map.Entry<?, ?> e : map.entrySet()) {
                    string((String) e.getKey());
                    value(e.getValue());
                }
            } else if (v instanceof Ref r) {
                body.write(REF);
                string(r.kind());
                string(r.id());
            } else if (v instanceof Tag t) {
                body.write(TAG);
                string(t.kind());
                string(t.id());
            } else if (v instanceof Res r) {
                body.write(RES);
                string(r.set());
                string(r.key());
            } else if (v instanceof Text t) {
                body.write(TEXT);
                string(t.text());
            } else if (v instanceof Image img) {
                body.write(IMAGE);
                string(img.source());
            } else if (v instanceof Lang l) {
                body.write(LANG);
                string(l.key());
            } else if (v instanceof Color c) {
                body.write(COLOR);
                fixed(c.argb() & 0xFFFFFFFFL, 4);
            } else {
                throw new IllegalArgumentException("not a Yog Data value: " + v.getClass().getName());
            }
        }
    }

    private static void varint(ByteArrayOutputStream out, long n) {
        while ((n & ~0x7FL) != 0) {
            out.write((int) (n & 0x7F) | 0x80);
            n >>>= 7;
        }
        out.write((int) n);
    }

    // ── JSON ─────────────────────────────────────────────────────────────

    /**
     * {@code v} as Gson's JSON tree — what {@link #toJson} writes, without
     * the text in between (typed values as their string forms).
     */
    public static com.google.gson.JsonElement toGson(Object v) {
        if (v == null) return com.google.gson.JsonNull.INSTANCE;
        if (v instanceof Boolean b) return new com.google.gson.JsonPrimitive(b);
        if (v instanceof Long || v instanceof Integer || v instanceof Double) return new com.google.gson.JsonPrimitive((Number) v);
        if (v instanceof String s) return new com.google.gson.JsonPrimitive(s);
        if (v instanceof List<?> l) {
            com.google.gson.JsonArray a = new com.google.gson.JsonArray(l.size());
            for (Object x : l) a.add(toGson(x));
            return a;
        }
        if (v instanceof Map<?, ?> m) {
            com.google.gson.JsonObject o = new com.google.gson.JsonObject();
            for (Map.Entry<?, ?> e : m.entrySet()) o.add(String.valueOf(e.getKey()), toGson(e.getValue()));
            return o;
        }
        if (v instanceof Text t) return new com.google.gson.JsonPrimitive(t.text());
        StringBuilder one = new StringBuilder();
        json(v, one);
        return com.google.gson.JsonParser.parseString(one.toString());
    }

    /** {@code v} as JSON text (typed values as their string forms: {@code "@item:minecraft:stick"}). */
    public static String toJson(Object v) {
        StringBuilder out = new StringBuilder();
        json(v, out);
        return out.toString();
    }

    private static void json(Object v, StringBuilder out) {
        if (v == null) {
            out.append("null");
        } else if (v instanceof Boolean || v instanceof Long || v instanceof Integer) {
            out.append(v);
        } else if (v instanceof Double d) {
            out.append(d.isNaN() || d.isInfinite() ? "null" : (d == Math.rint(d) && Math.abs(d) < 1e15 ? String.valueOf(d.longValue()) + ".0" : String.valueOf(d)));
        } else if (v instanceof String s) {
            quote(s, out);
        } else if (v instanceof List<?> l) {
            out.append('[');
            for (int i = 0; i < l.size(); i++) {
                if (i > 0) out.append(',');
                json(l.get(i), out);
            }
            out.append(']');
        } else if (v instanceof Map<?, ?> m) {
            out.append('{');
            boolean first = true;
            for (Map.Entry<?, ?> e : m.entrySet()) {
                if (!first) out.append(',');
                first = false;
                quote(String.valueOf(e.getKey()), out);
                out.append(':');
                json(e.getValue(), out);
            }
            out.append('}');
        } else if (v instanceof Ref r) {
            quote("@" + r.kind() + ":" + r.id(), out);
        } else if (v instanceof Tag t) {
            quote("@tag:" + t.kind() + ":" + t.id(), out);
        } else if (v instanceof Res r) {
            quote("@" + r.set() + "." + r.key(), out);
        } else if (v instanceof Text t) {
            quote(t.text(), out);
        } else if (v instanceof Image i) {
            quote("@image:" + i.source(), out);
        } else if (v instanceof Lang l) {
            quote("@lang:" + l.key(), out);
        } else if (v instanceof Color c) {
            quote(String.format("#%08X", c.argb()), out);
        } else {
            quote(String.valueOf(v), out);
        }
    }

    private static void quote(String s, StringBuilder out) {
        out.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> {
                    if (c < 0x20) out.append(String.format("\\u%04x", (int) c));
                    else out.append(c);
                }
            }
        }
        out.append('"');
    }

    // ── Reading values ───────────────────────────────────────────────────

    /** {@code v} as a map, or an empty one. */
    @SuppressWarnings("unchecked")
    public static Map<String, Object> map(Object v) {
        return v instanceof Map<?, ?> m ? (Map<String, Object>) m : Collections.emptyMap();
    }

    /** {@code v} as a list, or an empty one. */
    @SuppressWarnings("unchecked")
    public static List<Object> list(Object v) {
        return v instanceof List<?> l ? (List<Object>) l : Collections.emptyList();
    }

    /** {@code v} as a string ({@link Text} and {@link Image} give theirs), or {@code def}. */
    public static String str(Object v, String def) {
        if (v instanceof String s) return s;
        if (v instanceof Text t) return t.text();
        if (v instanceof Image i) return i.source();
        return def;
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
}
