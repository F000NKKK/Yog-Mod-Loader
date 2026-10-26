package dev.yog.script;

import dev.yog.script.reflection.ArrayType;
import dev.yog.script.reflection.ConstructorInfo;
import dev.yog.script.reflection.DeclaredType;
import dev.yog.script.reflection.FieldInfo;
import dev.yog.script.reflection.ForeignType;
import dev.yog.script.reflection.MemberInfo;
import dev.yog.script.reflection.MemberTypes;
import dev.yog.script.reflection.Members;
import dev.yog.script.reflection.MethodInfo;
import dev.yog.script.reflection.PropertyInfo;
import dev.yog.script.reflection.TypeDescription;
import dev.yog.script.reflection.library.Library;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Consumer;

/**
 * A type as a value: Yog Script's {@code System.Type}. One object per type
 * (a constructed one and an array cached by what it's made of), so
 * {@code ==} tells them apart. A declared type's description is what the
 * compiler wrote beside it ({@code $TYPE}, or the library's companion).
 */
public abstract class Type extends MemberInfo {
    // What a declared type is (Declare's traits).
    public static final int CLASS = 1;
    public static final int STRUCT = 1 << 1;
    public static final int INTERFACE = 1 << 2;
    public static final int ENUM = 1 << 3;
    public static final int DELEGATE = 1 << 4;
    public static final int ABSTRACT = 1 << 5;
    public static final int SEALED = 1 << 6;
    public static final int PRIMITIVE = 1 << 7;

    /** The field a compiled type's description is in. */
    private static final String DESCRIPTION_FIELD = "$TYPE";
    private static final ClassValue<Type> OF_CLASS = new ClassValue<>() {
        @Override
        protected Type computeValue(Class<?> c) {
            return find(c);
        }
    };

    private final Map<Integer, Type> arrays = new ConcurrentHashMap<>();

    /** Type {@code fullName} (its namespace, a dot, its name) of class {@code java}: what it is ({@link #CLASS}…), its type parameters, its bases. */
    public static Type Declare(Class<?> java, String fullName, int traits, String[] parameters, Consumer<TypeDescription> describe) {
        return new DeclaredType(java, fullName, traits, parameters, describe);
    }

    /** The type of class {@code c}: its description, the library's, or a Java class's own. */
    public static Type Of(Class<?> c) {
        return OF_CLASS.get(c);
    }

    /** What {@code value} is. */
    public static Type Of(java.lang.Object value) {
        if (value == null) {
            throw new NullPointerException("`null` has no type");
        }
        return Of(value.getClass());
    }

    /**
     * Class {@code c}'s type: its own description; the library's companion
     * of it; for a lambda (a delegate's) or an enum's case (a record of its
     * sealed interface), what it's of; else Java's class as it is.
     */
    private static Type find(Class<?> c) {
        if (c.isArray()) {
            return Of(c.getComponentType()).MakeArrayType();
        }
        Type own = declared(c);
        if (own != null) {
            return own;
        }
        Type known = Library.of(c);
        if (known != null) {
            return known;
        }
        for (Class<?> of : c.getInterfaces()) {
            Type made = c.isHidden() || of.isSealed() ? declared(of) : null;
            if (made != null) {
                return made;
            }
        }
        return new ForeignType(c);
    }

    /** The description {@code c} declares itself ({@code $TYPE}), if it does. */
    private static Type declared(Class<?> c) {
        try {
            return c.getDeclaredField(DESCRIPTION_FIELD).get(null) instanceof Type t ? t : null;
        } catch (ReflectiveOperationException e) {
            // Not compiled from Yog Script.
            return null;
        }
    }

    // What it is.

    /** Its traits ({@link #CLASS}…; an array's and a parameter's their own). */
    protected abstract int traits();

    @Override
    public MemberTypes MemberType() {
        return MemberTypes.TypeInfo;
    }

    @Override
    public Type DeclaringType() {
        return null;
    }

    @Override
    protected List<java.util.function.Supplier<java.lang.Object>> attributeMakers() {
        return List.of();
    }

    /** A type's attributes are inherited from its base's, as .NET's are by default. */
    @Override
    protected MemberInfo inheritedFrom() {
        return BaseType();
    }

    public abstract String Namespace();

    public abstract String FullName();

    public abstract Type BaseType();

    /** The interfaces it implements itself (its bases' aside). */
    protected abstract List<Type> ownInterfaces();

    public boolean IsClass() {
        return (traits() & (CLASS | DELEGATE)) != 0 || IsArray();
    }

    public boolean IsInterface() {
        return (traits() & INTERFACE) != 0;
    }

    public boolean IsValueType() {
        return (traits() & (STRUCT | ENUM)) != 0;
    }

