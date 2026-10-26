package dev.yog.script.collections.generic;

import java.util.Iterator;
import java.util.NoSuchElementException;
import java.util.function.Supplier;

/** Java's iterators as {@link IEnumerator}s and back. */
public final class Enumerators {
    private Enumerators() {}

    /** A walk over what {@code start} iterates, from its start again on {@code Reset}. */
    public static <T> IEnumerator<T> of(Supplier<Iterator<T>> start) {
        return new IEnumerator<>() {
            private Iterator<T> items = start.get();
            private T current;

            @Override
            public T Current() {
                return current;
            }

            @Override
            public boolean MoveNext() {
                if (!items.hasNext()) {
                    return false;
                }
                current = items.next();
                return true;
            }

            @Override
            public void Reset() {
                items = start.get();
                current = null;
            }
        };
    }

    /** What {@code items} iterates, gone through anew each time, as an {@link IEnumerable}. */
    public static <T> IEnumerable<T> enumerable(Supplier<Iterator<T>> items) {
        return new IEnumerable<>() {
            @Override
            public Iterator<T> iterator() {
                return items.get();
            }

            @Override
            public IEnumerator<T> GetEnumerator() {
                return of(items);
            }
        };
    }

    /** An enumerator walked as Java's iterator. */
    public static <T> Iterator<T> iterator(IEnumerator<T> e) {
        return new Iterator<>() {
            private boolean ahead;
            private boolean more;

            @Override
            public boolean hasNext() {
                if (!ahead) {
                    more = e.MoveNext();
                    ahead = true;
                }
                return more;
            }

            @Override
            public T next() {
                if (!hasNext()) {
                    throw new NoSuchElementException();
                }
                ahead = false;
                return e.Current();
            }
        };
    }
}
