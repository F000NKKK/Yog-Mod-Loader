package dev.yog.script.host;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.RoundingMode;
import java.text.DecimalFormat;
import java.text.DecimalFormatSymbols;
import java.util.Locale;

/**
 * Numbers written by a .NET format string, invariant culture: the standard
 * ones ({@code C D E F G N P R X B}, each with its precision: {@code F2},
 * {@code D5}, {@code X8}) and custom ones ({@code 0.00}, {@code #,##0},
 * {@code 0.#%}), as {@code ToString(format)} and {@code {x:format}} take them.
 */
public final class NumberFormats {
    private NumberFormats() {}

    /** The digits {@code C}, {@code F}, {@code N} and {@code P} show when no precision's given. */
    private static final int DECIMALS = 2;
    /** {@code E}'s default digits after the point, and its exponent's least digits. */
    private static final int EXPONENT_DECIMALS = 6;
    private static final int EXPONENT_DIGITS = 3;
    /** Invariant culture's currency symbol, and percent's factor. */
    private static final java.lang.String CURRENCY = "¤";
    private static final BigDecimal HUNDRED = BigDecimal.valueOf(100);
    private static final int HEX = 16;
    private static final int BINARY = 2;
    private static final DecimalFormatSymbols INVARIANT = DecimalFormatSymbols.getInstance(Locale.ROOT);

    /** An integer {@code value} of {@code bits} (signed or not) written by {@code format}. */
    public static java.lang.String integer(long value, int bits, boolean signed, java.lang.String format) {
        Spec s = Spec.of(format);
        if (s == null) {
            return custom(BigDecimal.valueOf(value), format);
        }
        return switch (s.letter) {
            case 'D' -> digits(value, s.precision);
            case 'X' -> padded(Long.toHexString(masked(value, bits, signed)), s.precision, s.upper);
            case 'B' -> padded(Long.toBinaryString(masked(value, bits, signed)), s.precision, false);
            default -> decimal(BigDecimal.valueOf(value), s, true);
        };
    }

    /** A floating {@code value} written by {@code format} ({@code single}: an {@code f32}). */
    public static java.lang.String floating(double value, boolean single, java.lang.String format) {
        if (java.lang.Double.isNaN(value)) {
            return "NaN";
        }
        if (java.lang.Double.isInfinite(value)) {
            return value > 0 ? "∞" : "-∞";
        }
        Spec s = Spec.of(format);
        BigDecimal exact = single ? new BigDecimal(java.lang.Float.toString((float) value)) : new BigDecimal(value);
        if (s == null) {
            return custom(exact, format);
        }
        if (s.letter == 'R' || (s.letter == 'G' && s.precision < 0)) {
            return single ? Single.ToString((float) value) : Double.ToString(value);
        }
        return decimal(exact, s, false);
    }

    /** A {@code decimal} written by {@code format}. */
    public static java.lang.String decimal(BigDecimal value, java.lang.String format) {
        Spec s = Spec.of(format);
        if (s == null) {
            return custom(value, format);
        }
        if (s.letter == 'G' && s.precision < 0) {
            return Decimal.ToString(value);
        }
        return decimal(value, s, false);
    }

    /** A value's standard format, but for an integer's own letters. */
    private static java.lang.String decimal(BigDecimal v, Spec s, boolean integral) {
        return switch (s.letter) {
            case 'F' -> fixed(v, s.or(DECIMALS), false);
            case 'N' -> fixed(v, s.or(DECIMALS), true);
            case 'C' -> signed(v, CURRENCY + fixed(v.abs(), s.or(DECIMALS), true));
            case 'P' -> fixed(v.multiply(HUNDRED), s.or(DECIMALS), true) + " %";
            case 'E' -> exponential(v, s.or(EXPONENT_DECIMALS), s.upper);
            case 'G' -> general(v, s.precision, s.upper);
            case 'R', 'D' -> integral ? v.toPlainString() : general(v, -1, s.upper);
            default -> throw new dev.yog.script.YsPanic("`" + s.letter + "` isn't a format of numbers", "");
        };
    }

    /** {@code text} with {@code v}'s sign before it. */
    private static java.lang.String signed(BigDecimal v, java.lang.String text) {
        return v.signum() < 0 ? "-" + text : text;
    }

