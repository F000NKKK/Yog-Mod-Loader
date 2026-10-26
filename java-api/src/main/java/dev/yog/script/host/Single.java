package dev.yog.script.host;

/**
 * A floating-point number's members: Yog Script's {@code System.Single}, a static each, the value
 * first. Java's own types are written in full here: this package's
 * classes are named as the CLR's.
 */
public final class Single {
    public static final float MinValue = -java.lang.Float.MAX_VALUE;
    public static final float MaxValue = java.lang.Float.MAX_VALUE;
    public static final float Epsilon = java.lang.Float.MIN_VALUE;
    public static final float NaN = java.lang.Float.NaN;
    public static final float PositiveInfinity = java.lang.Float.POSITIVE_INFINITY;
    public static final float NegativeInfinity = java.lang.Float.NEGATIVE_INFINITY;
    public static final float Pi = (float) java.lang.Math.PI;
    public static final float E = (float) java.lang.Math.E;
    public static final float Tau = (float) (2 * java.lang.Math.PI);

    private Single() {}

    public static boolean IsNaN(float f) {
        return java.lang.Float.isNaN(f);
    }

    public static boolean IsInfinity(float f) {
        return java.lang.Float.isInfinite(f);
    }

    public static boolean IsPositiveInfinity(float f) {
        return f == PositiveInfinity;
    }

    public static boolean IsNegativeInfinity(float f) {
        return f == NegativeInfinity;
    }

    public static boolean IsFinite(float f) {
        return java.lang.Float.isFinite(f);
    }

    public static float Parse(java.lang.String s) {
        try {
            return java.lang.Float.parseFloat(s.strip());
        } catch (java.lang.NumberFormatException e) {
            throw new java.lang.IllegalArgumentException("`" + s + "` isn't a f32", e);
        }
    }

    public static java.lang.Float TryParse(java.lang.String s) {
        try {
            return Parse(s);
        } catch (java.lang.IllegalArgumentException e) {
            return null;
        }
    }

    public static int CompareTo(float value, float other) {
        return java.lang.Float.compare(value, other);
    }

    public static boolean Equals(float value, float obj) {
        return java.lang.Float.compare(value, obj) == 0;
    }

    public static java.lang.String ToString(float value) {
        return java.lang.String.valueOf(value);
    }
}
