package dev.yog.script.reflection;

import dev.yog.script.Type;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Supplier;

/** A type's member as Reflection sees it: Yog Script's {@code System.Reflection.MemberInfo}. */
public abstract class MemberInfo {
    // What a member is (its description's traits).
    public static final int STATIC = 1;
    public static final int PRIVATE = 1 << 1;
    public static final int PROTECTED = 1 << 2;
    public static final int ABSTRACT = 1 << 3;
    public static final int VIRTUAL = 1 << 4;
    /** A field set only where it's made ({@code readonly}). */
    public static final int INIT_ONLY = 1 << 5;
    /** A {@code const}. */
    public static final int LITERAL = 1 << 6;

    public abstract String Name();

    public abstract Type DeclaringType();

    public abstract MemberTypes MemberType();

    /** Its traits ({@link #STATIC}…); a type's are none of these. */
    protected int memberTraits() {
        return 0;
    }

    /** What makes each of its attributes, as written on it. */
    protected abstract List<Supplier<Object>> attributeMakers();

    /** Where its attributes are inherited from ({@code inherit}): a type's base; none for a member. */
    protected MemberInfo inheritedFrom() {
        return null;
    }

    /** Its attributes, made anew; its base's too when {@code inherit}. */
    public Object[] GetCustomAttributes(boolean inherit) {
        List<Object> out = new ArrayList<>();
        for (MemberInfo m = this; m != null; m = inherit ? m.inheritedFrom() : null) {
            for (Supplier<Object> make : m.attributeMakers()) {
                out.add(make.get());
            }
        }
        return out.toArray();
    }

    /** Its attributes of {@code attributeType} (one inheriting it too). */
    public Object[] GetCustomAttributes(Type attributeType, boolean inherit) {
        List<Object> out = new ArrayList<>();
        for (Object a : GetCustomAttributes(inherit)) {
            if (attributeType.IsInstanceOfType(a)) {
                out.add(a);
            }
        }
        return out.toArray();
    }

    public boolean IsDefined(Type attributeType, boolean inherit) {
        return GetCustomAttributes(attributeType, inherit).length > 0;
    }

    /** Neither {@code private} nor {@code protected}: what {@code GetMethods()} and the rest give. */
    public boolean isPublic() {
        return (memberTraits() & (PRIVATE | PROTECTED)) == 0;
    }
}
