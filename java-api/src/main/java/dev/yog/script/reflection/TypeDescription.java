package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.ArrayList;
import java.util.List;
import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import java.util.function.Function;

/**
 * What the compiler wrote of a declared type beside its name: the class
 * it inherits, the interfaces it implements, its members — each with what
 * reads, writes or calls it. Filled the first time it's asked for, its
 * types seen with the type's own parameters.
 */
public final class TypeDescription {
    private final Type owner;
    private Type base;
    private final List<Type> interfaces = new ArrayList<>();
    private final List<Member> members = new ArrayList<>();
    /** The method or constructor its parameters ({@link #Takes}) are added to. */
    private MethodBase last;

    TypeDescription(Type owner) {
        this.owner = owner;
    }

    /** The described type's type parameter at {@code position}. */
    public Type Parameter(int position) {
        return owner.GetGenericArguments()[position];
    }

    /** The last method's own type parameter at {@code position}. */
    public Type MethodParameter(int position) {
        return ((MethodInfo) last).GetGenericArguments()[position];
    }

    public TypeDescription Base(Type base) {
        this.base = base;
        return this;
    }

    public TypeDescription Interface(Type implemented) {
        interfaces.add(implemented);
        return this;
    }

    /** Field {@code name}: read by {@code get}, written by {@code set} ({@code null} when it's set only where it's made). */
    public TypeDescription Field(String name, int traits, Type type, Function<Object, Object> get, BiConsumer<Object, Object> set) {
        members.add(new FieldInfo(parts(name, traits), type, get, set));
        return this;
    }

    public TypeDescription Property(String name, int traits, Type type, Function<Object, Object> get, BiConsumer<Object, Object> set) {
        members.add(new PropertyInfo(parts(name, traits), type, get, set));
        return this;
    }

    /** Method {@code name}, of type parameters {@code generics}; its result ({@link #Returns}) and parameters ({@link #Takes}) after. */
    public TypeDescription Method(String name, int traits, String[] generics, BiFunction<Object, Object[], Object> invoke) {
        MethodInfo m = new MethodInfo(parts(name, traits), generics, invoke);
        members.add(m);
        last = m;
        return this;
    }

    public TypeDescription Returns(Type type) {
        ((MethodInfo) last).returns = type;
        return this;
    }

    public TypeDescription Constructor(int traits, Function<Object[], Object> make) {
        ConstructorInfo c = new ConstructorInfo(parts(ConstructorInfo.NAME, traits), make);
        members.add(c);
        last = c;
        return this;
    }

    /** The last method's or constructor's next parameter. */
    public TypeDescription Takes(String name, Type type, boolean hasDefault) {
        last.parameters.add(new ParameterInfo(name, type, last.parameters.size(), hasDefault));
        return this;
    }

    private Member.Parts parts(String name, int traits) {
        return new Member.Parts(name, traits, owner);
    }

    Type base() {
        return base;
    }

    List<Type> interfaces() {
        return interfaces;
    }

    List<Member> members() {
        return members;
    }
}
