package dev.yog.script.linq.expressions;

import dev.yog.script.collections.generic.IReadOnlyList;
import dev.yog.script.reflection.ConstructorInfo;

/** A new object made: Yog Script's {@code NewExpression}. */
public class NewExpression extends Expression {
    private final ConstructorInfo constructor;
    private final Expression[] arguments;

    NewExpression(ConstructorInfo constructor, Expression[] arguments) {
        super(ExpressionType.New, constructor.DeclaringType());
        this.constructor = constructor;
        this.arguments = arguments.clone();
    }

    public ConstructorInfo Constructor() {
        return constructor;
    }

    public IReadOnlyList<Expression> Arguments() {
        return Nodes.listed(arguments);
    }

    @Override
    Object evaluate(Scope scope) {
        return constructor.Invoke(Nodes.evaluated(arguments, scope));
    }

    @Override
    public String toString() {
        return "new " + Type().Name() + Nodes.joined(arguments);
    }
}
