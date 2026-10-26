package dev.yog.script.host;

/**
 * A whole number's members: Yog Script's {@code System.Int64}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class Int64 {
    public static final long MinValue = java.lang.Long.MIN_VALUE;
    public static final long MaxValue = java.lang.Long.MAX_VALUE;

    private Int64() {}

    public static long Parse(java.lang.String s) {
        try {
            return java.lang.Long.parseLong(s.strip());
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a i64", e);
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

    /** Written by .NET format string {@code format} ({@code F2}, {@code X8}, {@code 0.00}). */
    public static java.lang.String ToString(long value, java.lang.String format) {
        return NumberFormats.integer(value, Long.SIZE, true, format);
    }

    public static java.lang.String ToString(long value) {
        return java.lang.String.valueOf(value);
    }
}
