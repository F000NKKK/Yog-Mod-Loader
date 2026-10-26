package dev.yog.script.linq.expressions;

import dev.yog.script.Type;

/** One operand and its operator: Yog Script's {@code UnaryExpression}. */
public class UnaryExpression extends Expression {
    private final Expression operand;
    private final dev.yog.script.reflection.MethodInfo method;

    UnaryExpression(ExpressionType nodeType, Expression operand, Type type, dev.yog.script.reflection.MethodInfo method) {
        super(nodeType, type);
        this.operand = operand;
        this.method = method;
    }

    public Expression Operand() {
        return operand;
    }

    /** The operator a type declares it is; {@code null} for the language's. */
    public dev.yog.script.reflection.MethodInfo Method() {
        return method;
    }

    @Override
    Object evaluate(Scope scope) {
        Object v = operand.evaluate(scope);
        if (method != null) {
            return method.Invoke(null, new Object[] {v});
        }
        if (NodeType() == ExpressionType.Not) {
            return v instanceof Boolean b ? !b : Numbers.complement(v);
        }
        return NodeType() == ExpressionType.Negate ? Numbers.negate(v) : Numbers.convert(v, Type());
    }

    /** {@code Not(x)}, {@code -x}, {@code Convert(x, Int32)}, as .NET writes them. */
    @Override
    public String toString() {
        if (NodeType() == ExpressionType.Not) {
            return "Not(" + operand + ")";
        }
        return NodeType() == ExpressionType.Negate ? "-" + operand : "Convert(" + operand + ", " + Type().Name() + ")";
    }
}
