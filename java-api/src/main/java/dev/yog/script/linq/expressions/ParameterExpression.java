package dev.yog.script.linq.expressions;

import dev.yog.script.Type;

/** A lambda's parameter: Yog Script's {@code ParameterExpression}. */
public class ParameterExpression extends Expression {
    private final String name;

    ParameterExpression(Type type, String name) {
        super(ExpressionType.Parameter, type);
        this.name = name;
    }

    public String Name() {
        return name;
    }

    @Override
    Object evaluate(Scope scope) {
        return scope.of(this);
    }

    @Override
    public String toString() {
        return name;
    }
}
