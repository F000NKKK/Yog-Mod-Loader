package dev.yog.script.linq.expressions;

import dev.yog.script.Type;

/** A lambda of a delegate: Yog Script's {@code Expression<TDelegate>}, a {@code TDelegate} when compiled. */
public final class Expression1<D> extends LambdaExpression {
    /** The compiler's code of a lambda it quoted; none for one run by its nodes. */
    private final D compiled;
    /** The delegate's class, a lambda run by its nodes is made one of. */
    private final Class<D> delegate;

    Expression1(Type delegateType, Expression body, ParameterExpression[] parameters, D compiled, Class<D> delegate) {
        super(delegateType, body, parameters);
        this.compiled = compiled;
        this.delegate = delegate;
    }

    /** One run by its nodes (made by hand, or inside a tree). */
    Expression1(Type delegateType, Expression body, ParameterExpression[] parameters, Class<D> delegate) {
        this(delegateType, body, parameters, null, delegate);
    }

    /** The delegate: the compiler's code for a lambda it quoted, one running the nodes for one made by hand. */
    public D Compile() {
        return compiled != null ? compiled : Nodes.compiled(delegate, Body(), parameters(), null);
    }

    /** Inside a tree: the delegate, seeing the parameters of the lambdas around it. */
    @Override
    Object evaluate(Scope scope) {
        return compiled != null ? compiled : Nodes.compiled(delegate, Body(), parameters(), scope);
    }
}
