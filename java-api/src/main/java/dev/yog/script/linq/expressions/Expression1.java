package dev.yog.script.linq.expressions;

import dev.yog.script.Type;

/** A lambda of a delegate: Yog Script's {@code Expression<TDelegate>}, a {@code TDelegate} when compiled. */
public final class Expression1<D> extends LambdaExpression {
    private final D compiled;

    Expression1(Type delegate, Expression body, ParameterExpression[] parameters, D compiled) {
        super(delegate, body, parameters);
        this.compiled = compiled;
    }

    /** The delegate: the compiler's code for a lambda it quoted, one running the nodes for one made by hand. */
    public D Compile() {
        return compiled;
    }

    @Override
    Object evaluate(Scope scope) {
        return compiled;
    }
}
