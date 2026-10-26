package dev.yog.script.linq.expressions;

import dev.yog.script.Type;
import java.util.Map;
import java.util.Set;
import java.util.function.BiFunction;

/** Two operands and their operator: Yog Script's {@code BinaryExpression}. */
public class BinaryExpression extends Expression {
    /** What gives a {@code bool}, whatever it compares. */
    private static final Set<ExpressionType> TESTS = Set.of(
            ExpressionType.Equal, ExpressionType.NotEqual,
            ExpressionType.LessThan, ExpressionType.LessThanOrEqual,
            ExpressionType.GreaterThan, ExpressionType.GreaterThanOrEqual,
            ExpressionType.AndAlso, ExpressionType.OrElse);

    /** Comparisons, by what they ask of their operands. */
    private static final Map<ExpressionType, BiFunction<Object, Object, Object>> COMPARED = Map.of(
            ExpressionType.Equal, Numbers::equal,
            ExpressionType.NotEqual, (l, r) -> !Numbers.equal(l, r),
            ExpressionType.LessThan, (l, r) -> Numbers.compare(l, r) < 0,
            ExpressionType.LessThanOrEqual, (l, r) -> Numbers.compare(l, r) <= 0,
            ExpressionType.GreaterThan, (l, r) -> Numbers.compare(l, r) > 0,
            ExpressionType.GreaterThanOrEqual, (l, r) -> Numbers.compare(l, r) >= 0);

    /** Each operator as .NET writes it in a tree's text; `And`, `Or` of `bool`s by their names. */
    private static final Map<ExpressionType, String> SYMBOLS = Map.ofEntries(
            Map.entry(ExpressionType.Add, "+"),
            Map.entry(ExpressionType.Subtract, "-"),
            Map.entry(ExpressionType.Multiply, "*"),
            Map.entry(ExpressionType.Divide, "/"),
            Map.entry(ExpressionType.Modulo, "%"),
            Map.entry(ExpressionType.And, "&"),
            Map.entry(ExpressionType.Or, "|"),
            Map.entry(ExpressionType.ExclusiveOr, "^"),
            Map.entry(ExpressionType.LeftShift, "<<"),
            Map.entry(ExpressionType.RightShift, ">>"),
            Map.entry(ExpressionType.Equal, "=="),
            Map.entry(ExpressionType.NotEqual, "!="),
            Map.entry(ExpressionType.LessThan, "<"),
            Map.entry(ExpressionType.LessThanOrEqual, "<="),
            Map.entry(ExpressionType.GreaterThan, ">"),
            Map.entry(ExpressionType.GreaterThanOrEqual, ">="),
            Map.entry(ExpressionType.Coalesce, "??"));

    private final Expression left;
    private final Expression right;

    private BinaryExpression(ExpressionType nodeType, Expression left, Expression right, Type type) {
        super(nodeType, type);
        this.left = left;
        this.right = right;
    }

    /** {@code left op right}: a test gives a {@code bool}, {@code ??} the right's type, an array's item its item's, the rest the left's. */
    static BinaryExpression of(ExpressionType op, Expression left, Expression right) {
        Type type;
        if (TESTS.contains(op)) {
            type = Type.Of(boolean.class);
        } else if (op == ExpressionType.Coalesce) {
            type = right.Type();
        } else if (op == ExpressionType.ArrayIndex) {
            type = left.Type().GetElementType();
        } else {
            type = left.Type();
        }
        return new BinaryExpression(op, left, right, type);
    }

    public Expression Left() {
        return left;
    }

    public Expression Right() {
        return right;
    }

    @Override
    Object evaluate(Scope scope) {
        Object l = left.evaluate(scope);
        ExpressionType op = NodeType();
        if (op == ExpressionType.AndAlso) {
            return (Boolean) l && (Boolean) right.evaluate(scope);
        }
        if (op == ExpressionType.OrElse) {
            return (Boolean) l || (Boolean) right.evaluate(scope);
        }
        if (op == ExpressionType.Coalesce) {
            return l != null ? l : right.evaluate(scope);
        }
        return applied(op, l, right.evaluate(scope));
    }

    private static Object applied(ExpressionType op, Object l, Object r) {
        BiFunction<Object, Object, Object> compared = COMPARED.get(op);
        if (compared != null) {
            return compared.apply(l, r);
        }
        if (op == ExpressionType.ArrayIndex) {
            return java.lang.reflect.Array.get(l, ((Number) r).intValue());
        }
        if (op == ExpressionType.Add && (l instanceof String || r instanceof String)) {
            return dev.yog.script.host.Object.ToString(l) + dev.yog.script.host.Object.ToString(r);
        }
        if (l instanceof Boolean a) {
            return logical(op, a, (Boolean) r);
        }
        return Numbers.arithmetic(op, l, r);
    }

    /** {@code &}, {@code |}, {@code ^} of two {@code bool}s: both evaluated. */
    private static boolean logical(ExpressionType op, boolean a, boolean b) {
        if (op == ExpressionType.And) {
            return a & b;
        }
        return op == ExpressionType.Or ? a | b : a ^ b;
    }

    /** {@code (x > 10)}, {@code (a AndAlso b)}, {@code xs[i]}, as .NET writes them. */
    @Override
    public String toString() {
        if (NodeType() == ExpressionType.ArrayIndex) {
            return left + "[" + right + "]";
        }
        return "(" + left + " " + symbol() + " " + right + ")";
    }

    private String symbol() {
        boolean logical = left.Type() == Type.Of(boolean.class);
        boolean named = logical && (NodeType() == ExpressionType.And || NodeType() == ExpressionType.Or);
        return named ? NodeType().toString() : SYMBOLS.getOrDefault(NodeType(), NodeType().toString());
    }
}
