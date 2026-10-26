package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.function.BiFunction;

/** A method: Yog Script's {@code System.Reflection.MethodInfo}, called by what the compiler wrote beside it. */
public class MethodInfo extends MethodBase {
    private final Type[] generics;
    private final BiFunction<Object, Object[], Object> invoke;
    Type returns;

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

    /** Its own type parameters. */
    public Type[] GetGenericArguments() {
        return generics.clone();
    }

    /** It called on {@code obj} ({@code null} for a static one) with {@code parameters}; what it gives ({@code null} for nothing). */
    public Object Invoke(Object obj, Object[] parameters) {
        if (invoke == null) {
            throw new IllegalStateException("`" + Name() + "` can't be called through Reflection yet");
        }
        return invoke.apply(obj, counted(parameters));
    }

    @Override
    Member of(Type declaring, Type[] arguments) {
        MethodInfo made = new MethodInfo(partsOf(declaring), generics, invoke);
        made.returns = returns.substitute(arguments);
        parametersInto(made, arguments);
        return made;
    }

    @Override
    public String toString() {
        return returns + " " + Name() + parameters.toString().replace('[', '(').replace(']', ')');
    }
}
