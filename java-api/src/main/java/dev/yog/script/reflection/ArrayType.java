package dev.yog.script.reflection;

import dev.yog.script.Type;
import dev.yog.script.reflection.library.Library;
import java.util.List;

/** An array of a type, of its rank ({@code Int32[]}, {@code Int32[,]}): a {@code System.Array}. */
public final class ArrayType extends Type {
    private static final String OPEN = "[";
    private static final String CLOSE = "]";
    private static final String RANK_SEP = ",";

    private final Type element;
    private final int rank;

    public ArrayType(Type element, int rank) {
        this.element = element;
        this.rank = rank;
    }

    @Override
    protected int traits() {
        return CLASS | SEALED;
    }

    private String ranks() {
        return OPEN + RANK_SEP.repeat(rank - 1) + CLOSE;
    }

    @Override
    public String Name() {
        return element.Name() + ranks();
    }

    @Override
    public String Namespace() {
        return element.Namespace();
    }

    @Override
    public String FullName() {
        String of = element.FullName();
        return of == null ? null : of + ranks();
    }

    @Override
    public Type BaseType() {
        return Library.array();
    }

    /** A one-dimensional one's: {@code IList<T>} and the rest, as C#'s. */
    @Override
    protected List<Type> ownInterfaces() {
        return rank == 1 ? List.of(Library.arrayInterfaces(element)) : List.of();
    }

    @Override
    public boolean IsArray() {
        return true;
    }

    @Override
    public Type GetElementType() {
        return element;
    }

    @Override
    public int GetArrayRank() {
        return rank;
    }

    @Override
    public Type substitute(Type[] arguments) {
        return element.substitute(arguments).MakeArrayType(rank);
    }

    @Override
    public String toString() {
        return element + ranks();
    }
}
