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

    /** {@code u64}'s {@code a + b}: past its range (it wraps below {@code a}), a panic. */
    public static long uadd(long a, long b, String at) {
        long sum = a + b;
        if (Long.compareUnsigned(sum, a) < 0) {
            throw new YsPanic(OVERFLOW, at);
        }
        return sum;
    }

    /** {@code u64}'s {@code a - b}: below zero, a panic. */
    public static long usub(long a, long b, String at) {
        if (Long.compareUnsigned(a, b) < 0) {
            throw new YsPanic(OVERFLOW, at);
        }
        return a - b;
    }

    /** {@code u64}'s {@code a * b}: its 128-bit product's high half not zero, a panic. */
    public static long umul(long a, long b, String at) {
        // Java 17 has no unsignedMultiplyHigh: the signed one, corrected.
        long high = java.lang.Math.multiplyHigh(a, b) + ((a >> (Long.SIZE - 1)) & b) + ((b >> (Long.SIZE - 1)) & a);
        if (high != 0) {
            throw new YsPanic(OVERFLOW, at);
        }
        return a * b;
    }

    /** {@code u64}'s {@code -a}: anything but zero is below zero. */
    public static long uneg(long a, String at) {
        if (a != 0) {
            throw new YsPanic(OVERFLOW, at);
        }
        return 0;
    }

    /** A {@code u64} {@code v} no more than {@code max} (unsigned); else a panic. */
    public static long uatMost(long v, long max, String at) {
        if (Long.compareUnsigned(v, max) > 0) {
            throw new YsPanic(OVERFLOW, at);
        }
        return v;
    }

    /** {@code v} made a {@code u64}: NaN, below zero or past 2^64, a panic. */
    public static long ufromFloat(double v, String at) {
        if (Double.isNaN(v) || v < 0 || v >= U64_END) {
            throw new YsPanic(OVERFLOW, at);
        }
        return v < Long.MAX_VALUE ? (long) v : java.math.BigDecimal.valueOf(v).toBigInteger().longValue();
    }

    /** 2^64: the first {@code double} past a {@code u64}'s range. */
    private static final double U64_END = 0x1p64;

    public static long neg(long a, String at) {
        try {
            return java.lang.Math.negateExact(a);
        } catch (ArithmeticException e) {
            throw new YsPanic(OVERFLOW, at);
        }
    }
}
