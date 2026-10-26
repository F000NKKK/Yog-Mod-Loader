package dev.yog.script.linq.expressions;

import dev.yog.script.Type;

/** One operand and its operator: Yog Script's {@code UnaryExpression}. */
public class UnaryExpression extends Expression {
    private final Expression operand;

    UnaryExpression(ExpressionType nodeType, Expression operand, Type type) {
        super(nodeType, type);
        this.operand = operand;
    }

    public Expression Operand() {
        return operand;
    }

    @Override
    Object evaluate(Scope scope) {
        Object v = operand.evaluate(scope);
        return switch (NodeType()) {
            case Not -> v instanceof Boolean b ? !b : Numbers.complement(v);
            case Negate -> Numbers.negate(v);
            default -> Numbers.convert(v, Type());
        };
    }

    /** {@code Not(x)}, {@code -x}, {@code Convert(x, Int32)}, as .NET writes them. */
    @Override
    public String toString() {
        return switch (NodeType()) {
            case Not -> "Not(" + operand + ")";
            case Negate -> "-" + operand;
            default -> "Convert(" + operand + ", " + Type().Name() + ")";
        };
    }
}
