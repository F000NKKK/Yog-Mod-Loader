package dev.yog.script.linq;

import static dev.yog.script.linq.Sequence.stream;

import dev.yog.script.Func2;
import dev.yog.script.Func3;
import dev.yog.script.collections.generic.Dictionary;
import dev.yog.script.collections.generic.HashSet;
import dev.yog.script.collections.generic.IEnumerable;
import dev.yog.script.collections.generic.List;
import dev.yog.script.collections.generic.Order;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.stream.IntStream;
import java.util.stream.Stream;

/**
 * Yog Script's {@code System.Linq.Enumerable}: .NET's LINQ, lazy as C#'s —
 * what gives items runs when they're gone through, every time. Overloads
 * Java's erasure can't tell apart are numbered as the compiler names them
 * ({@code Sum}, {@code Sum$1}, … in their declared order).
 */
public final class Enumerable {
    private static final String NONE = "the sequence has no items";
    private static final String MANY = "the sequence has more than one item";

    private Enumerable() {}

    // Filtered, projected.

    public static <T> IEnumerable<T> Where(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return Sequence.of(() -> stream(source).filter(predicate::Invoke));
    }

    public static <T> IEnumerable<T> Where(IEnumerable<T> source, Func3<T, Integer, Boolean> predicate) {
        return Sequence.of(() -> {
            int[] at = {0};
            return stream(source).filter(x -> predicate.Invoke(x, at[0]++));
        });
    }

    public static <T, R> IEnumerable<R> Select(IEnumerable<T> source, Func2<T, R> selector) {
        return Sequence.of(() -> stream(source).map(selector::Invoke));
    }

    public static <T, R> IEnumerable<R> Select(IEnumerable<T> source, Func3<T, Integer, R> selector) {
        return Sequence.of(() -> {
            int[] at = {0};
            return stream(source).map(x -> selector.Invoke(x, at[0]++));
        });
    }

    public static <T, R> IEnumerable<R> SelectMany(IEnumerable<T> source, Func2<T, IEnumerable<R>> selector) {
        return Sequence.of(() -> stream(source).flatMap(x -> stream(selector.Invoke(x))));
    }

    /** Its items that are {@code type}'s ({@code [Reified]}: the class handed first). */
    @SuppressWarnings("unchecked")
    public static <R> IEnumerable<R> OfType(Class<?> type, IEnumerable<Object> source) {
        Class<?> boxed = boxed(type);
        return Sequence.of(() -> stream(source).filter(boxed::isInstance).map(x -> (R) x));
    }

    /** Each item as a {@code type}: a panic at one that isn't. */
    @SuppressWarnings("unchecked")
    public static <R> IEnumerable<R> Cast(Class<?> type, IEnumerable<Object> source) {
        Class<?> boxed = boxed(type);
        return Sequence.of(() -> stream(source).map(x -> (R) boxed.cast(x)));
    }

    // Taken, skipped.

    public static <T> IEnumerable<T> Take(IEnumerable<T> source, int count) {
        return Sequence.of(() -> stream(source).limit(Math.max(0, count)));
    }

    public static <T> IEnumerable<T> Skip(IEnumerable<T> source, int count) {
        return Sequence.of(() -> stream(source).skip(Math.max(0, count)));
    }

    public static <T> IEnumerable<T> TakeLast(IEnumerable<T> source, int count) {
        return Sequence.of(() -> {
            java.util.List<T> all = stream(source).toList();
            return all.subList(Math.max(0, all.size() - Math.max(0, count)), all.size()).stream();
        });
    }

    public static <T> IEnumerable<T> SkipLast(IEnumerable<T> source, int count) {
        return Sequence.of(() -> {
            java.util.List<T> all = stream(source).toList();
            return all.subList(0, Math.max(0, all.size() - Math.max(0, count))).stream();
        });
    }

    public static <T> IEnumerable<T> TakeWhile(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return Sequence.of(() -> stream(source).takeWhile(predicate::Invoke));
    }

    public static <T> IEnumerable<T> SkipWhile(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return Sequence.of(() -> stream(source).dropWhile(predicate::Invoke));
    }

    // Ordered.

    public static <T> IOrderedEnumerable<T> Order(IEnumerable<T> source) {
        return new OrderedSequence<>(source, Order.natural());
    }

