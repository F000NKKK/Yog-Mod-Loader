package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.List;

/** A type parameter ({@code T}) of a generic type or method: its name, its place. */
final class GenericParameterType extends Type {
    private final String name;
    private final int position;
    private final Type declaring;
    /** A method's: its type's arguments don't stand for it. */
    private final boolean ofMethod;

    GenericParameterType(String name, int position, Type declaring, boolean ofMethod) {
        this.name = name;
        this.position = position;
        this.declaring = declaring;
        this.ofMethod = ofMethod;
    }

    @Override
    protected int traits() {
        return 0;
    }

    @Override
    public String Name() {
        return name;
    }

    @Override
    public String Namespace() {
        return declaring.Namespace();
    }

    @Override
    public String FullName() {
        return null;
    }

    @Override
    public Type DeclaringType() {
        return declaring;
    }

    @Override
    public Type BaseType() {
        return Type.Of(java.lang.Object.class);
    }

    @Override
    protected List<Type> ownInterfaces() {
        return List.of();
    }

    @Override
    public boolean IsGenericParameter() {
        return true;
    }

    @Override
    public int GenericParameterPosition() {
        return position;
    }

    /** What stands for it where {@code arguments} are its type's (a method's stays). */
    @Override
    public Type substitute(Type[] arguments) {
        return !ofMethod && position < arguments.length ? arguments[position] : this;
    }

    @Override
    public String toString() {
        return name;
    }
}
