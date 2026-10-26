package dev.yog.script.linq.expressions;

import java.util.IdentityHashMap;
import java.util.Map;

/** What a tree's parameters stand for while it runs. */
final class Scope {
    private final Map<ParameterExpression, Object> values = new IdentityHashMap<>();
    private final Scope outer;

    Scope(Scope outer) {
        this.outer = outer;
    }

    void bind(ParameterExpression p, Object value) {
        values.put(p, value);
    }

    Object of(ParameterExpression p) {
        for (Scope s = this; s != null; s = s.outer) {
            if (s.values.containsKey(p)) {
                return s.values.get(p);
            }
        }
        throw new IllegalStateException("the parameter `" + p.Name() + "` isn't the lambda's");
    }
}
