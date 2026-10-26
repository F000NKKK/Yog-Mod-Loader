package dev.yog.script.reflection;

import dev.yog.script.Type;

/** A method's or a constructor's parameter: Yog Script's {@code System.Reflection.ParameterInfo}. */
public class ParameterInfo {
    /** How it's passed, .NET's {@code ParameterAttributes}: {@code out} ({@code ref} none). */
    public static final int OUT = 1 << 1;

    private final String name;
    private final Type type;
    private final int position;
    private final boolean hasDefault;
    private final int passing;

    ParameterInfo(String name, Type type, int position, boolean hasDefault, int passing) {
        this.name = name;
        this.type = type;
        this.position = position;
        this.hasDefault = hasDefault;
        this.passing = passing;
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

    /** Given by the method ({@code out}): its type a by-reference one. */
    public boolean IsOut() {
        return (passing & OUT) != 0;
    }

    ParameterInfo substituted(Type[] arguments) {
        return new ParameterInfo(name, type.substitute(arguments), position, hasDefault, passing);
    }

    @Override
    public String toString() {
        return type + " " + name;
    }
}
