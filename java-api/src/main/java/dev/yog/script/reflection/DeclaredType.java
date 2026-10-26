package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Consumer;

/** A type as declared: a definition, of its type parameters when it's generic. */
public final class DeclaredType extends Type {
    /** What sits between a generic type's name and its arity ({@code List`1}), as the CLR writes it. */
    static final String ARITY = "`";
    private static final char NAMESPACE_SEP = '.';

    private final Class<?> java;
    private final String name;
    private final String namespace;
    private final int traits;
    private final Type[] parameters;
    private final Consumer<TypeDescription> describe;
    private volatile TypeDescription described;
    private final Map<List<Type>, Type> constructed = new ConcurrentHashMap<>();

    public DeclaredType(Class<?> java, String fullName, int traits, String[] parameters, Consumer<TypeDescription> describe) {
        this.java = java;
        int dot = fullName.lastIndexOf(NAMESPACE_SEP);
        this.name = fullName.substring(dot + 1);
        this.namespace = dot < 0 ? null : fullName.substring(0, dot);
        this.traits = traits;
        this.parameters = new Type[parameters.length];
        for (int i = 0; i < parameters.length; i++) {
            this.parameters[i] = new GenericParameterType(parameters[i], i, this, false);
        }
        this.describe = describe;
    }

    /** The class its objects are in Java. */
    public Class<?> javaClass() {
        return java;
    }

    TypeDescription description() {
        TypeDescription d = described;
        if (d == null) {
            synchronized (this) {
                if (described == null) {
                    TypeDescription made = new TypeDescription(this);
                    describe.accept(made);
                    described = made;
                }
                d = described;
            }
        }
        return d;
    }

    @Override
    protected int traits() {
        return traits;
    }

    @Override
    public String Name() {
        return parameters.length == 0 ? name : name + ARITY + parameters.length;
    }

    @Override
    public String Namespace() {
        return namespace;
    }

    @Override
    public String FullName() {
        return toString();
    }

    @Override
    public Type BaseType() {
        Type base = description().base();
        if (base != null || IsInterface()) {
            return base;
        }
        Type object = Type.Of(java.lang.Object.class);
        return object == this ? null : object;
    }

    @Override
    protected List<Type> ownInterfaces() {
        return description().interfaces();
    }

    @Override
    public List<MemberInfo> members() {
        return List.copyOf(description().members());
    }

    @Override
    public boolean IsGenericTypeDefinition() {
        return parameters.length > 0;
    }

    @Override
    public Type[] GetGenericArguments() {
        return parameters.clone();
    }

    @Override
    public Type GetGenericTypeDefinition() {
        return IsGenericTypeDefinition() ? this : super.GetGenericTypeDefinition();
    }

    @Override
    public Type MakeGenericType(Type[] typeArguments) {
        if (typeArguments.length != parameters.length) {
            throw new IllegalArgumentException("`" + Name() + "` takes " + parameters.length + " type arguments, not " + typeArguments.length);
        }
        if (Arrays.equals(typeArguments, parameters)) {
            return this;
        }
        return constructed.computeIfAbsent(List.of(typeArguments), args -> new ConstructedType(this, typeArguments.clone()));
    }

    @Override
    public Type substitute(Type[] arguments) {
        return parameters.length == 0 ? this : MakeGenericType(arguments);
    }
}
