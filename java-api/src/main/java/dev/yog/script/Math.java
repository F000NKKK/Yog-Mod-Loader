package dev.yog.script;

/**
 * Yog Script's {@code System.Math}, as .NET's: {@code Round} to the even
 * whole number, {@code Clamp} a panic when {@code min > max}, {@code Sign}
 * of NaN one. Java's own is written in full here: this class's name is the same.
 */
public final class Math {
    public static final double PI = java.lang.Math.PI;
    public static final double E = java.lang.Math.E;
    public static final double Tau = 2 * java.lang.Math.PI;

    private Math() {}

    public static int Max(int a, int b) {
        return java.lang.Math.max(a, b);
    }

    public static int Min(int a, int b) {
        return java.lang.Math.min(a, b);
    }

    public static int Abs(int value) {
        return java.lang.Math.abs(value);
    }

    public static int Clamp(int value, int min, int max) {
        if (min > max) {
            throw new IllegalArgumentException("`min` " + min + " is above `max` " + max);
        }
        return java.lang.Math.min(java.lang.Math.max(value, min), max);
    }

    public static long Max(long a, long b) {
        return java.lang.Math.max(a, b);
    }

    public static long Min(long a, long b) {
        return java.lang.Math.min(a, b);
    }

    public static long Abs(long value) {
        return java.lang.Math.abs(value);
    }

    public static long Clamp(long value, long min, long max) {
        if (min > max) {
            throw new IllegalArgumentException("`min` " + min + " is above `max` " + max);
        }
        return java.lang.Math.min(java.lang.Math.max(value, min), max);
    }

    public static float Max(float a, float b) {
        return java.lang.Math.max(a, b);
    }

    public static float Min(float a, float b) {
        return java.lang.Math.min(a, b);
    }

    public static float Abs(float value) {
        return java.lang.Math.abs(value);
    }

    public static float Clamp(float value, float min, float max) {
        if (min > max) {
            throw new IllegalArgumentException("`min` " + min + " is above `max` " + max);
        }
        return java.lang.Math.min(java.lang.Math.max(value, min), max);
    }

    public static double Max(double a, double b) {
        return java.lang.Math.max(a, b);
    }

    public static double Min(double a, double b) {
        return java.lang.Math.min(a, b);
    }

    public static double Abs(double value) {
        return java.lang.Math.abs(value);
    }

    public static double Clamp(double value, double min, double max) {
        if (min > max) {
            throw new IllegalArgumentException("`min` " + min + " is above `max` " + max);
        }
        return java.lang.Math.min(java.lang.Math.max(value, min), max);
    }

    public static int Sign(int value) {
        return Integer.signum(value);
    }

    public static int Sign(long value) {
        return Long.signum(value);
    }

    public static int Sign(float value) {
        if (Float.isNaN(value)) {
            throw new ArithmeticException("the sign of NaN");
        }
        return (int) java.lang.Math.signum(value);
    }

    public static int Sign(double value) {
        if (Double.isNaN(value)) {
            throw new ArithmeticException("the sign of NaN");
        }
        return (int) java.lang.Math.signum(value);
    }

    public static double Sqrt(double x) {
        return java.lang.Math.sqrt(x);
    }

    public static double Cbrt(double x) {
        return java.lang.Math.cbrt(x);
    }

    public static double Floor(double x) {
        return java.lang.Math.floor(x);
    }

    public static double Ceiling(double x) {
        return java.lang.Math.ceil(x);
    }

    public static double Exp(double x) {
        return java.lang.Math.exp(x);
    }

    public static double Log(double x) {
        return java.lang.Math.log(x);
    }

    public static double Log10(double x) {
        return java.lang.Math.log10(x);
    }

    public static double Sin(double x) {
        return java.lang.Math.sin(x);
    }

    public static double Cos(double x) {
        return java.lang.Math.cos(x);
    }

    public static double Tan(double x) {
        return java.lang.Math.tan(x);
    }

    public static double Asin(double x) {
        return java.lang.Math.asin(x);
    }

    public static double Acos(double x) {
        return java.lang.Math.acos(x);
    }

    public static double Atan(double x) {
        return java.lang.Math.atan(x);
    }

    public static double Sinh(double x) {
        return java.lang.Math.sinh(x);
    }

    public static double Cosh(double x) {
        return java.lang.Math.cosh(x);
    }

    public static double Tanh(double x) {
        return java.lang.Math.tanh(x);
    }

    public static double Round(double x) {
        return java.lang.Math.rint(x);
    }

    public static double Truncate(double x) {
        return x < 0 ? java.lang.Math.ceil(x) : java.lang.Math.floor(x);
    }

    public static double Log2(double x) {
        return java.lang.Math.log(x) / java.lang.Math.log(2);
    }

    public static double Pow(double x, double y) {
        return java.lang.Math.pow(x, y);
    }

    public static double Atan2(double y, double x) {
        return java.lang.Math.atan2(y, x);
    }
}