    public boolean IsEnum() {
        return (traits() & ENUM) != 0;
    }

    public boolean IsArray() {
        return false;
    }

    public boolean IsAbstract() {
        return (traits() & (ABSTRACT | INTERFACE)) != 0;
    }

    public boolean IsSealed() {
        return (traits() & (SEALED | STRUCT | ENUM | DELEGATE)) != 0 || IsArray();
    }

    public boolean IsPrimitive() {
        return (traits() & PRIMITIVE) != 0;
    }

    public boolean IsGenericType() {
        return IsGenericTypeDefinition() || IsConstructedGenericType();
    }

    public boolean IsGenericTypeDefinition() {
        return false;
    }

    public boolean IsConstructedGenericType() {
        return false;
    }

    public boolean IsGenericParameter() {
        return false;
    }

    public int GenericParameterPosition() {
        throw new IllegalStateException("`" + Name() + "` isn't a type parameter");
    }

    // Generics and arrays.

    public Type[] GetGenericArguments() {
        return new Type[0];
    }

    public Type GetGenericTypeDefinition() {
        throw new IllegalStateException("`" + Name() + "` isn't generic");
    }

    public Type MakeGenericType(Type[] typeArguments) {
        throw new IllegalStateException("`" + Name() + "` isn't a generic type's definition");
    }

    public Type GetElementType() {
        return null;
    }

    public int GetArrayRank() {
        throw new IllegalStateException("`" + Name() + "` isn't an array");
    }

    public Type MakeArrayType() {
        return MakeArrayType(1);
    }

    public Type MakeArrayType(int rank) {
        return arrays.computeIfAbsent(rank, r -> new ArrayType(this, r));
    }

    /** This type with its type parameters as {@code arguments} say (a definition's own as they are). */
    public abstract Type substitute(Type[] arguments);

    // What it inherits.

    public Type[] GetInterfaces() {
        Set<Type> all = new LinkedHashSet<>();
        for (Type t = this; t != null; t = t.BaseType()) {
            for (Type i : t.ownInterfaces()) {
                all.add(i);
                all.addAll(List.of(i.GetInterfaces()));
            }
        }
        return all.toArray(new Type[0]);
    }

    public boolean IsAssignableFrom(Type c) {
        if (c == null) {
            return false;
        }
        if (c == this || this == Of(java.lang.Object.class)) {
            return true;
        }
        return c.IsSubclassOf(this) || (IsInterface() && List.of(c.GetInterfaces()).contains(this));
    }

    public boolean IsInstanceOfType(java.lang.Object o) {
        return o != null && IsAssignableFrom(Of(o));
    }

    public boolean IsSubclassOf(Type c) {
        for (Type t = BaseType(); t != null; t = t.BaseType()) {
            if (t == c) {
                return true;
            }
        }
        return false;
    }

    // Its members.

    /** Its own members as described: none for what isn't declared. */
    public List<MemberInfo> members() {
        return List.of();
    }

    public MemberInfo[] GetMembers() {
        return Members.of(this, MemberInfo.class).toArray(new MemberInfo[0]);
    }

    public FieldInfo[] GetFields() {
        return Members.of(this, FieldInfo.class).toArray(new FieldInfo[0]);
    }

    public FieldInfo GetField(String name) {
        return Members.named(this, FieldInfo.class, name);
    }

    public PropertyInfo[] GetProperties() {
        return Members.of(this, PropertyInfo.class).toArray(new PropertyInfo[0]);
    }

    public PropertyInfo GetProperty(String name) {
        return Members.named(this, PropertyInfo.class, name);
    }

    public MethodInfo[] GetMethods() {
        return Members.of(this, MethodInfo.class).toArray(new MethodInfo[0]);
    }

    public MethodInfo GetMethod(String name) {
        return Members.named(this, MethodInfo.class, name);
    }

    public MethodInfo GetMethod(String name, Type[] types) {
        return Members.taking(this, MethodInfo.class, name, types);
    }

    public ConstructorInfo[] GetConstructors() {
        return Members.of(this, ConstructorInfo.class).toArray(new ConstructorInfo[0]);
    }

    public ConstructorInfo GetConstructor(Type[] types) {
        return Members.taking(this, ConstructorInfo.class, ConstructorInfo.NAME, types);
    }

    /** {@code types} each as {@code arguments} say. */
    protected static List<Type> substituted(List<Type> types, Type[] arguments) {
        List<Type> out = new ArrayList<>(types.size());
        for (Type t : types) {
            out.add(t.substitute(arguments));
        }
        return out;
    }

    public String ToString() {
        return toString();
    }

    @Override
    public String toString() {
        return Namespace() == null ? Name() : Namespace() + "." + Name();
    }
}
