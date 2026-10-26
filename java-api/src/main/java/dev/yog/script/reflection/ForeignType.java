package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.List;

/** A Java class Yog Script doesn't declare (a game's): seen by its Java names, without members. */
public final class ForeignType extends Type {
    private final Class<?> java;

    public ForeignType(Class<?> java) {
        this.java = java;
    }

    @Override
    protected int traits() {
        int traits = java.isInterface() ? INTERFACE : java.isEnum() ? ENUM : CLASS;
        if (Modifier.isAbstract(java.getModifiers())) {
            traits |= ABSTRACT;
        }
        if (Modifier.isFinal(java.getModifiers())) {
            traits |= SEALED;
        }
        return traits;
    }

    @Override
    public String Name() {
        return java.getSimpleName();
    }

    @Override
    public String Namespace() {
        return java.getPackageName();
    }

    @Override
    public String FullName() {
        return java.getName();
    }

    @Override
    public Type BaseType() {
        Class<?> base = java.getSuperclass();
        if (base != null) {
            return Type.Of(base);
        }
        return java.isInterface() ? null : Type.Of(java.lang.Object.class);
    }

    @Override
    protected List<Type> ownInterfaces() {
        List<Type> out = new ArrayList<>();
        for (Class<?> i : java.getInterfaces()) {
            out.add(Type.Of(i));
        }
        return out;
    }

    @Override
    public Type substitute(Type[] arguments) {
        return this;
    }

    @Override
    public String toString() {
        return FullName();
    }
}
