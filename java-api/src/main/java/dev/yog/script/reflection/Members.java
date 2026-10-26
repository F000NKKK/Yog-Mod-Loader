package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.ArrayList;
import java.util.List;

/**
 * A type's members as {@code GetMethods()} and the rest give them, as .NET's
 * default: the public ones, its own and — of the instance's — those it
 * inherits that nothing nearer hides.
 */
public final class Members {
    private Members() {}

    /** {@code t}'s public members of {@code kind}. */
    public static <M extends MemberInfo> List<M> of(Type t, Class<M> kind) {
        List<M> out = new ArrayList<>();
        boolean own = true;
        for (Type at = t; at != null; at = at.BaseType()) {
            for (MemberInfo m : at.members()) {
                if (!kind.isInstance(m) || !m.isPublic()) {
                    continue;
                }
                boolean inherits = !own && (((Member) m).IsStatic() || m instanceof ConstructorInfo);
                if (!inherits && !hidden(out, m)) {
                    out.add(kind.cast(m));
                }
            }
            own = false;
        }
        return out;
    }

    /** {@code t}'s public member of {@code kind} named {@code name}; {@code null} when there's none, a panic when there's more than one. */
    public static <M extends MemberInfo> M named(Type t, Class<M> kind, String name) {
        M found = null;
        for (M m : of(t, kind)) {
            if (m.Name().equals(name)) {
                if (found != null) {
                    throw new IllegalStateException("`" + t.Name() + "` has more than one `" + name + "`: say which by its parameters' types");
                }
                found = m;
            }
        }
        return found;
    }

    /** {@code t}'s public method or constructor (of {@code kind}) named {@code name} taking {@code types}. */
    public static <M extends MethodBase> M taking(Type t, Class<M> kind, String name, Type[] types) {
        for (M m : of(t, kind)) {
            if (m.Name().equals(name) && m.takes(types)) {
                return m;
            }
        }
        return null;
    }

    /** Whether one nearer (of {@code found}) hides {@code m}: of its name, and its parameters for a method. */
    private static boolean hidden(List<? extends MemberInfo> found, MemberInfo m) {
        for (MemberInfo f : found) {
            if (!f.Name().equals(m.Name())) {
                continue;
            }
            if (!(m instanceof MethodBase method) || !(f instanceof MethodBase nearer)) {
                return true;
            }
            Type[] types = new Type[method.parameters.size()];
            for (int i = 0; i < types.length; i++) {
                types[i] = method.parameters.get(i).ParameterType();
            }
            if (nearer.takes(types)) {
                return true;
            }
        }
        return false;
    }
}