    public static <T> IOrderedEnumerable<T> OrderDescending(IEnumerable<T> source) {
        return new OrderedSequence<>(source, Order.<T>natural().reversed());
    }

    public static <T, K> IOrderedEnumerable<T> OrderBy(IEnumerable<T> source, Func2<T, K> keySelector) {
        return new OrderedSequence<>(source, by(keySelector));
    }

    public static <T, K> IOrderedEnumerable<T> OrderByDescending(IEnumerable<T> source, Func2<T, K> keySelector) {
        return new OrderedSequence<>(source, by(keySelector).reversed());
    }

    public static <T, K> IOrderedEnumerable<T> ThenBy(IOrderedEnumerable<T> source, Func2<T, K> keySelector) {
        return new OrderedSequence<>(source.source(), source.order().thenComparing(by(keySelector)));
    }

    public static <T, K> IOrderedEnumerable<T> ThenByDescending(IOrderedEnumerable<T> source, Func2<T, K> keySelector) {
        return new OrderedSequence<>(source.source(), source.order().thenComparing(by(keySelector).reversed()));
    }

    public static <T> IEnumerable<T> Reverse(IEnumerable<T> source) {
        return Sequence.of(() -> {
            java.util.List<T> all = new ArrayList<>(stream(source).toList());
            java.util.Collections.reverse(all);
            return all.stream();
        });
    }

    // Grouped, joined.

    public static <T, K> IEnumerable<IGrouping<K, T>> GroupBy(IEnumerable<T> source, Func2<T, K> keySelector) {
        return GroupBy(source, keySelector, x -> x);
    }

    public static <T, K, E> IEnumerable<IGrouping<K, E>> GroupBy(
            IEnumerable<T> source, Func2<T, K> keySelector, Func2<T, E> elementSelector) {
        return Sequence.of(() -> {
            Lookup<K, E> groups = new Lookup<>();
            stream(source).forEach(x -> groups.add(keySelector.Invoke(x), elementSelector.Invoke(x)));
            return stream(groups);
        });
    }

    public static <O, I, K, R> IEnumerable<R> Join(
            IEnumerable<O> outer,
            IEnumerable<I> inner,
            Func2<O, K> outerKeySelector,
            Func2<I, K> innerKeySelector,
            Func3<O, I, R> resultSelector) {
        return Sequence.of(() -> {
            Lookup<K, I> inners = lookup(inner, innerKeySelector);
            return stream(outer).flatMap(o -> stream(inners.get_Item(outerKeySelector.Invoke(o)))
                    .map(i -> resultSelector.Invoke(o, i)));
        });
    }

    public static <O, I, K, R> IEnumerable<R> GroupJoin(
            IEnumerable<O> outer,
            IEnumerable<I> inner,
            Func2<O, K> outerKeySelector,
            Func2<I, K> innerKeySelector,
            Func3<O, IEnumerable<I>, R> resultSelector) {
        return Sequence.of(() -> {
            Lookup<K, I> inners = lookup(inner, innerKeySelector);
            return stream(outer).map(o -> resultSelector.Invoke(o, inners.get_Item(outerKeySelector.Invoke(o))));
        });
    }

    public static <A, B, R> IEnumerable<R> Zip(IEnumerable<A> first, IEnumerable<B> second, Func3<A, B, R> resultSelector) {
        return Sequence.of(() -> {
            Iterator<A> a = first.iterator();
            Iterator<B> b = second.iterator();
            Stream.Builder<R> out = Stream.builder();
            while (a.hasNext() && b.hasNext()) {
                out.add(resultSelector.Invoke(a.next(), b.next()));
            }
            return out.build();
        });
    }

    // Combined as sets and sequences.

    public static <T> IEnumerable<T> Concat(IEnumerable<T> first, IEnumerable<T> second) {
        return Sequence.of(() -> Stream.concat(stream(first), stream(second)));
    }

    public static <T> IEnumerable<T> Union(IEnumerable<T> first, IEnumerable<T> second) {
        return Sequence.of(() -> Stream.concat(stream(first), stream(second)).distinct());
    }

    public static <T> IEnumerable<T> Intersect(IEnumerable<T> first, IEnumerable<T> second) {
        return Sequence.of(() -> {
            Set<T> other = stream(second).collect(Collectors.toSet());
            return stream(first).distinct().filter(other::contains);
        });
    }

