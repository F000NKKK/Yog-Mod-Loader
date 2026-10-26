package dev.yog.script.reflection;

import dev.yog.script.Type;

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

    /** Neither {@code private} nor {@code protected}: what {@code GetMethods()} and the rest give. */
    public boolean isPublic() {
        return (memberTraits() & (PRIVATE | PROTECTED)) == 0;
    }
}
