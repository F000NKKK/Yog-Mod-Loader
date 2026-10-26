package dev.yog.script.linq.expressions;

import dev.yog.script.Type;

/** A value: Yog Script's {@code ConstantExpression}. */
public class ConstantExpression extends Expression {
    private static final String QUOTE = "\"";
    private static final String NULL = "null";

    private final Object value;

    ConstantExpression(Object value, Type type) {
        super(ExpressionType.Constant, type);
        this.value = value;
    }

    public Object Value() {
        return value;
    }

    @Override
    Object evaluate(Scope scope) {
        return value;
    }

    @Override
    public String toString() {
        if (value == null) {
            return NULL;
        }
        return value instanceof String s ? QUOTE + s + QUOTE : dev.yog.script.host.Object.ToString(value);
    }
}
