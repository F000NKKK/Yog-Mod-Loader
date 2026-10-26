package dev.yog.script.host;

/**
 * A whole number's members: Yog Script's {@code System.Int32}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class Int32 {
    public static final int MinValue = java.lang.Integer.MIN_VALUE;
    public static final int MaxValue = java.lang.Integer.MAX_VALUE;

    private Int32() {}

    public static int Parse(java.lang.String s) {
        try {
            return java.lang.Integer.parseInt(s.strip());
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a i32", e);
        }
    }

    public static java.lang.Integer TryParse(java.lang.String s) {
        try {
            return Parse(s);
        } catch (java.lang.IllegalArgumentException e) {
            return null;
        }
    }

    public static int CompareTo(int value, int other) {
        return java.lang.Integer.compare(value, other);
    }

    public static boolean Equals(int value, int obj) {
        return value == obj;
    }

    /** Written by .NET format string {@code format} ({@code F2}, {@code X8}, {@code 0.00}). */
    public static java.lang.String ToString(int value, java.lang.String format) {
        return NumberFormats.integer(value, Integer.SIZE, true, format);
    }

    public static java.lang.String ToString(int value) {
        return java.lang.String.valueOf(value);
    }
}
