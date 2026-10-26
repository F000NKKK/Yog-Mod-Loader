package dev.yog.script.reflection;

import dev.yog.script.Attribute;
import dev.yog.script.Type;
import dev.yog.script.collections.generic.List;

/** A member's attributes by their type: Yog Script's {@code System.Reflection.CustomAttributeExtensions}. */
public final class CustomAttributeExtensions {
    private CustomAttributeExtensions() {}

    /** {@code element}'s attribute of {@code type} (its class handed first, {@code [Reified]}); {@code null} for none, a panic for several. */
    public static <T> T GetCustomAttribute(Class<T> type, MemberInfo element) {
        Object found = null;
        for (Object a : element.GetCustomAttributes(true)) {
            if (type.isInstance(a)) {
                if (found != null) {
                    throw new IllegalStateException("`" + element.Name() + "` has more than one `" + type.getSimpleName() + "`");
                }
                found = a;
            }
        }
        return type.cast(found);
    }

    /** {@code element}'s attributes of {@code type}. */
    public static <T> List<T> GetCustomAttributes(Class<T> type, MemberInfo element) {
        List<T> out = new List<>();
        for (Object a : element.GetCustomAttributes(true)) {
            if (type.isInstance(a)) {
                out.Add(type.cast(a));
            }
        }
        return out;
    }

    public static Attribute GetCustomAttribute(MemberInfo element, Type attributeType) {
        Object[] found = element.GetCustomAttributes(attributeType, true);
        if (found.length > 1) {
            throw new IllegalStateException("`" + element.Name() + "` has more than one `" + attributeType.Name() + "`");
        }
        return found.length == 0 ? null : (Attribute) found[0];
    }
}
