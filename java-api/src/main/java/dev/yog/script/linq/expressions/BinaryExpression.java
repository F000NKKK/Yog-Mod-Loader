package dev.yog.script.linq.expressions;

import dev.yog.script.Type;
import java.util.Objects;

/** Two operands and their operator: Yog Script's {@code BinaryExpression}. */
public class BinaryExpression extends Expression {
    private final Expression left;
    private final Expression right;

    private BinaryExpression(ExpressionType nodeType, Expression left, Expression right, Type type) {
        super(nodeType, type);
        this.left = left;
        this.right = right;
    }

    /** {@code left op right}: a comparison gives a {@code bool}, {@code ??} the right's type, an array's item its item's, the rest the left's. */
    static BinaryExpression of(ExpressionType op, Expression left, Expression right) {
        Type type = switch (op) {
            case Equal, NotEqual, LessThan, LessThanOrEqual, GreaterThan, GreaterThanOrEqual, AndAlso, OrElse -> Type.Of(boolean.class);
            case Coalesce -> right.Type();
            case ArrayIndex -> left.Type().GetElementType();
            default -> left.Type();
        };
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
        return switch (NodeType()) {
            case AndAlso -> (Boolean) l && (Boolean) right.evaluate(scope);
            case OrElse -> (Boolean) l || (Boolean) right.evaluate(scope);
            case Coalesce -> l != null ? l : right.evaluate(scope);
            default -> applied(l, right.evaluate(scope));
        };
    }

    private Object applied(Object l, Object r) {
        return switch (NodeType()) {
            case Equal -> Numbers.equal(l, r);
            case NotEqual -> !Numbers.equal(l, r);
            case LessThan -> Numbers.compare(l, r) < 0;
            case LessThanOrEqual -> Numbers.compare(l, r) <= 0;
            case GreaterThan -> Numbers.compare(l, r) > 0;
            case GreaterThanOrEqual -> Numbers.compare(l, r) >= 0;
            case ArrayIndex -> java.lang.reflect.Array.get(l, ((Number) r).intValue());
            case Add -> l instanceof String || r instanceof String
                    ? dev.yog.script.host.Object.ToString(l) + dev.yog.script.host.Object.ToString(r)
                    : Numbers.arithmetic(NodeType(), l, r);
            case And, Or, ExclusiveOr -> l instanceof Boolean a ? logical(a, (Boolean) r) : Numbers.arithmetic(NodeType(), l, r);
            default -> Numbers.arithmetic(NodeType(), l, r);
        };
    }

    private Object logical(boolean a, boolean b) {
        return switch (NodeType()) {
            case And -> a & b;
            case Or -> a | b;
            default -> a ^ b;
        };
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
        boolean logical = Objects.equals(left.Type(), Type.Of(boolean.class));
        return switch (NodeType()) {
            case Add -> "+";
            case Subtract -> "-";
            case Multiply -> "*";
            case Divide -> "/";
            case Modulo -> "%";
            case And -> logical ? "And" : "&";
            case Or -> logical ? "Or" : "|";
            case ExclusiveOr -> "^";
            case LeftShift -> "<<";
            case RightShift -> ">>";
            case Equal -> "==";
            case NotEqual -> "!=";
            case LessThan -> "<";
            case LessThanOrEqual -> "<=";
            case GreaterThan -> ">";
            case GreaterThanOrEqual -> ">=";
            case Coalesce -> "??";
            default -> NodeType().name();
        };
    }
}
