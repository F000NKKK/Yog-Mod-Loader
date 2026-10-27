package dev.yog.script.yog;

import java.util.Arrays;
import java.util.Objects;

/**
 * Yog Script's {@code Yog.Text}: words a player reads — a
 * translation key and its arguments, translated into each player's language
 * as the game shows it (a translatable text component), or plain words.
 */
public final class Text {
    private static final Object[] NONE = new Object[0];

    private final String key;
    private final Object[] args;
    private final boolean plain;

    private Text(String key, Object[] args, boolean plain) {
        this.key = key;
        this.args = args;
        this.plain = plain;
    }

    /** The text of translation key {@code key}: what {@code t"key"} is. */
    public static Text Of(String key) {
        return new Text(key, NONE, false);
    }

    /** Plain words, shown as they are. */
    public static Text Plain(String words) {
        return new Text(words, NONE, true);
    }

    /** A {@code string} where a {@code Text} is wanted. */
    public static Text op_Implicit(String words) {
        return Plain(words);
    }

    /** This text with {@code args} in its placeholders. */
    public Text With(Object[] args) {
        return new Text(key, args.clone(), plain);
    }

    public String Key() {
        return key;
    }

    public boolean IsPlain() {
        return plain;
    }

    /** What a text's Yog Data form names its parts by. */
    private static final String KEY = "key";
    private static final String PLAIN = "plain";
    private static final String ARGS = "args";

    /**
     * This text as Yog Data (a {@code saved} member's): its key, whether
     * it's plain, its arguments — a number, a {@code bool}, a string as it
     * is, a text as its own, anything else as its words.
     */
    public byte[] toBytes() {
        return dev.yog.data.YogData.encode(data());
    }

    private java.util.Map<String, Object> data() {
        java.util.Map<String, Object> m = new java.util.LinkedHashMap<>();
        m.put(KEY, key);
        m.put(PLAIN, plain);
        java.util.List<Object> kept = new java.util.ArrayList<>();
        for (Object a : args) {
            if (a instanceof Text t) kept.add(t.data());
            else if (a instanceof Number || a instanceof Boolean || a instanceof String) kept.add(a);
            else kept.add(String.valueOf(a));
        }
        m.put(ARGS, kept);
        return m;
    }

    /** A text read back from its Yog Data ({@link #toBytes}). */
    public static Text fromBytes(byte[] bytes) {
        return of(dev.yog.data.YogData.map(dev.yog.data.YogData.decode(bytes)));
    }

    private static Text of(java.util.Map<String, Object> m) {
        java.util.List<Object> read = dev.yog.data.YogData.list(m.get(ARGS));
        Object[] back = new Object[read.size()];
        for (int i = 0; i < back.length; i++) {
            Object a = read.get(i);
            back[i] = a instanceof java.util.Map<?, ?> inner ? of(dev.yog.data.YogData.map(inner)) : a;
        }
        return new Text(dev.yog.data.YogData.str(m.get(KEY), ""), back, dev.yog.data.YogData.bool(m.get(PLAIN), false));
    }

    /** Its arguments (a copy). */
    public Object[] args() {
        return args.clone();
    }

    /**
     * As the game's text component JSON: {@code {"translate":key,"with":[…]}},
     * an argument that's a text as its own component; plain words as
     * {@code {"text":…}}.
     */
    public String json() {
        StringBuilder out = new StringBuilder();
        if (plain) {
            out.append("{\"text\":");
            quote(out, key);
            return out.append('}').toString();
        }
        out.append("{\"translate\":");
        quote(out, key);
        if (args.length > 0) {
            out.append(",\"with\":[");
            for (int i = 0; i < args.length; i++) {
                if (i > 0) out.append(',');
                if (args[i] instanceof Text t) out.append(t.json());
                else quote(out, String.valueOf(args[i]));
            }
            out.append(']');
        }
        return out.append('}').toString();
    }

    /** {@code s} as a JSON string. */
    private static void quote(StringBuilder out, String s) {
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

    /** The key (a plain text's words), its arguments after it: what logs show. */
    @Override
    public String toString() {
        return args.length == 0 ? key : key + Arrays.toString(args);
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof Text t && t.plain == plain && t.key.equals(key) && Arrays.equals(t.args, args);
    }

    @Override
    public int hashCode() {
        return Objects.hash(key, plain, Arrays.hashCode(args));
    }
}