    private static java.lang.String fixed(BigDecimal v, int decimals, boolean grouped) {
        BigDecimal rounded = v.setScale(decimals, RoundingMode.HALF_UP);
        if (!grouped) {
            return rounded.toPlainString();
        }
        DecimalFormat f = new DecimalFormat(decimals == 0 ? "#,##0" : "#,##0." + "0".repeat(decimals), INVARIANT);
        f.setRoundingMode(RoundingMode.HALF_UP);
        return f.format(rounded);
    }

    private static java.lang.String exponential(BigDecimal v, int decimals, boolean upper) {
        if (v.signum() == 0) {
            return "0." + "0".repeat(decimals) + (upper ? "E+" : "e+") + "0".repeat(EXPONENT_DIGITS);
        }
        int exponent = v.precision() - v.scale() - 1;
        BigDecimal mantissa = v.movePointLeft(exponent).setScale(decimals, RoundingMode.HALF_UP);
        if (mantissa.abs().compareTo(BigDecimal.TEN) >= 0) {
            mantissa = mantissa.movePointLeft(1).setScale(decimals, RoundingMode.HALF_UP);
            exponent += 1;
        }
        java.lang.String digits = java.lang.String.valueOf(Math.abs(exponent));
        java.lang.String padded = "0".repeat(Math.max(0, EXPONENT_DIGITS - digits.length())) + digits;
        return mantissa.toPlainString() + (upper ? "E" : "e") + (exponent < 0 ? "-" : "+") + padded;
    }

    /** {@code G}: as few digits as written (or {@code digits} significant ones), exponential when far. */
    private static java.lang.String general(BigDecimal v, int digits, boolean upper) {
        BigDecimal rounded = digits > 0 ? v.round(new java.math.MathContext(digits, RoundingMode.HALF_UP)) : v;
        rounded = rounded.stripTrailingZeros();
        int exponent = rounded.precision() - rounded.scale() - 1;
        int limit = digits > 0 ? digits : rounded.precision();
        if (rounded.signum() != 0 && (exponent >= Math.max(limit, 1) || exponent < -5)) {
            java.lang.String e = exponential(rounded, Math.max(0, rounded.precision() - 1), upper);
            int at = e.indexOf(upper ? 'E' : 'e');
            java.lang.String sign = e.substring(at + 1, at + 2);
            java.lang.String exp = e.substring(at + 2).replaceFirst("^0(?=\\d\\d)", "");
            return e.substring(0, at) + (upper ? "E" : "e") + sign + exp;
        }
        return rounded.toPlainString();
    }

    private static java.lang.String digits(long value, int least) {
        java.lang.String d = java.lang.String.valueOf(Math.abs(value));
        java.lang.String padded = least > d.length() ? "0".repeat(least - d.length()) + d : d;
        return value < 0 ? "-" + padded : padded;
    }

    /** A value's bits as its type's ({@code -1} of an {@code i32}: 8 hex digits). */
    private static long masked(long value, int bits, boolean signed) {
        return signed && bits < Long.SIZE ? value & ((1L << bits) - 1) : value;
    }

    private static java.lang.String padded(java.lang.String digits, int least, boolean upper) {
        java.lang.String d = upper ? digits.toUpperCase(Locale.ROOT) : digits;
        return least > d.length() ? "0".repeat(least - d.length()) + d : d;
    }

    /** A custom format ({@code 0.00}, {@code #,##0.#}, {@code 0%}): Java's own pattern of the same signs. */
    private static java.lang.String custom(BigDecimal v, java.lang.String format) {
        DecimalFormat f = new DecimalFormat(format, INVARIANT);
        f.setRoundingMode(RoundingMode.HALF_UP);
        return f.format(v);
    }

    /** A standard format: its letter (upper-cased), its precision ({@code -1}: none), its case. */
    private record Spec(char letter, int precision, boolean upper) {
        static Spec of(java.lang.String format) {
            if (format == null || format.isEmpty()) {
                return new Spec('G', -1, true);
            }
            char c = format.charAt(0);
            java.lang.String rest = format.substring(1);
            if (!Character.isLetter(c) || !rest.chars().allMatch(Character::isDigit) || rest.length() > 3) {
                return null;
            }
            int precision = rest.isEmpty() ? -1 : Integer.parseInt(rest);
            return new Spec(Character.toUpperCase(c), precision, Character.isUpperCase(c));
        }

        int or(int fallback) {
            return precision < 0 ? fallback : precision;
        }
    }

    /** An unsigned 64-bit value ({@code u64}) as a {@code decimal}. */
    static BigDecimal unsigned(long value) {
        return new BigDecimal(new BigInteger(Long.toUnsignedString(value)));
    }
}
