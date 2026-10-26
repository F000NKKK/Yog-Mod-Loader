package dev.yog.script;

import dev.yog.script.reflection.ConstructorInfo;
import dev.yog.script.reflection.MethodInfo;
import java.lang.reflect.Proxy;
import java.math.BigDecimal;
import java.util.Map;

/**
 * What generic code does with a type parameter's value that Java's erased
 * generics can't (C#'s constraints): reach it through its bound
 * ({@code where T : IComparable<T>}, a number's too), make one
 * ({@code new T()}), give its type's default ({@code default(T)}).
 */
public final class Generics {
    private Generics() {}

    /** The built-in values' zeros, by their boxes. */
    private static final Map<Class<?>, Object> ZEROS = Map.of(
            Integer.class, 0,
            Long.class, 0L,
            Short.class, (short) 0,
            Byte.class, (byte) 0,
            Double.class, 0.0,
            Float.class, 0.0f,
            Boolean.class, false,
            Character.class, '\0',
            BigDecimal.class, BigDecimal.ZERO);

    /**
     * {@code value} as interface {@code view}: itself when it implements it,
     * else a view running its own type's members of the names (a number's
     * {@code IComparable<int>}, its companion's).
     */
    public static <I> I view(Object value, Class<I> view) {
        if (value == null || view.isInstance(value)) {
            return view.cast(value);
        }
        Type type = Type.Of(value);
        return view.cast(Proxy.newProxyInstance(view.getClassLoader(), new Class<?>[] {view}, (proxy, m, args) -> switch (m.getName()) {
            case "equals" -> value.equals(args[0]);
            case "hashCode" -> value.hashCode();
            case "toString" -> value.toString();
            default -> run(type, value, m.getName(), args == null ? new Object[0] : args);
        }));
    }

    /** {@code value}'s method {@code name} of {@code args.length} parameters, run. */
    private static Object run(Type type, Object value, String name, Object[] args) {
        for (MethodInfo m : type.GetMethods()) {
            if (m.Name().equals(name) && !m.IsStatic() && m.GetParameters().length == args.length) {
                return m.Invoke(value, args);
            }
        }
        throw new IllegalStateException("`" + type + "` has no method `" + name + "` of " + args.length + " parameters");
    }

    /** A new {@code type} made with nothing ({@code new T()}): a value's zero, else its constructor's. */
    public static Object made(Type type) {
        Object zero = zero(type);
        if (zero != null) {
            return zero;
        }
        for (ConstructorInfo c : type.GetConstructors()) {
            if (c.GetParameters().length == 0) {
                return c.Invoke(new Object[0]);
            }
        }
        throw new IllegalStateException("`" + type + "` isn't made with nothing");
    }

    /** {@code default(T)} of {@code type}: a built-in value's zero, else {@code null}. */
    public static Object zero(Type type) {
        for (Map.Entry<Class<?>, Object> z : ZEROS.entrySet()) {
            if (Type.Of(z.getKey()) == type) {
                return z.getValue();
            }
        }
        return null;
    }
}
