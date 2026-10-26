package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.ArrayList;
import java.util.List;

/**
 * A type's members as {@code GetMethods(…)} and the rest give them, as
 * .NET's: those {@link BindingFlags} ask for — public or not, of an
 * instance or static, its own or inherited too (a base's private ones
 * never; its static ones only flattened) — those nearer hiding what they
 * replace.
 */
public final class Members {
    private Members() {}

    /** What {@code GetMethods()} without flags asks for: public, instance and static. */
    private static BindingFlags standard() {
        return BindingFlags.of(BindingFlags.Public.value() | BindingFlags.Instance.value() | BindingFlags.Static.value());
    }

    public static <M extends MemberInfo> List<M> of(Type t, Class<M> kind) {
        return of(t, kind, standard());
    }

    /** {@code t}'s members of {@code kind} that {@code flags} asks for. */
    public static <M extends MemberInfo> List<M> of(Type t, Class<M> kind, BindingFlags flags) {
        List<M> out = new ArrayList<>();
        boolean own = true;
        for (Type at = t; at != null; at = flags.HasFlag(BindingFlags.DeclaredOnly) ? null : at.BaseType()) {
            for (MemberInfo m : at.members()) {
                if (kind.isInstance(m) && asked(m, flags, own) && !hidden(out, m)) {
                    out.add(kind.cast(m));
                }
            }
            own = false;
        }
        return out;
    }

    /** Whether {@code flags} asks for {@code m} ({@code own}: of the type itself, not a base). */
    private static boolean asked(MemberInfo m, BindingFlags flags, boolean own) {
        Member member = (Member) m;
        boolean visible = m.isPublic() ? flags.HasFlag(BindingFlags.Public) : flags.HasFlag(BindingFlags.NonPublic);
        boolean placed = member.IsStatic() ? flags.HasFlag(BindingFlags.Static) : flags.HasFlag(BindingFlags.Instance);
        if (own) {
            return visible && placed;
        }
        boolean inherits = !member.IsPrivate()
                && !(m instanceof ConstructorInfo)
                && (!member.IsStatic() || flags.HasFlag(BindingFlags.FlattenHierarchy));
        return visible && placed && inherits;
    }

    public static <M extends MemberInfo> M named(Type t, Class<M> kind, String name) {
        return named(t, kind, name, standard());
    }

    /** {@code t}'s member of {@code kind} named {@code name} that {@code flags} asks for; {@code null} for none, a panic for several. */
    public static <M extends MemberInfo> M named(Type t, Class<M> kind, String name, BindingFlags flags) {
        boolean anyCase = flags.HasFlag(BindingFlags.IgnoreCase);
        M found = null;
        for (M m : of(t, kind, flags)) {
            if (anyCase ? m.Name().equalsIgnoreCase(name) : m.Name().equals(name)) {
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
