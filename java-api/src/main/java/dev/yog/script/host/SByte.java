package dev.yog.script.host;

/**
 * A whole number's members: Yog Script's {@code System.SByte}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class SByte {
    public static final byte MinValue = java.lang.Byte.MIN_VALUE;
    public static final byte MaxValue = java.lang.Byte.MAX_VALUE;

    private SByte() {}

    public static byte Parse(java.lang.String s) {
        try {
            return java.lang.Byte.parseByte(s.strip());
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a i8", e);
        }
    }

    public static java.lang.Byte TryParse(java.lang.String s) {
        try {
            return Parse(s);
        } catch (java.lang.IllegalArgumentException e) {
            return null;
        }
    }

    public static int CompareTo(byte value, byte other) {
        return java.lang.Byte.compare(value, other);
    }

    public static boolean Equals(byte value, byte obj) {
        return value == obj;
    }

    /** Written by .NET format string {@code format} ({@code F2}, {@code X8}, {@code 0.00}). */
    public static java.lang.String ToString(byte value, java.lang.String format) {
        return NumberFormats.integer(value, java.lang.Byte.SIZE, true, format);
    }

    public static java.lang.String ToString(byte value) {
        return java.lang.String.valueOf(value);
    }
}
