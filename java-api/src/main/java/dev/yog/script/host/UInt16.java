package dev.yog.script.host;

/**
 * A whole number's members: Yog Script's {@code System.UInt16}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class UInt16 {
    public static final int MinValue = 0;
    public static final int MaxValue = 65535;

    private UInt16() {}

    public static int Parse(java.lang.String s) {
        try {
            return within(java.lang.Integer.parseInt(s.strip()), MaxValue, s);
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a u16", e);
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
        return NumberFormats.integer(value, Short.SIZE, false, format);
    }

    public static java.lang.String ToString(int value) {
        return java.lang.String.valueOf(value);
    }

    /** {@code n}, when it's in the unsigned range up to {@code max}; a panic for {@code s} else. */
    private static int within(int n, int max, java.lang.String s) {
        if (n < 0 || n > max) {
            throw new java.lang.NumberFormatException("`" + s + "` is out of a u16's range");
        }
        return n;
    }
}
