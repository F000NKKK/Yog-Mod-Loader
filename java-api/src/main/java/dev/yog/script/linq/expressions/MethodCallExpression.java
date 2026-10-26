package dev.yog.script.linq.expressions;

import dev.yog.script.collections.generic.IReadOnlyList;
import dev.yog.script.reflection.MethodInfo;

/** A method called: Yog Script's {@code MethodCallExpression}. */
public class MethodCallExpression extends Expression {
    private final Expression object;
    private final MethodInfo method;
    private final Expression[] arguments;

    MethodCallExpression(Expression object, MethodInfo method, Expression[] arguments) {
        super(ExpressionType.Call, method.ReturnType());
        this.object = object;
        this.method = method;
        this.arguments = arguments.clone();
    }

    public Expression Object() {
        return object;
    }

    public MethodInfo Method() {
        return method;
    }

    public IReadOnlyList<Expression> Arguments() {
        return Nodes.listed(arguments);
    }

    @Override
    Object evaluate(Scope scope) {
        Object target = object == null ? null : object.evaluate(scope);
        return method.Invoke(target, Nodes.evaluated(arguments, scope));
    }

    @Override
    public String toString() {
        String on = object == null ? method.DeclaringType().Name() : object.toString();
        return on + "." + method.Name() + Nodes.joined(arguments);
    }
}
