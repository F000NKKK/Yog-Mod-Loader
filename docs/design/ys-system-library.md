# Yog Script's System library

.NET's base class library as Yog Script's own (`ys.md` §3, The System
library): the same namespaces, types, interfaces and their chain, members
by C#'s names — what a C# programmer reaches for is there, and acts the
same. Declared in Yog Script (`yog-script-analyze/src/system/*.ys`, one
file a namespace); what runs is the runtime's, in Java (`java-api`'s
`dev.yog.script`).

## One truth, no tables

- A type's declaration is the only place its members are written: the
  type checker reads it, the JVM lowering reads it. No list of names
  kept anywhere else (the tables `yog-script-java` had are gone).
- Where it runs is its place: `System.Collections.Generic.List<T>` is
  `dev.yog.script.system.collections.generic.List<T>` — the namespace
  lowered, the name kept. Members keep C#'s names in Java too
  (`list.Add(x)` is `list.Add(x)`): the runtime implements them, on top
  of `java.util`'s classes where they fit.
- A type the JVM has already and can't be given members — `string`
  (`java.lang.String`), the numbers, `bool`, `char` — is marked
  `[Host("java.lang.String")]`: its instance members lower to the
  runtime's static ones with the value first (`s.ToUpper()` is
  `system.String.ToUpper(s)`).
- `System`, `System.Collections.Generic`, `System.Linq`, `System.Text`
  and `System.Yog` are every file's `using`s; the rest is `using`'d.

## The chain

As .NET's (variance included: `IEnumerable<out T>`, `IComparer<in T>`).

```text
System
  Object                      Equals, GetHashCode, ToString
  String          [Host]      Length, this[i], Substring, IndexOf, Split, Replace, Trim…,
                              Join, Concat, Format, IsNullOrEmpty, Compare…  : IComparable<string>,
                              IEquatable<string>, IEnumerable<char>
  Char, Boolean, SByte…UInt64, Single, Double, Decimal   [Host]  Parse, TryParse (→ T?),
                              MinValue, MaxValue, CompareTo, IsDigit…   : IComparable<T>, IEquatable<T>
  Math, MathF                 Abs, Min, Max, Clamp, Sign, Floor, Ceiling, Round, Truncate, Sqrt,
                              Cbrt, Pow, Exp, Log, Log2, Log10, Sin, Cos, Tan, Asin, Acos, Atan,
                              Atan2, Sinh, Cosh, Tanh, Hypot(F), PI, E, Tau
  Random                      Next, NextDouble, NextSingle, Shuffle
  IEquatable<T>               Equals(T)
  IComparable<T>, IComparable CompareTo
  IDisposable                 Dispose (`using`)
  ICloneable                  Clone
  Nullable<T>                 (T?) HasValue, Value, GetValueOrDefault
  Action<…>, Func<…>, Predicate<T>, Comparison<T>, Converter<TIn, TOut>
  Tuple / ValueTuple          (a, b) Item1…, deconstruction
  Lazy<T>                     Value, IsValueCreated
  TimeSpan, DateTime          (with Yog's Ticks / Seconds units)
  Result<T, E>, Error, Panic  (Yog Script's own, `ys.md` §3 Failures)

System.Collections            IEnumerable, IEnumerator (the untyped forms)

System.Collections.Generic
  IEnumerable<out T>          GetEnumerator
  IEnumerator<out T>          Current, MoveNext, Reset            : IDisposable
  IReadOnlyCollection<out T>  Count                               : IEnumerable<T>
  IReadOnlyList<out T>        this[i]                             : IReadOnlyCollection<T>
  ICollection<T>              Count, IsReadOnly, Add, Clear, Contains, CopyTo, Remove
                                                                  : IEnumerable<T>
  IList<T>                    this[i], IndexOf, Insert, RemoveAt   : ICollection<T>
  ISet<T>, IReadOnlySet<T>    Add, UnionWith, IntersectWith, ExceptWith, SymmetricExceptWith,
                              IsSubsetOf, IsSupersetOf, Overlaps, SetEquals
  IDictionary<K, V>           this[k], Keys, Values, Add, ContainsKey, Remove, TryGetValue (→ V?)
                                                                  : ICollection<KeyValuePair<K, V>>
  IReadOnlyDictionary<K, V>                                       : IReadOnlyCollection<KeyValuePair<K, V>>
  KeyValuePair<K, V>          Key, Value, Deconstruct
  IEqualityComparer<in T>, EqualityComparer<T>.Default
  IComparer<in T>, Comparer<T>.Default, Comparer<T>.Create
  List<T>                     : IList<T>, IReadOnlyList<T>   AddRange, InsertRange, RemoveAll,
                              RemoveRange, Sort, Reverse, Find, FindAll, FindIndex, Exists,
                              TrueForAll, ForEach, BinarySearch, GetRange, ToArray, Capacity
  Dictionary<K, V>            : IDictionary<K, V>, IReadOnlyDictionary<K, V>   TryAdd, GetValueOrDefault
  HashSet<T>, SortedSet<T>    : ISet<T>, IReadOnlySet<T>   Min, Max (sorted)
  SortedDictionary<K, V>, SortedList<K, V>
  Queue<T>                    Enqueue, Dequeue, Peek, TryDequeue (→ T?)
  Stack<T>                    Push, Pop, Peek, TryPop (→ T?)
  PriorityQueue<E, P>         Enqueue, Dequeue, Peek, TryDequeue
  LinkedList<T>, LinkedListNode<T>

System.Linq
  Enumerable (on IEnumerable<T>, by `extend`):
    Where, Select, SelectMany, OfType, Cast, Distinct(By), Take(While), Skip(While),
    OrderBy(Descending), ThenBy(Descending), Reverse, GroupBy, Join, GroupJoin, Zip,
    Concat, Union(By), Intersect(By), Except(By), Append, Prepend, Chunk,
    First / Last / Single / ElementAt (OrDefault), Any, All, Contains, Count, LongCount,
    Sum, Average, Min(By), Max(By), Aggregate, SequenceEqual, DefaultIfEmpty,
    ToList, ToArray, ToDictionary, ToHashSet, ToLookup,
    Enumerable.Range, Repeat, Empty
  IOrderedEnumerable<T>, IGrouping<K, T>, ILookup<K, T>
  Query syntax (`from x in xs where … select …`) lowers to the same calls.

System.Text                   StringBuilder (Append, AppendLine, Insert, Remove, Clear, Length)
System.Reflection             Type, MethodInfo, PropertyInfo, FieldInfo (`ys.md` §3)
System.Linq.Expressions       Expression<Func<…>> (`ys.md` §3)
```

