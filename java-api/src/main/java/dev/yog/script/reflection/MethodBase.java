package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.ArrayList;
import java.util.List;

/** What a method and a constructor share: their parameters. Yog Script's {@code System.Reflection.MethodBase}. */
public abstract class MethodBase extends Member {
    final List<ParameterInfo> parameters = new ArrayList<>();

    MethodBase(Member.Parts parts) {
        super(parts);
    }

    public ParameterInfo[] GetParameters() {
        return parameters.toArray(new ParameterInfo[0]);
    }

    public boolean IsAbstract() {
        return (memberTraits() & ABSTRACT) != 0;
    }

    public boolean IsVirtual() {
        return (memberTraits() & (VIRTUAL | ABSTRACT)) != 0;
    }

    /** Whether it takes exactly {@code types}, in order. */
    boolean takes(Type[] types) {
        if (types.length != parameters.size()) {
            return false;
        }
        for (int i = 0; i < types.length; i++) {
            if (parameters.get(i).ParameterType() != types[i]) {
                return false;
            }
        }
        return true;
    }

    /** {@code args}, checked to be as many as it takes ({@code null}: none). */
    Object[] counted(Object[] args) {
        Object[] given = args == null ? new Object[0] : args;
        if (given.length != parameters.size()) {
            throw new IllegalArgumentException("`" + Name() + "` takes " + parameters.size() + " arguments, not " + given.length);
        }
        return given;
    }

    /** Its parameters, as {@code arguments} say, into {@code into}. */
    void parametersInto(MethodBase into, Type[] arguments) {
        for (ParameterInfo p : parameters) {
            into.parameters.add(p.substituted(arguments));
        }
    }
}
