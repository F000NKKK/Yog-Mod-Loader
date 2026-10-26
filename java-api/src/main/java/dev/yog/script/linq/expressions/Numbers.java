package dev.yog.script.linq.expressions;

import dev.yog.script.Type;
import java.math.BigDecimal;
import java.math.MathContext;
import java.util.Map;
import java.util.Objects;
import java.util.function.BinaryOperator;
import java.util.function.DoubleBinaryOperator;
import java.util.function.LongBinaryOperator;

/** Numbers' arithmetic as a tree runs it: both operands at the wider of their types, as C#'s. */
final class Numbers {
    private Numbers() {}

    /** How wide a number is: what it's worked out as. */
    private enum Width { INT, LONG, FLOAT, DOUBLE, DECIMAL }

    /** An {@code int}'s shift counts are its low bits only, as Java's and C#'s. */
    private static final long INT_SHIFT = Integer.SIZE - 1;

    private static final Map<ExpressionType, LongBinaryOperator> WHOLE = Map.ofEntries(
            Map.entry(ExpressionType.Add, (a, b) -> a + b),
            Map.entry(ExpressionType.Subtract, (a, b) -> a - b),
            Map.entry(ExpressionType.Multiply, (a, b) -> a * b),
            Map.entry(ExpressionType.Divide, (a, b) -> a / b),
            Map.entry(ExpressionType.Modulo, (a, b) -> a % b),
            Map.entry(ExpressionType.And, (a, b) -> a & b),
            Map.entry(ExpressionType.Or, (a, b) -> a | b),
            Map.entry(ExpressionType.ExclusiveOr, (a, b) -> a ^ b),
            Map.entry(ExpressionType.LeftShift, (a, b) -> a << b),
            Map.entry(ExpressionType.RightShift, (a, b) -> a >> b));

    private static final Map<ExpressionType, DoubleBinaryOperator> REAL = Map.of(
            ExpressionType.Add, (a, b) -> a + b,
            ExpressionType.Subtract, (a, b) -> a - b,
            ExpressionType.Multiply, (a, b) -> a * b,
            ExpressionType.Divide, (a, b) -> a / b,
            ExpressionType.Modulo, (a, b) -> a % b);

    private static final Map<ExpressionType, BinaryOperator<BigDecimal>> DECIMAL = Map.of(
            ExpressionType.Add, BigDecimal::add,
            ExpressionType.Subtract, BigDecimal::subtract,
            ExpressionType.Multiply, BigDecimal::multiply,
            ExpressionType.Divide, (a, b) -> a.divide(b, MathContext.DECIMAL128),
            ExpressionType.Modulo, BigDecimal::remainder);

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
            case INT -> (int) of(WHOLE, op).applyAsLong(a.intValue(), shifts(op) ? b.intValue() & INT_SHIFT : b.intValue());
            case LONG -> of(WHOLE, op).applyAsLong(a.longValue(), b.longValue());
            case FLOAT -> (float) of(REAL, op).applyAsDouble(a.doubleValue(), b.doubleValue());
            case DOUBLE -> of(REAL, op).applyAsDouble(a.doubleValue(), b.doubleValue());
            case DECIMAL -> of(DECIMAL, op).apply(decimal(a), decimal(b));
        };
    }

    private static boolean shifts(ExpressionType op) {
        return op == ExpressionType.LeftShift || op == ExpressionType.RightShift;
    }

    private static <F> F of(Map<ExpressionType, F> ops, ExpressionType op) {
        F f = ops.get(op);
        if (f == null) {
            throw new IllegalStateException("`" + op + "` isn't an operator of these numbers");
        }
        return f;
    }

    private static BigDecimal decimal(Number n) {
        return n instanceof BigDecimal d ? d : new BigDecimal(n.toString());
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