    public static <T> IEnumerable<T> Except(IEnumerable<T> first, IEnumerable<T> second) {
        return Sequence.of(() -> {
            Set<T> other = stream(second).collect(Collectors.toSet());
            return stream(first).distinct().filter(x -> !other.contains(x));
        });
    }

    public static <T, K> IEnumerable<T> UnionBy(IEnumerable<T> first, IEnumerable<T> second, Func2<T, K> keySelector) {
        return DistinctBy(Concat(first, second), keySelector);
    }

    public static <T, K> IEnumerable<T> IntersectBy(IEnumerable<T> first, IEnumerable<K> second, Func2<T, K> keySelector) {
        return Sequence.of(() -> {
            Set<K> keys = stream(second).collect(Collectors.toSet());
            Set<K> seen = new LinkedHashSet<>();
            return stream(first).filter(x -> {
                K key = keySelector.Invoke(x);
                return keys.contains(key) && seen.add(key);
            });
        });
    }

    public static <T, K> IEnumerable<T> ExceptBy(IEnumerable<T> first, IEnumerable<K> second, Func2<T, K> keySelector) {
        return Sequence.of(() -> {
            Set<K> seen = stream(second).collect(Collectors.toCollection(LinkedHashSet::new));
            return stream(first).filter(x -> seen.add(keySelector.Invoke(x)));
        });
    }

    public static <T> IEnumerable<T> Distinct(IEnumerable<T> source) {
        return Sequence.of(() -> stream(source).distinct());
    }

    public static <T, K> IEnumerable<T> DistinctBy(IEnumerable<T> source, Func2<T, K> keySelector) {
        return Sequence.of(() -> {
            Set<K> seen = new LinkedHashSet<>();
            return stream(source).filter(x -> seen.add(keySelector.Invoke(x)));
        });
    }

    public static <T> IEnumerable<T> Append(IEnumerable<T> source, T element) {
        return Sequence.of(() -> Stream.concat(stream(source), Stream.of(element)));
    }

    public static <T> IEnumerable<T> Prepend(IEnumerable<T> source, T element) {
        return Sequence.of(() -> Stream.concat(Stream.of(element), stream(source)));
    }

    /**
     * Its items in arrays of {@code size} (the last may be shorter), each a
     * Java array of {@code item} ({@code int[]} for {@code int.class}).
     */
    public static <T> IEnumerable<Object> Chunk(Class<?> item, IEnumerable<T> source, int size) {
        if (size < 1) {
            throw new IllegalArgumentException("chunks of " + size + " items");
        }
        return Sequence.of(() -> {
            java.util.List<T> all = stream(source).toList();
            return IntStream.range(0, (all.size() + size - 1) / size).mapToObj(c -> {
                java.util.List<T> part = all.subList(c * size, Math.min(all.size(), (c + 1) * size));
                Object chunk = java.lang.reflect.Array.newInstance(item, part.size());
                for (int i = 0; i < part.size(); i++) {
                    java.lang.reflect.Array.set(chunk, i, part.get(i));
                }
                return chunk;
            });
        });
    }

    public static <T> IEnumerable<T> DefaultIfEmpty(IEnumerable<T> source, T defaultValue) {
        return Sequence.of(() -> source.iterator().hasNext() ? stream(source) : Stream.of(defaultValue));
    }

    // One item.

    public static <T> T First(IEnumerable<T> source) {
        return first(stream(source), true);
    }

    public static <T> T First(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return first(stream(source).filter(predicate::Invoke), true);
    }

    public static <T> T FirstOrDefault(IEnumerable<T> source) {
        return first(stream(source), false);
    }

    public static <T> T FirstOrDefault(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return first(stream(source).filter(predicate::Invoke), false);
    }

    public static <T> T Last(IEnumerable<T> source) {
        return last(stream(source), true);
    }

    public static <T> T Last(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return last(stream(source).filter(predicate::Invoke), true);
    }

    public static <T> T LastOrDefault(IEnumerable<T> source) {
        return last(stream(source), false);
    }

    public static <T> T LastOrDefault(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return last(stream(source).filter(predicate::Invoke), false);
    }

    public static <T> T Single(IEnumerable<T> source) {
        return single(stream(source), true);
    }

