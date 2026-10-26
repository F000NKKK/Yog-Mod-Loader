package dev.yog.script;

/**
 * An object of a generic type the compiler made, keeping the type
 * arguments it was made with ({@code new Box<i32>(…)}): what
 * {@code GetType()} gives with them ({@code Box`1[System.Int32]}), as .NET's
 * objects keep theirs. {@code null} where generic code made it, not knowing them.
 */
public interface Typed {
    Type[] $types();

    void $types(Type[] arguments);
}
