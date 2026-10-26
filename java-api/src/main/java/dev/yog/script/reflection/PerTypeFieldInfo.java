package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.function.BiConsumer;
import java.util.function.Function;

/**
 * A generic type's static field, one per list of its arguments
 * ({@code Box<int>.Count}): its constructed types' field reads and writes
 * their arguments' one; the definition's has none to reach, as .NET's.
 */
final class PerTypeFieldInfo extends FieldInfo {
    private final Function<Type[], Object> get;
    private final BiConsumer<Type[], Object> set;

    PerTypeFieldInfo(Member.Parts parts, Type type, Function<Type[], Object> get, BiConsumer<Type[], Object> set) {
        super(parts, type, o -> open(), null);
        this.get = get;
        this.set = set;
    }

    private static Object open() {
        throw new IllegalStateException("a generic type's static field is reached through its type's arguments");
    }

    @Override
    Member of(Type declaring, Type[] arguments) {
        BiConsumer<Object, Object> written = set == null ? null : (o, v) -> set.accept(arguments, v);
        return new FieldInfo(partsOf(declaring), FieldType().substitute(arguments), o -> get.apply(arguments), written);
    }
}
