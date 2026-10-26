package dev.yog.script;

/**
 * What the runtime's own generic types (the library's {@code [Intrinsic]}
 * ones) extend: the type arguments they're made with kept, as
 * {@link Typed} asks.
 */
public abstract class TypedObject implements Typed {
    private Type[] $types;

    @Override
    public Type[] $types() {
        return $types;
    }

    @Override
    public void $types(Type[] arguments) {
        this.$types = arguments;
    }
}
