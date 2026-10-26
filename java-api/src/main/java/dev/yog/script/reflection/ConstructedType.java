package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;

/** A generic type made of its definition and arguments ({@code List<i32>}). */
final class ConstructedType extends Type {
    private final DeclaredType definition;
    private final Type[] arguments;
    private volatile List<MemberInfo> members;

    ConstructedType(DeclaredType definition, Type[] arguments) {
        this.definition = definition;
        this.arguments = arguments;
    }

    @Override
    protected int traits() {
        return definition.traits();
    }

    @Override
    public String Name() {
        return definition.Name();
    }

    @Override
    public String Namespace() {
        return definition.Namespace();
    }

    /** {@code System.Collections.Generic.List`1[[System.Int32]]}; {@code null} while an argument is a parameter. */
    @Override
    public String FullName() {
        for (Type a : arguments) {
            if (a.FullName() == null) {
                return null;
            }
        }
        return definition.FullName() + listed(a -> "[" + a.FullName() + "]");
    }

    @Override
    public Type BaseType() {
        Type base = definition.description().base();
        return base == null ? definition.BaseType() : base.substitute(arguments);
    }

    @Override
    protected List<Type> ownInterfaces() {
        return substituted(definition.ownInterfaces(), arguments);
    }

    /** Its definition's, their types made of its arguments. */
    @Override
    public List<MemberInfo> members() {
        List<MemberInfo> made = members;
        if (made == null) {
            List<MemberInfo> out = new ArrayList<>();
            for (Member m : definition.description().members()) {
                out.add(m.of(this, arguments));
            }
            made = List.copyOf(out);
            members = made;
        }
        return made;
    }

    @Override
    public boolean IsConstructedGenericType() {
        return true;
    }

    @Override
    public Type[] GetGenericArguments() {
        return arguments.clone();
    }

    @Override
    public Type GetGenericTypeDefinition() {
        return definition;
    }

    @Override
    public Type substitute(Type[] context) {
        Type[] made = new Type[arguments.length];
        for (int i = 0; i < arguments.length; i++) {
            made[i] = arguments[i].substitute(context);
        }
        return definition.MakeGenericType(made);
    }

    /** {@code List`1[System.Int32]}, as the CLR writes it. */
    @Override
    public String toString() {
        return definition + listed(Type::toString);
    }

    private String listed(Function<Type, String> each) {
        StringBuilder out = new StringBuilder("[");
        for (int i = 0; i < arguments.length; i++) {
            out.append(i == 0 ? "" : ",").append(each.apply(arguments[i]));
        }
        return out.append(']').toString();
    }
}
