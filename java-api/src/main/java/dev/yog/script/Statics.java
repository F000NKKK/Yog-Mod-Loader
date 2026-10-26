package dev.yog.script;

import java.util.Arrays;
import java.util.List;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Function;

/**
 * A generic type's static fields, one set per list of type arguments, as
 * .NET keeps them ({@code Box<int>.Count} isn't {@code Box<string>.Count}):
 * each set made the first time its arguments are reached, its fields' first
 * values set then. Types are canonical, so their lists compare by them.
 */
public final class Statics<S> {
    private final ConcurrentHashMap<List<Type>, S> each = new ConcurrentHashMap<>();
    private final Function<Type[], S> make;

    public Statics(Function<Type[], S> make) {
        this.make = make;
    }

    /** The set of {@code arguments}' (made now, the first time they're reached). */
    public S of(Type[] arguments) {
        List<Type> key = Arrays.asList(arguments);
        S found = each.get(key);
        if (found != null) {
            return found;
        }
        // Made outside the map: a first value may reach another type's set.
        S made = make.apply(arguments);
        S raced = each.putIfAbsent(key, made);
        return raced != null ? raced : made;
    }
}
