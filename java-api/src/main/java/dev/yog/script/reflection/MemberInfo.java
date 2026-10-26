package dev.yog.script.reflection;

/** A type's member as Reflection sees it: Yog Script's {@code System.Reflection.MemberInfo}. */
public abstract class MemberInfo {
    public abstract String Name();

    public abstract dev.yog.script.Type DeclaringType();

    public abstract MemberTypes MemberType();
}
