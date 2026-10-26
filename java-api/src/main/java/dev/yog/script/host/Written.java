package dev.yog.script.host;

/**
 * A floating-point number written as .NET writes it: its shortest digits,
 * {@code 2} for {@code 2.0}, {@code 1E+20} and {@code 1E-05} past fifteen
 * digits up or five down, {@code ∞} and {@code NaN}.
 */
final class Written {
    private static final double UP = 1e15;
    private static final double DOWN = 1e-5;
    private static final int EXPONENT_DIGITS = 2;

    private Written() {}

    /** {@code value}, whose shortest digits Java writes as {@code digits}. */
    static java.lang.String number(java.lang.String digits, double value) {
        if (java.lang.Double.isNaN(value)) {
            return "NaN";
        }
        if (java.lang.Double.isInfinite(value)) {
            return value > 0 ? "∞" : "-∞";
        }
        java.math.BigDecimal d = new java.math.BigDecimal(digits).stripTrailingZeros();
        double abs = java.lang.Math.abs(value);
        if (abs == 0 || (abs < UP && abs >= DOWN)) {
            return d.toPlainString();
        }
        java.lang.String all = d.unscaledValue().abs().toString();
        int exponent = all.length() - 1 - d.scale();
        java.lang.String mantissa = all.length() == 1 ? all : all.charAt(0) + "." + all.substring(1);
        java.lang.String power = java.lang.String.valueOf(java.lang.Math.abs(exponent));
        power = "0".repeat(java.lang.Math.max(0, EXPONENT_DIGITS - power.length())) + power;
        return (d.signum() < 0 ? "-" : "") + mantissa + "E" + (exponent < 0 ? "-" : "+") + power;
    }
}
