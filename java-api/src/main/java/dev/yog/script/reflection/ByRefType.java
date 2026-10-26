package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.List;

/** A type passed by reference ({@code Int32&}): a {@code ref} or {@code out} parameter's. */
public final class ByRefType extends Type {
    private static final String MARK = "&";

    private final Type element;

    public ByRefType(Type element) {
        this.element = element;
    }

    @Override
    protected int traits() {
        return 0;
    }

    @Override
    public String Name() {
        return element.Name() + MARK;
    }

    @Override
    public String Namespace() {
        return element.Namespace();
    }

    @Override
    public String FullName() {
        String of = element.FullName();
        return of == null ? null : of + MARK;
    }

    @Override
    public Type BaseType() {
        return null;
    }

    @Override
    protected List<Type> ownInterfaces() {
        return List.of();
    }

    @Override
    public boolean IsByRef() {
        return true;
    }

    @Override
    public Type GetElementType() {
        return element;
    }

    @Override
    public Type substitute(Type[] arguments) {
        return element.substitute(arguments).MakeByRefType();
    }

    @Override
    public String toString() {
        return element + MARK;
    }
}
