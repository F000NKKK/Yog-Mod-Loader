package dev.yog.script.host;

/**
 * .NET's composite format ({@code string.Format("{0,5:F2} of {1}", a, b)}):
 * each hole an argument by its index, its alignment padding it (right for
 * a positive one, left for a negative one), its format written by the
 * argument's own ({@code F2} of a number, {@code yyyy-MM-dd} of a date);
 * {@code {{ }}} are braces.
 */
public final class CompositeFormat {
    private CompositeFormat() {}

    public static java.lang.String format(java.lang.String format, java.lang.Object[] args) {
        StringBuilder out = new StringBuilder();
        int i = 0;
        while (i < format.length()) {
            char c = format.charAt(i);
            boolean doubled = i + 1 < format.length() && format.charAt(i + 1) == c;
            if ((c == '{' || c == '}') && doubled) {
                out.append(c);
                i += 2;
            } else if (c == '{') {
                int end = format.indexOf('}', i);
                if (end < 0) {
                    throw new dev.yog.script.YsPanic("a `{` in the format isn't closed", "");
                }
                out.append(hole(format.substring(i + 1, end), args));
                i = end + 1;
            } else if (c == '}') {
                throw new dev.yog.script.YsPanic("a `}` in the format opens nothing", "");
            } else {
                out.append(c);
                i += 1;
            }
        }
        return out.toString();
    }

    /** Hole {@code inside} ({@code 0}, {@code 0,5}, {@code 0:F2}) of {@code args}, written. */
    private static java.lang.String hole(java.lang.String inside, java.lang.Object[] args) {
        int colon = inside.indexOf(':');
        java.lang.String head = colon < 0 ? inside : inside.substring(0, colon);
        java.lang.String format = colon < 0 ? null : inside.substring(colon + 1);
        int comma = head.indexOf(',');
        int index = Integer.parseInt((comma < 0 ? head : head.substring(0, comma)).trim());
        int alignment = comma < 0 ? 0 : Integer.parseInt(head.substring(comma + 1).trim());
        if (index < 0 || index >= args.length) {
            throw new dev.yog.script.YsPanic("the format's hole " + index + " has no argument", "");
        }
        java.lang.String text = written(args[index], format);
        if (alignment > 0) {
            return String.PadLeft(text, alignment);
        }
        return alignment < 0 ? String.PadRight(text, -alignment) : text;
    }

    /** {@code value} written by {@code format} (none: as Yog Script writes it). */
    public static java.lang.String written(java.lang.Object value, java.lang.String format) {
        if (format == null || value == null) {
            return Object.ToString(value);
        }
        if (value instanceof java.lang.Integer n) {
            return Int32.ToString(n, format);
        }
        if (value instanceof java.lang.Long n) {
            return Int64.ToString(n, format);
        }
        if (value instanceof java.lang.Short n) {
            return Int16.ToString(n, format);
        }
        if (value instanceof java.lang.Byte n) {
            return SByte.ToString(n, format);
        }
        if (value instanceof java.lang.Double n) {
            return Double.ToString(n, format);
        }
        if (value instanceof java.lang.Float n) {
            return Single.ToString(n, format);
        }
        if (value instanceof java.math.BigDecimal n) {
            return Decimal.ToString(n, format);
        }
        if (value instanceof dev.yog.script.DateTime d) {
            return d.ToString(format);
        }
        return Object.ToString(value);
    }
}
