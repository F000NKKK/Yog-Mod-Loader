package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.ArrayList;
import java.util.List;
import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.Supplier;

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
    private final List<Supplier<Object>> attributes = new ArrayList<>();
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

    /** A generic type's static field {@code name}, one per list of its arguments: read and written by them. */
    public TypeDescription PerTypeField(String name, int traits, Type type, Function<Type[], Object> get, BiConsumer<Type[], Object> set) {
        members.add(new PerTypeFieldInfo(parts(name, traits), type, get, set));
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

    /** The last method compiled from Yog Script: handed its type arguments first. */
    public TypeDescription TakesTypes() {
        ((MethodInfo) last).takesTypes = true;
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
        return Takes(name, type, hasDefault, 0);
    }

    /** The last method's or constructor's next parameter, passed as {@code passing} says ({@link ParameterInfo#OUT}, none). */
    public TypeDescription Takes(String name, Type type, boolean hasDefault, int passing) {
        last.parameters.add(new ParameterInfo(name, type, last.parameters.size(), hasDefault, passing));
        return this;
    }

    /** An attribute ({@code make} makes it): on the member described last; before any, on the type. */
    public TypeDescription Attribute(Supplier<Object> make) {
        Member to = members.isEmpty() ? null : members.get(members.size() - 1);
        (to == null ? attributes : to.attributes).add(make);
        return this;
    }

    private Member.Parts parts(String name, int traits) {
        return new Member.Parts(name, traits, owner, new ArrayList<>());
    }

    Type base() {
        return base;
    }

    List<Type> interfaces() {
        return interfaces;
    }

    List<Supplier<Object>> attributes() {
        return attributes;
    }

    List<Member> members() {
        return members;
    }
}
