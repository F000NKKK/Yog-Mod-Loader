package dev.yog.script.linq.expressions;

/** {@code Test ? IfTrue : IfFalse}: Yog Script's {@code ConditionalExpression}. */
public class ConditionalExpression extends Expression {
    private final Expression test;
    private final Expression ifTrue;
    private final Expression ifFalse;

    ConditionalExpression(Expression test, Expression ifTrue, Expression ifFalse) {
        super(ExpressionType.Conditional, ifTrue.Type());
        this.test = test;
        this.ifTrue = ifTrue;
        this.ifFalse = ifFalse;
    }

    public Expression Test() {
        return test;
    }

    public Expression IfTrue() {
        return ifTrue;
    }

    public Expression IfFalse() {
        return ifFalse;
    }

    @Override
    Object evaluate(Scope scope) {
        return (Boolean) test.evaluate(scope) ? ifTrue.evaluate(scope) : ifFalse.evaluate(scope);
    }

    /** {@code IIF(test, a, b)}, as .NET writes it. */
    @Override
    public String toString() {
        return "IIF(" + test + ", " + ifTrue + ", " + ifFalse + ")";
    }
}
