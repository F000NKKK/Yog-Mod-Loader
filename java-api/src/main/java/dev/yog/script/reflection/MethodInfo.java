package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.function.BiFunction;

/** A method: Yog Script's {@code System.Reflection.MethodInfo}, called by what the compiler wrote beside it. */
public class MethodInfo extends MethodBase {
    private final Type[] generics;
    private final BiFunction<Object, Object[], Object> invoke;
    Type returns;
    /** Compiled from Yog Script: handed its type arguments before its arguments. */
    boolean takesTypes;
    /** Made of its definition's type arguments ({@link #MakeGenericMethod}); {@code null} for a definition. */
    private Type[] made;

    MethodInfo(Member.Parts parts, String[] generics, BiFunction<Object, Object[], Object> invoke) {
        super(parts);
        this.generics = new Type[generics.length];
        for (int i = 0; i < generics.length; i++) {
            this.generics[i] = new GenericParameterType(generics[i], i, parts.declaring(), true);
        }
        this.invoke = invoke;
    }

    private MethodInfo(Member.Parts parts, Type[] generics, BiFunction<Object, Object[], Object> invoke) {
        super(parts);
        this.generics = generics;
        this.invoke = invoke;
    }

    @Override
    public MemberTypes MemberType() {
        return MemberTypes.Method;
    }

    public Type ReturnType() {
        return returns;
    }

    /** Generic: it has type parameters of its own (called erased: Java's generics are). */
    public boolean IsGenericMethod() {
        return generics.length > 0;
    }

    /** A generic one's definition: its type parameters not given arguments. */
    public boolean IsGenericMethodDefinition() {
        return generics.length > 0 && made == null;
    }

    /** Whether it can't be called yet: a generic one's definition. */
    public boolean ContainsGenericParameters() {
        return IsGenericMethodDefinition();
    }

    /** Its own type parameters; a made one's, their arguments. */
    public Type[] GetGenericArguments() {
        return (made == null ? generics : made).clone();
    }

    /** This definition made of {@code typeArguments}: what can be called. */
    public MethodInfo MakeGenericMethod(Type[] typeArguments) {
        if (!IsGenericMethodDefinition()) {
            throw new IllegalStateException("`" + Name() + "` isn't a generic method's definition");
        }
        if (typeArguments.length != generics.length) {
            throw new IllegalArgumentException("`" + Name() + "` takes " + generics.length + " type arguments, not " + typeArguments.length);
        }
        MethodInfo m = new MethodInfo(partsOf(DeclaringType()), generics, invoke);
        m.returns = returns;
        m.takesTypes = takesTypes;
        m.made = typeArguments.clone();
        m.parameters.addAll(parameters);
        return m;
    }

    /** It called on {@code obj} ({@code null} for a static one) with {@code parameters}; what it gives ({@code null} for nothing). */
    public Object Invoke(Object obj, Object[] parameters) {
        if (invoke == null) {
            throw new IllegalStateException("`" + Name() + "` can't be called through Reflection yet");
        }
        if (ContainsGenericParameters()) {
            throw new IllegalStateException("`" + Name() + "` is generic: call what MakeGenericMethod makes of it");
        }
        Object[] given = counted(parameters);
        if (!takesTypes || made == null) {
            return invoke.apply(obj, given);
        }
        // Its type arguments first; what it gives back to `ref`/`out` ones copied after.
        Object[] all = new Object[made.length + given.length];
        System.arraycopy(made, 0, all, 0, made.length);
        System.arraycopy(given, 0, all, made.length, given.length);
        Object result = invoke.apply(obj, all);
        System.arraycopy(all, made.length, given, 0, given.length);
        return result;
    }

    @Override
    Member of(Type declaring, Type[] arguments) {
        MethodInfo m = new MethodInfo(partsOf(declaring), generics, invoke);
        m.returns = returns.substitute(arguments);
        m.takesTypes = takesTypes;
        m.made = made;
        parametersInto(m, arguments);
        return m;
    }

    @Override
    public String toString() {
        return returns + " " + Name() + parameters.toString().replace('[', '(').replace(']', ')');
    }
}
