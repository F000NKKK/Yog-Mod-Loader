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
