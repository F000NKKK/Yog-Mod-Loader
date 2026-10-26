package dev.yog.script.host;

/**
 * A whole number's members: Yog Script's {@code System.Int16}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class Int16 {
    public static final short MinValue = java.lang.Short.MIN_VALUE;
    public static final short MaxValue = java.lang.Short.MAX_VALUE;

    private Int16() {}

    public static short Parse(java.lang.String s) {
        try {
            return java.lang.Short.parseShort(s.strip());
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a i16", e);
        }
    }

    public static java.lang.Short TryParse(java.lang.String s) {
        try {
            return Parse(s);
        } catch (java.lang.IllegalArgumentException e) {
            return null;
        }
    }

    public static int CompareTo(short value, short other) {
        return java.lang.Short.compare(value, other);
    }

    public static boolean Equals(short value, short obj) {
        return value == obj;
    }

    public static java.lang.String ToString(short value) {
        return java.lang.String.valueOf(value);
    }
}
