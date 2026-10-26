package dev.yog.script.linq.expressions;

import dev.yog.script.Type;
import java.math.BigDecimal;
import java.util.Objects;

/** Numbers' arithmetic as a tree runs it: both operands at the wider of their types, as C#'s. */
final class Numbers {
    private Numbers() {}

    /** How wide a number is: what it's worked out as. */
    private enum Width { INT, LONG, FLOAT, DOUBLE, DECIMAL }

    private static Width width(Object n) {
        if (n instanceof BigDecimal) {
            return Width.DECIMAL;
        }
        if (n instanceof Double) {
            return Width.DOUBLE;
        }
        if (n instanceof Float) {
            return Width.FLOAT;
        }
        return n instanceof Long ? Width.LONG : Width.INT;
    }

    private static Width wider(Object a, Object b) {
        Width x = width(a);
        Width y = width(b);
        return x.ordinal() >= y.ordinal() ? x : y;
    }

    static Object arithmetic(ExpressionType op, Object l, Object r) {
        Number a = (Number) l;
        Number b = (Number) r;
        return switch (wider(l, r)) {
            case INT -> ints(op, a.intValue(), b.intValue());
            case LONG -> longs(op, a.longValue(), b.longValue());
            case FLOAT -> (float) doubles(op, a.doubleValue(), b.doubleValue());
            case DOUBLE -> doubles(op, a.doubleValue(), b.doubleValue());
            case DECIMAL -> decimals(op, decimal(a), decimal(b));
        };
    }

    private static int ints(ExpressionType op, int a, int b) {
        return switch (op) {
            case Add -> a + b;
            case Subtract -> a - b;
            case Multiply -> a * b;
            case Divide -> a / b;
            case Modulo -> a % b;
            case And -> a & b;
            case Or -> a | b;
            case ExclusiveOr -> a ^ b;
            case LeftShift -> a << b;
            case RightShift -> a >> b;
            default -> throw unknown(op);
        };
    }

    private static long longs(ExpressionType op, long a, long b) {
        return switch (op) {
            case Add -> a + b;
            case Subtract -> a - b;
            case Multiply -> a * b;
            case Divide -> a / b;
            case Modulo -> a % b;
            case And -> a & b;
            case Or -> a | b;
            case ExclusiveOr -> a ^ b;
            case LeftShift -> a << b;
            case RightShift -> a >> b;
            default -> throw unknown(op);
        };
    }

    private static double doubles(ExpressionType op, double a, double b) {
        return switch (op) {
            case Add -> a + b;
            case Subtract -> a - b;
            case Multiply -> a * b;
            case Divide -> a / b;
            case Modulo -> a % b;
            default -> throw unknown(op);
        };
    }

    private static BigDecimal decimals(ExpressionType op, BigDecimal a, BigDecimal b) {
        return switch (op) {
            case Add -> a.add(b);
            case Subtract -> a.subtract(b);
            case Multiply -> a.multiply(b);
            case Divide -> a.divide(b, java.math.MathContext.DECIMAL128);
            case Modulo -> a.remainder(b);
            default -> throw unknown(op);
        };
    }

    private static BigDecimal decimal(Number n) {
        return n instanceof BigDecimal d ? d : new BigDecimal(n.toString());
    }

    private static IllegalStateException unknown(ExpressionType op) {
        return new IllegalStateException("`" + op + "` isn't an operator of numbers");
    }

    static boolean equal(Object l, Object r) {
        if (l instanceof Number a && r instanceof Number b) {
            return compare(a, b) == 0;
        }
        return Objects.equals(l, r);
    }

    /** Numbers by value, strings and chars by their order; anything else has no order of its own. */
    static int compare(Object l, Object r) {
        if (l instanceof Number a && r instanceof Number b) {
            return decimal(a).compareTo(decimal(b));
        }
        if (l instanceof String a && r instanceof String b) {
            return a.compareTo(b);
        }
        if (l instanceof Character a && r instanceof Character b) {
            return a.compareTo(b);
        }
        throw new IllegalStateException("`" + l + "` and `" + r + "` aren't compared by order");
    }

    static Object negate(Object v) {
        return arithmetic(ExpressionType.Subtract, 0, v);
    }

    static Object complement(Object v) {
        return v instanceof Long l ? ~l : ~((Number) v).intValue();
    }

    /** {@code v} as {@code type}: a number made of its kind, anything else as it is. */
    static Object convert(Object v, Type type) {
        if (!(v instanceof Number n)) {
            return v;
        }
        if (type == Type.Of(int.class)) {
            return n.intValue();
        }
        if (type == Type.Of(long.class)) {
            return n.longValue();
        }
        if (type == Type.Of(float.class)) {
            return n.floatValue();
        }
        if (type == Type.Of(double.class)) {
            return n.doubleValue();
        }
        if (type == Type.Of(BigDecimal.class)) {
            return decimal(n);
        }
        return v;
    }
}
