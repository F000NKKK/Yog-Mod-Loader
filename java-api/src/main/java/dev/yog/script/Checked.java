package dev.yog.script;

/**
 * Yog Script's {@code checked} arithmetic (C#'s): a result outside its
 * type's range panics where it was computed, instead of wrapping. A type
 * narrower than {@code i64} is computed in {@code long} and its range
 * checked; {@code i64}'s operations are exact.
 */
public final class Checked {
    private Checked() {}

    /** The message of an overflow (.NET's {@code OverflowException}'s). */
    private static final String OVERFLOW = "arithmetic operation resulted in an overflow";

    /** {@code v}, when it's in {@code min..max}; else a panic at {@code at}. */
    public static long range(long v, long min, long max, String at) {
        if (v < min || v > max) {
            throw new YsPanic(OVERFLOW, at);
        }
        return v;
    }

    /** {@code v} made a whole number in {@code min..max}; NaN or out of it, a panic. */
    public static long fromFloat(double v, long min, long max, String at) {
        if (Double.isNaN(v) || v < min || v > max) {
            throw new YsPanic(OVERFLOW, at);
        }
        return (long) v;
    }

    public static long add(long a, long b, String at) {
        try {
            return java.lang.Math.addExact(a, b);
        } catch (ArithmeticException e) {
            throw new YsPanic(OVERFLOW, at);
        }
    }

    public static long sub(long a, long b, String at) {
        try {
            return java.lang.Math.subtractExact(a, b);
        } catch (ArithmeticException e) {
            throw new YsPanic(OVERFLOW, at);
        }
    }

    public static long mul(long a, long b, String at) {
        try {
            return java.lang.Math.multiplyExact(a, b);
        } catch (ArithmeticException e) {
            throw new YsPanic(OVERFLOW, at);
        }
    }

    public static long neg(long a, String at) {
        try {
            return java.lang.Math.negateExact(a);
        } catch (ArithmeticException e) {
            throw new YsPanic(OVERFLOW, at);
        }
    }
}