    public static <T> T Single(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return single(stream(source).filter(predicate::Invoke), true);
    }

    public static <T> T SingleOrDefault(IEnumerable<T> source) {
        return single(stream(source), false);
    }

    public static <T> T SingleOrDefault(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return single(stream(source).filter(predicate::Invoke), false);
    }

    public static <T> T ElementAt(IEnumerable<T> source, int index) {
        return first(stream(source).skip(Math.max(0, index)), index >= 0);
    }

    public static <T> T ElementAtOrDefault(IEnumerable<T> source, int index) {
        return index < 0 ? null : first(stream(source).skip(index), false);
    }

    // Asked of all.

    public static <T> boolean Any(IEnumerable<T> source) {
        return source.iterator().hasNext();
    }

    public static <T> boolean Any(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return stream(source).anyMatch(predicate::Invoke);
    }

    public static <T> boolean All(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return stream(source).allMatch(predicate::Invoke);
    }

    public static <T> boolean Contains(IEnumerable<T> source, T value) {
        return stream(source).anyMatch(x -> java.util.Objects.equals(x, value));
    }

    public static <T> int Count(IEnumerable<T> source) {
        return Math.toIntExact(LongCount(source));
    }

    public static <T> int Count(IEnumerable<T> source, Func2<T, Boolean> predicate) {
        return Math.toIntExact(stream(source).filter(predicate::Invoke).count());
    }

    public static <T> long LongCount(IEnumerable<T> source) {
        return stream(source).count();
    }

    public static <T> boolean SequenceEqual(IEnumerable<T> first, IEnumerable<T> second) {
        return stream(first).toList().equals(stream(second).toList());
    }

    // Summed, averaged: one of each width, numbered as the compiler names them.

    public static int Sum(IEnumerable<Integer> source) {
        return stream(source).reduce(0, Math::addExact);
    }

    public static long Sum$1(IEnumerable<Long> source) {
        return stream(source).reduce(0L, Math::addExact);
    }

    public static float Sum$2(IEnumerable<Float> source) {
        return stream(source).reduce(0f, Float::sum);
    }

    public static double Sum$3(IEnumerable<Double> source) {
        return stream(source).reduce(0d, Double::sum);
    }

    public static <T> int Sum(IEnumerable<T> source, Func2<T, Integer> selector) {
        return Sum(Select(source, selector));
    }

    public static <T> long Sum$1(IEnumerable<T> source, Func2<T, Long> selector) {
        return Sum$1(Select(source, selector));
    }

    public static <T> float Sum$2(IEnumerable<T> source, Func2<T, Float> selector) {
        return Sum$2(Select(source, selector));
    }

    public static <T> double Sum$3(IEnumerable<T> source, Func2<T, Double> selector) {
        return Sum$3(Select(source, selector));
    }

