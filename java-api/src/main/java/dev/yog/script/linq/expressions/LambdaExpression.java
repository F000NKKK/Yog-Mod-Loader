package dev.yog.script.linq.expressions;

import dev.yog.script.Type;
import dev.yog.script.collections.generic.IReadOnlyList;

/** A lambda: its parameters, what it gives. Yog Script's {@code LambdaExpression}. */
public abstract class LambdaExpression extends Expression {
    private final Expression body;
    private final ParameterExpression[] parameters;

    LambdaExpression(Type delegate, Expression body, ParameterExpression[] parameters) {
        super(ExpressionType.Lambda, delegate);
        this.body = body;
        this.parameters = parameters.clone();
    }

    public Expression Body() {
        return body;
    }

    public IReadOnlyList<ParameterExpression> Parameters() {
        return Nodes.listed(parameters);
    }

    public Type ReturnType() {
        return body.Type();
    }

    ParameterExpression[] parameters() {
        return parameters;
    }

    /** {@code x => body}, {@code (x, y) => body}. */
    @Override
    public String toString() {
        String params = parameters.length == 1 ? parameters[0].toString() : Nodes.joined(parameters);
        return params + " => " + body;
    }
}
