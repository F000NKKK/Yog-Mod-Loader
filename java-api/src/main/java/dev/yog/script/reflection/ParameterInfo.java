package dev.yog.script.reflection;

import dev.yog.script.Type;

/** A method's or a constructor's parameter: Yog Script's {@code System.Reflection.ParameterInfo}. */
public class ParameterInfo {
    private final String name;
    private final Type type;
    private final int position;
    private final boolean hasDefault;

    ParameterInfo(String name, Type type, int position, boolean hasDefault) {
        this.name = name;
        this.type = type;
        this.position = position;
        this.hasDefault = hasDefault;
    }

    public String Name() {
        return name;
    }

    public Type ParameterType() {
        return type;
    }

    public int Position() {
        return position;
    }

    /** Whether a call may leave it out: it has a default. */
    public boolean HasDefaultValue() {
        return hasDefault;
    }

    public boolean IsOptional() {
        return hasDefault;
    }

    ParameterInfo substituted(Type[] arguments) {
        return new ParameterInfo(name, type.substitute(arguments), position, hasDefault);
    }

    @Override
    public String toString() {
        return type + " " + name;
    }
}
