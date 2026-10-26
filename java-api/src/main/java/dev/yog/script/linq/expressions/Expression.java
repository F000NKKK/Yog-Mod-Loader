package dev.yog.script.linq.expressions;

import dev.yog.script.Type;
import dev.yog.script.reflection.ConstructorInfo;
import dev.yog.script.reflection.FieldInfo;
import dev.yog.script.reflection.MethodInfo;
import dev.yog.script.reflection.PropertyInfo;
import java.util.function.Function;

/**
 * Code as a tree to read: Yog Script's {@code System.Linq.Expressions.Expression},
 * its nodes made by these factories — or by the compiler, from a lambda
 * given where an {@code Expression<Func<…>>} is wanted ({@link #quote}).
 */
public abstract class Expression {
    private final ExpressionType nodeType;
    private final Type type;

    Expression(ExpressionType nodeType, Type type) {
        this.nodeType = nodeType;
        this.type = type;
    }

    public ExpressionType NodeType() {
        return nodeType;
    }

    public Type Type() {
        return type;
    }

    /** What it gives where {@code scope} says what its parameters are. */
    abstract Object evaluate(Scope scope);

    public String ToString() {
        return toString();
    }

    /** A lambda the compiler quoted: its parameters ({@code names} of {@code types}), its {@code body} made of them, {@code compiled} as code. */
    public static <D> Expression1<D> quote(Type delegate, String[] names, Type[] types, Function<ParameterExpression[], Expression> body, D compiled) {
        ParameterExpression[] parameters = new ParameterExpression[names.length];
        for (int i = 0; i < names.length; i++) {
            parameters[i] = new ParameterExpression(types[i], names[i]);
        }
        return new Expression1<>(delegate, body.apply(parameters), parameters, compiled);
    }

    /** {@code owner}'s method {@code name}, the {@code overload}-th of its name, as the compiler chose it. */
    public static MethodInfo methodOf(Type owner, String name, int overload) {
        int seen = 0;
        for (dev.yog.script.reflection.MemberInfo m : owner.members()) {
            if (m instanceof MethodInfo method && method.Name().equals(name) && seen++ == overload) {
                return method;
            }
        }
        throw new IllegalStateException("`" + owner.Name() + "` has no `" + name + "` #" + overload);
    }

    /** {@code type}'s static property {@code name}, else its static field of the name. */
    public static MemberExpression staticMember(Type type, String name) {
        PropertyInfo property = type.GetProperty(name);
        if (property != null) {
            return Property(null, property);
        }
        FieldInfo field = type.GetField(name);
        if (field == null) {
            throw new IllegalArgumentException("`" + type.Name() + "` has no property or field `" + name + "`");
        }
        return Field(null, field);
    }

    // Leaves.

    public static ConstantExpression Constant(Object value) {
        return new ConstantExpression(value, value == null ? Type.Of(Object.class) : Type.Of(value));
    }

    public static ConstantExpression Constant(Object value, Type type) {
        return new ConstantExpression(value, type);
    }

    public static ParameterExpression Parameter(Type type) {
        return new ParameterExpression(type, null);
    }

    public static ParameterExpression Parameter(Type type, String name) {
        return new ParameterExpression(type, name);
    }

    // Operators.

    public static BinaryExpression Add(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.Add, left, right);
    }

    public static BinaryExpression Subtract(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.Subtract, left, right);
    }

    public static BinaryExpression Multiply(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.Multiply, left, right);
    }

    public static BinaryExpression Divide(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.Divide, left, right);
    }

    public static BinaryExpression Modulo(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.Modulo, left, right);
    }

    public static BinaryExpression And(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.And, left, right);
    }

    public static BinaryExpression Or(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.Or, left, right);
    }

    public static BinaryExpression ExclusiveOr(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.ExclusiveOr, left, right);
    }

    public static BinaryExpression LeftShift(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.LeftShift, left, right);
    }

    public static BinaryExpression RightShift(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.RightShift, left, right);
    }

    public static BinaryExpression AndAlso(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.AndAlso, left, right);
    }

    public static BinaryExpression OrElse(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.OrElse, left, right);
    }

    public static BinaryExpression Equal(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.Equal, left, right);
    }

    public static BinaryExpression NotEqual(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.NotEqual, left, right);
    }

    public static BinaryExpression LessThan(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.LessThan, left, right);
    }

    public static BinaryExpression LessThanOrEqual(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.LessThanOrEqual, left, right);
    }

    public static BinaryExpression GreaterThan(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.GreaterThan, left, right);
    }

    public static BinaryExpression GreaterThanOrEqual(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.GreaterThanOrEqual, left, right);
    }

    public static BinaryExpression Coalesce(Expression left, Expression right) {
        return BinaryExpression.of(ExpressionType.Coalesce, left, right);
    }

    public static BinaryExpression ArrayIndex(Expression array, Expression index) {
        return BinaryExpression.of(ExpressionType.ArrayIndex, array, index);
    }

    public static UnaryExpression Not(Expression expression) {
        return new UnaryExpression(ExpressionType.Not, expression, expression.Type());
    }

    public static UnaryExpression Negate(Expression expression) {
        return new UnaryExpression(ExpressionType.Negate, expression, expression.Type());
    }

    public static UnaryExpression Convert(Expression expression, Type type) {
        return new UnaryExpression(ExpressionType.Convert, expression, type);
    }

    // Members, calls, the rest.

    public static MemberExpression Field(Expression expression, FieldInfo field) {
        return new MemberExpression(expression, field, field.FieldType());
    }

    public static MemberExpression Property(Expression expression, PropertyInfo property) {
        return new MemberExpression(expression, property, property.PropertyType());
    }

    public static MemberExpression PropertyOrField(Expression expression, String propertyOrFieldName) {
        PropertyInfo property = expression.Type().GetProperty(propertyOrFieldName);
        if (property != null) {
            return Property(expression, property);
        }
        FieldInfo field = expression.Type().GetField(propertyOrFieldName);
        if (field == null) {
            throw new IllegalArgumentException("`" + expression.Type().Name() + "` has no property or field `" + propertyOrFieldName + "`");
        }
        return Field(expression, field);
    }

    public static MethodCallExpression Call(Expression instance, MethodInfo method, Expression[] arguments) {
        return new MethodCallExpression(instance, method, arguments);
    }

    public static ConditionalExpression Condition(Expression condition, Expression ifTrue, Expression ifFalse) {
        return new ConditionalExpression(condition, ifTrue, ifFalse);
    }

    public static NewExpression New(ConstructorInfo constructor, Expression[] arguments) {
        return new NewExpression(constructor, arguments);
    }

    /** A lambda of {@code parameters} giving {@code body}, compiled as a {@code delegate} (its class handed first, {@code [Reified]}). */
    public static <D> Expression1<D> Lambda(Class<D> delegate, Expression body, ParameterExpression[] parameters) {
        return new Expression1<>(Type.Of(delegate), body, parameters, Nodes.compiled(delegate, body, parameters));
    }
}