Exceptions aren't Yog Script's way: what fails gives a `Result` or `T?`
(`TryGetValue`, `TryParse` give the value or `null`, no `out`), what can't
go on panics (`list[99]` out of range, `Dequeue` on an empty queue) — a
panic named as C#'s exception would be (`ArgumentOutOfRange`).

## On the JVM

- The interfaces are Java interfaces of the same names, the chain kept
  (`IList<T> extends ICollection<T>`), each extending `java.util`'s where
  one fits (`IEnumerable<T> extends Iterable<T>`, `ICollection<T> extends
  java.util.Collection<T>`), C#'s members as default methods on Java's
  (`Add` is `add`): a `List<T>` is a `java.util.ArrayList` the rest of the
  JVM takes as one.
- `Dictionary<K, V>` is a `LinkedHashMap` that is also an
  `IEnumerable<KeyValuePair<K, V>>` (C#'s order: insertion).
- Yog's own lists (a payload's `List<string>`) are `java.util.List`s:
  given to Yog Script as a `List<T>` view, not copied.
- LINQ is lazy as C#'s: `Where` gives an `IEnumerable<T>` that runs when
  it's gone through, on Java's iterators (no streams' one-shot rule).

## Delegates and events

As .NET's: a delegate is a type declared (`delegate void Action<in T>(T
obj);`), `System`'s among them — `Action<…>`, `Func<…, out TResult>`,
`Predicate<in T>`, `Comparison<in T>`, `Converter<in TIn, out TOut>`,
`EventHandler<TEventArgs>` (with `EventArgs`) — each a file of its own,
none known to the checker by name. An `event` is a member of any
delegate type: `+=` / `-=` subscribe and leave, `Invoke` (or `?.Invoke`)
raises it. Yog's events (`Player.Join += e => …`) are `event`s of the
surface's delegates.

## Steps

0. The language under the library: indexers (done), `value` / `field`
   (done), the folders as namespaces (done); delegates declared and
   events of any delegate type; operators (`operator +`, `==`,
   `implicit` / `explicit` conversions).
1. `System.Collections.Generic`'s interfaces and `List`, `Dictionary`,
   `HashSet`, `KeyValuePair`; `System`'s `IEquatable`, `IComparable`,
   `IDisposable`, `Math`, `String`'s members; the type checker reading
   members off the declarations; the lowering by place. The tables go.
2. `System.Linq` (extensions on `IEnumerable<T>`), `Queue`, `Stack`,
   `SortedSet`, `SortedDictionary`, `PriorityQueue`, `LinkedList`,
   comparers.
3. The numbers' and `char`'s members, `Random`, `StringBuilder`,
   `Lazy`, `TimeSpan`, `DateTime`; query syntax.
4. `System.Reflection`, `System.Linq.Expressions`.
