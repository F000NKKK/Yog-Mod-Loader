package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.ArrayList;
import java.util.List;

/**
 * What the compiler wrote of a declared type beside its name: the class
 * it inherits, the interfaces it implements. Filled the first time it's
 * asked for, its types seen with the type's own parameters.
 */
public final class TypeDescription {
    private final Type owner;
    private Type base;
    private final List<Type> interfaces = new ArrayList<>();

    TypeDescription(Type owner) {
        this.owner = owner;
    }

    /** The described type's type parameter at {@code position}. */
    public Type Parameter(int position) {
        return owner.GetGenericArguments()[position];
    }

    public TypeDescription Base(Type base) {
        this.base = base;
        return this;
    }

    public TypeDescription Interface(Type implemented) {
        interfaces.add(implemented);
        return this;
    }

    Type base() {
        return base;
    }

    List<Type> interfaces() {
        return interfaces;
    }
}