    public static double Average(IEnumerable<Integer> source) {
        return stream(source).mapToDouble(Number::doubleValue).average()
                .orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static double Average$1(IEnumerable<Long> source) {
        return stream(source).mapToDouble(Number::doubleValue).average()
                .orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static float Average$2(IEnumerable<Float> source) {
        return (float) stream(source).mapToDouble(Number::doubleValue).average()
                .orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static double Average$3(IEnumerable<Double> source) {
        return stream(source).mapToDouble(Number::doubleValue).average()
                .orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static <T> double Average(IEnumerable<T> source, Func2<T, Integer> selector) {
        return Average(Select(source, selector));
    }

    public static <T> double Average$1(IEnumerable<T> source, Func2<T, Long> selector) {
        return Average$1(Select(source, selector));
    }

    public static <T> float Average$2(IEnumerable<T> source, Func2<T, Float> selector) {
        return Average$2(Select(source, selector));
    }

    public static <T> double Average$3(IEnumerable<T> source, Func2<T, Double> selector) {
        return Average$3(Select(source, selector));
    }

    public static <T> T Min(IEnumerable<T> source) {
        return stream(source).min(Order.natural()).orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static <T> T Max(IEnumerable<T> source) {
        return stream(source).max(Order.natural()).orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static <T, R> R Min(IEnumerable<T> source, Func2<T, R> selector) {
        return Min(Select(source, selector));
    }

    public static <T, R> R Max(IEnumerable<T> source, Func2<T, R> selector) {
        return Max(Select(source, selector));
    }

    public static <T, K> T MinBy(IEnumerable<T> source, Func2<T, K> keySelector) {
        return stream(source).min(by(keySelector)).orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static <T, K> T MaxBy(IEnumerable<T> source, Func2<T, K> keySelector) {
        return stream(source).max(by(keySelector)).orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static <T> T Aggregate(IEnumerable<T> source, Func3<T, T, T> func) {
        return stream(source).reduce(func::Invoke).orElseThrow(() -> new IllegalStateException(NONE));
    }

    public static <T, A> A Aggregate(IEnumerable<T> source, A seed, Func3<A, T, A> func) {
        A acc = seed;
        for (T item : source) {
            acc = func.Invoke(acc, item);
        }
        return acc;
    }

    // Made into collections.

    public static <T> List<T> ToList(IEnumerable<T> source) {
        return new List<>(source);
    }

    /** Its items in an array of objects: made the array wanted where it's handed on ({@code Array.Of}). */
    public static <T> Object[] ToArray(IEnumerable<T> source) {
        return stream(source).toArray();
    }

    public static <T> HashSet<T> ToHashSet(IEnumerable<T> source) {
        return new HashSet<>(source);
    }

    public static <T, K> Dictionary<K, T> ToDictionary(IEnumerable<T> source, Func2<T, K> keySelector) {
        return ToDictionary(source, keySelector, x -> x);
    }

    public static <T, K, E> Dictionary<K, E> ToDictionary(
            IEnumerable<T> source, Func2<T, K> keySelector, Func2<T, E> elementSelector) {
        Dictionary<K, E> out = new Dictionary<>();
        for (T item : source) {
            out.Add(keySelector.Invoke(item), elementSelector.Invoke(item));
        }
        return out;
    }

    public static <T, K> ILookup<K, T> ToLookup(IEnumerable<T> source, Func2<T, K> keySelector) {
        return lookup(source, keySelector);
    }

    // Made from nothing.

    public static IEnumerable<Integer> Range(int start, int count) {
        if (count < 0) {
            throw new IllegalArgumentException("a range of " + count + " items");
        }
        return Sequence.of(() -> IntStream.range(0, count).mapToObj(i -> start + i));
    }

    public static <R> IEnumerable<R> Repeat(R element, int count) {
        return Sequence.of(() -> Stream.generate(() -> element).limit(Math.max(0, count)));
    }

    public static <R> IEnumerable<R> Empty() {
        return Sequence.of(Stream::empty);
    }

    // What the rest is made of.

    /** {@code type}, a primitive's as its box ({@code int} as {@code Integer}): what its values are as objects. */
    private static Class<?> boxed(Class<?> type) {
        return java.lang.invoke.MethodType.methodType(type).wrap().returnType();
    }

    /** Items ordered by their keys, in the keys' own order. */
    private static <T, K> Comparator<T> by(Func2<T, K> keySelector) {
        return Comparator.comparing(keySelector::Invoke, Order.natural());
    }

    private static <T, K> Lookup<K, T> lookup(IEnumerable<T> source, Func2<T, K> keySelector) {
        Lookup<K, T> out = new Lookup<>();
        stream(source).forEach(x -> out.add(keySelector.Invoke(x), x));
        return out;
    }

    /** The first of {@code items}: a panic, or {@code null}, when there's none ({@code must}). */
    private static <T> T first(Stream<T> items, boolean must) {
        Iterator<T> it = items.iterator();
        if (it.hasNext()) {
            return it.next();
        }
        return none(must);
    }

    private static <T> T last(Stream<T> items, boolean must) {
        T last = null;
        boolean any = false;
        for (Iterator<T> it = items.iterator(); it.hasNext(); ) {
            last = it.next();
            any = true;
        }
        return any ? last : none(must);
    }

    private static <T> T single(Stream<T> items, boolean must) {
        Iterator<T> it = items.iterator();
        if (!it.hasNext()) {
            return none(must);
        }
        T one = it.next();
        if (it.hasNext()) {
            throw new IllegalStateException(MANY);
        }
        return one;
    }

    private static <T> T none(boolean must) {
        if (must) {
            throw new IllegalStateException(NONE);
        }
        return null;
    }
}
