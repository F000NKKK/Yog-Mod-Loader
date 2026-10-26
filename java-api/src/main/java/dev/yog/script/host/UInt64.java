package dev.yog.script.host;

/**
 * A whole number's members: Yog Script's {@code System.UInt64}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class UInt64 {
    public static final long MinValue = 0L;
    /** All bits set: the JVM has no unsigned {@code long}; read unsigned. */
    public static final long MaxValue = -1L;

    private UInt64() {}

    public static long Parse(java.lang.String s) {
        try {
            return java.lang.Long.parseUnsignedLong(s.strip());
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a u64", e);
        }
    }

    public static java.lang.Long TryParse(java.lang.String s) {
        try {
            return Parse(s);
        } catch (java.lang.IllegalArgumentException e) {
            return null;
        }
    }

    public static int CompareTo(long value, long other) {
        return java.lang.Long.compareUnsigned(value, other);
    }

    public static boolean Equals(long value, long obj) {
        return value == obj;
    }

    public static java.lang.String ToString(long value) {
        return java.lang.Long.toUnsignedString(value);
    }
}
