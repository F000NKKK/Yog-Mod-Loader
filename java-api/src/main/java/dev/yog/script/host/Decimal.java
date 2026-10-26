package dev.yog.script.host;

/**
 * A decimal number's members: Yog Script's {@code System.Decimal}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class Decimal {
    /** .NET's range: 96 bits of digits. */
    public static final java.math.BigDecimal MaxValue = new java.math.BigDecimal("79228162514264337593543950335");
    public static final java.math.BigDecimal MinValue = MaxValue.negate();
    public static final java.math.BigDecimal Zero = java.math.BigDecimal.ZERO;
    public static final java.math.BigDecimal One = java.math.BigDecimal.ONE;

    private Decimal() {}

    /** A whole number as a decimal. */
    public static java.math.BigDecimal op_Implicit(long value) {
        return java.math.BigDecimal.valueOf(value);
    }

    /** A floating-point number as a decimal, as it's written. */
    public static java.math.BigDecimal op_Implicit(double value) {
        return java.math.BigDecimal.valueOf(value);
    }

    public static java.math.BigDecimal Round(java.math.BigDecimal d) {
        return d.setScale(0, java.math.RoundingMode.HALF_EVEN);
    }

    public static java.math.BigDecimal Floor(java.math.BigDecimal d) {
        return d.setScale(0, java.math.RoundingMode.FLOOR);
    }

    public static java.math.BigDecimal Ceiling(java.math.BigDecimal d) {
        return d.setScale(0, java.math.RoundingMode.CEILING);
    }

    public static java.math.BigDecimal Truncate(java.math.BigDecimal d) {
        return d.setScale(0, java.math.RoundingMode.DOWN);
    }

    public static java.math.BigDecimal Parse(java.lang.String s) {
        try {
            return new java.math.BigDecimal(s.strip());
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a decimal", e);
        }
    }

    public static java.math.BigDecimal TryParse(java.lang.String s) {
        try {
            return Parse(s);
        } catch (java.lang.IllegalArgumentException e) {
            return null;
        }
    }

    public static int CompareTo(java.math.BigDecimal value, java.math.BigDecimal other) {
        return value.compareTo(other);
    }

    public static boolean Equals(java.math.BigDecimal value, java.math.BigDecimal obj) {
        return value.compareTo(obj) == 0;
    }

    public static java.lang.String ToString(java.math.BigDecimal value) {
        return value.toPlainString();
    }
}
