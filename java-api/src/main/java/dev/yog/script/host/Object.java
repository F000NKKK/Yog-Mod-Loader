package dev.yog.script.host;

/**
 * {@code object}'s members as Yog Script has them ({@code System.Object}),
 * for a value whose type doesn't answer them itself: a static each, the
 * value first. Java's {@code Object} is written in full here: this
 * class's name is the same.
 */
public final class Object {
    private Object() {}

    public static boolean Equals(java.lang.Object value, java.lang.Object obj) {
        return java.util.Objects.equals(value, obj);
    }

    public static int GetHashCode(java.lang.Object value) {
        return java.util.Objects.hashCode(value);
    }

    public static dev.yog.script.Type GetType(java.lang.Object value) {
        return dev.yog.script.Type.Of(value);
    }

    /**
     * As Yog Script writes it: a boxed number, {@code bool} or {@code decimal}
     * as its type does ({@code 2}, {@code True}), {@code null} as nothing
     * (text made of it, as .NET's), else its own.
     */
    public static java.lang.String ToString(java.lang.Object value) {
        // An `if` chain, not a pattern `switch`: the runtime runs on Java 17 too.
        if (value == null) {
            return "";
        }
        if (value instanceof java.lang.Boolean b) {
            return Boolean.ToString(b);
        }
        if (value instanceof java.lang.Double d) {
            return Double.ToString(d);
        }
        if (value instanceof java.lang.Float f) {
            return Single.ToString(f);
        }
        if (value instanceof java.math.BigDecimal d) {
            return Decimal.ToString(d);
        }
        return value.toString();
    }
}
