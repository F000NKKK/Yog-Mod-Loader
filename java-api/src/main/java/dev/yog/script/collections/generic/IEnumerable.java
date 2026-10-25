package dev.yog.script.collections.generic;

/**
 * What can be gone through, an item at a time: Yog Script's
 * {@code IEnumerable<T>}, also Java's {@link Iterable} so {@code foreach}
 * and the rest of the JVM take it.
 */
public interface IEnumerable<T> extends Iterable<T> {
    IEnumerator<T> GetEnumerator();

    @Override
    default java.util.Iterator<T> iterator() {
        return Enumerators.iterator(GetEnumerator());
    }
}
