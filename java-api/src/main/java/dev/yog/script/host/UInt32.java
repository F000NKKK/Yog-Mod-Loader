package dev.yog.script.host;

/**
 * A whole number's members: Yog Script's {@code System.UInt32}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class UInt32 {
    public static final long MinValue = 0L;
    public static final long MaxValue = 4294967295L;

    private UInt32() {}

    public static long Parse(java.lang.String s) {
        try {
            return within(java.lang.Long.parseLong(s.strip()), MaxValue, s);
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a u32", e);
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
        return java.lang.Long.compare(value, other);
    }

    public static boolean Equals(long value, long obj) {
        return value == obj;
    }

    public static java.lang.String ToString(long value) {
        return java.lang.String.valueOf(value);
    }

    /** {@code n}, when it's in the unsigned range up to {@code max}; a panic for {@code s} else. */
    private static long within(long n, long max, java.lang.String s) {
        if (n < 0 || n > max) {
            throw new java.lang.NumberFormatException("`" + s + "` is out of a u32's range");
        }
        return n;
    }
}
