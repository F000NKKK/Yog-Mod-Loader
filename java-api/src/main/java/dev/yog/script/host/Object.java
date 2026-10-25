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

    public static java.lang.String ToString(java.lang.Object value) {
        return java.lang.String.valueOf(value);
    }
}
