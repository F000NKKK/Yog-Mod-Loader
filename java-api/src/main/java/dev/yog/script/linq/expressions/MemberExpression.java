package dev.yog.script.linq.expressions;

import dev.yog.script.Type;
import dev.yog.script.reflection.FieldInfo;
import dev.yog.script.reflection.MemberInfo;
import dev.yog.script.reflection.PropertyInfo;

/** A field or a property read: Yog Script's {@code MemberExpression}. */
public class MemberExpression extends Expression {
    private final Expression expression;
    private final MemberInfo member;

    MemberExpression(Expression expression, MemberInfo member, Type type) {
        super(ExpressionType.MemberAccess, type);
        this.expression = expression;
        this.member = member;
    }

    public Expression Expression() {
        return expression;
    }

    public MemberInfo Member() {
        return member;
    }

    @Override
    Object evaluate(Scope scope) {
        Object target = expression == null ? null : expression.evaluate(scope);
        return member instanceof FieldInfo f ? f.GetValue(target) : ((PropertyInfo) member).GetValue(target);
    }

    @Override
    public String toString() {
        String on = expression == null ? member.DeclaringType().Name() : expression.toString();
        return on + "." + member.Name();
    }
}
