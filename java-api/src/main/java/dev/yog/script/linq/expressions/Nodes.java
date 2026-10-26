package dev.yog.script.linq.expressions;

import dev.yog.script.collections.generic.List;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;

/** What the nodes share: their lists, their arguments run, a tree made a delegate. */
final class Nodes {
    /** The one method of a delegate's interface (the CLR's). */
    private static final String INVOKE = "Invoke";
    private static final String OPEN = "(";
    private static final String CLOSE = ")";
    private static final String SEP = ", ";

    private Nodes() {}

    static <T> List<T> listed(T[] items) {
        List<T> out = new List<>();
        for (T item : items) {
            out.Add(item);
        }
        return out;
    }

    static Object[] evaluated(Expression[] arguments, Scope scope) {
        Object[] out = new Object[arguments.length];
        for (int i = 0; i < arguments.length; i++) {
            out[i] = arguments[i].evaluate(scope);
        }
        return out;
    }

    /** {@code (a, b)}. */
    static String joined(Object[] items) {
        StringBuilder out = new StringBuilder(OPEN);
        for (int i = 0; i < items.length; i++) {
            out.append(i == 0 ? "" : SEP).append(items[i]);
        }
        return out.append(CLOSE).toString();
    }

    /** A {@code delegate} that runs {@code body} with its arguments as {@code parameters}. */
    static <D> D compiled(Class<D> delegate, Expression body, ParameterExpression[] parameters) {
        Object made = Proxy.newProxyInstance(delegate.getClassLoader(), new Class<?>[] {delegate}, (self, method, args) -> {
            if (!method.getName().equals(INVOKE)) {
                return ofObject(self, method, args);
            }
            Scope scope = new Scope(null);
            for (int i = 0; i < parameters.length; i++) {
                scope.bind(parameters[i], args[i]);
            }
            return body.evaluate(scope);
        });
        return delegate.cast(made);
    }

    /** {@code Object}'s methods of a delegate made of a tree: its identity's. */
    private static Object ofObject(Object self, Method method, Object[] args) {
        return switch (method.getName()) {
            case "equals" -> self == args[0];
            case "hashCode" -> System.identityHashCode(self);
            default -> self.getClass().getName();
        };
    }
}
