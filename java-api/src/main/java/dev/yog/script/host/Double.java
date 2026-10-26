package dev.yog.script.host;

/**
 * A floating-point number's members: Yog Script's {@code System.Double}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class Double {
    public static final double MinValue = -java.lang.Double.MAX_VALUE;
    public static final double MaxValue = java.lang.Double.MAX_VALUE;
    public static final double Epsilon = java.lang.Double.MIN_VALUE;
    public static final double NaN = java.lang.Double.NaN;
    public static final double PositiveInfinity = java.lang.Double.POSITIVE_INFINITY;
    public static final double NegativeInfinity = java.lang.Double.NEGATIVE_INFINITY;
    public static final double Pi = java.lang.Math.PI;
    public static final double E = java.lang.Math.E;
    public static final double Tau = 2 * java.lang.Math.PI;

    private Double() {}

    public static boolean IsNaN(double f) {
        return java.lang.Double.isNaN(f);
    }

    public static boolean IsInfinity(double f) {
        return java.lang.Double.isInfinite(f);
    }

    public static boolean IsPositiveInfinity(double f) {
        return f == PositiveInfinity;
    }

    public static boolean IsNegativeInfinity(double f) {
        return f == NegativeInfinity;
    }

    public static boolean IsFinite(double f) {
        return java.lang.Double.isFinite(f);
    }

    public static double Parse(java.lang.String s) {
        try {
            return java.lang.Double.parseDouble(s.strip());
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a f64", e);
        }
    }

    public static java.lang.Double TryParse(java.lang.String s) {
        try {
            return Parse(s);
        } catch (java.lang.IllegalArgumentException e) {
            return null;
        }
    }

    public static int CompareTo(double value, double other) {
        return java.lang.Double.compare(value, other);
    }

    public static boolean Equals(double value, double obj) {
        return java.lang.Double.compare(value, obj) == 0;
    }

    public static java.lang.String ToString(double value) {
        return java.lang.String.valueOf(value);
    }
}
