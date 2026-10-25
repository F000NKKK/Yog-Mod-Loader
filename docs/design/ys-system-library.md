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
- Where it runs is its place: `System` is the runtime's package
  (`dev.yog.script`), each namespace under it a package of its
  lowercased name (`System.Collections.Generic.List<T>` is
  `dev.yog.script.collections.generic.List<T>`, a Java word escaped:
  `System.Yog.Native` is `…yog.native_`). Members keep C#'s names in
  Java too (`list.Add(x)` is `list.Add(x)`).
- A name several arities share (`Action`, `Action<T>`, …) is told apart
  as the CLR does, by its arity: `Action2` for `Action`2`, the one of none
  keeping its name; a source's own type hides the library's of its name.
- The library is compiled by its own compiler: every type of it the
  compiler writes whole (delegates, classes, enums, attributes) is
  generated into the runtime (`yog-script-library`, kept current by a
  test); a type the runtime implements by hand is marked `[Intrinsic]`
  (.NET's), its declaration what code sees.
- A built-in type is its type in `System`, as C#'s (`string` is
  `String`, `i32` `Int32`: the table of built-ins names both). One the JVM
  has already and can't be given members — `string`, the numbers, `bool`,
  `char` — has its members declared on that type, marked `[Host]`: they
  lower to a companion of the runtime's, a static each with the value
  first (`s.ToUpper()` is `dev.yog.script.host.String.ToUpper(s)`; apart
  in `host`, so it doesn't hide Java's `String` in the runtime's package).
  `Split` gives a `List<string>` (Yog Script has no arrays of its own).
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
  TimeSpan                    Ticks, Days…, TotalSeconds…, FromSeconds…, Add, Negate, Duration,
                              + - * / == < … (operators)            : IEquatable, IComparable
  DateTime                    (with Yog's Ticks / Seconds units)
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

System.Numerics
  Vector2, Vector3            X Y Z, Zero, One, UnitX…, Length, Dot, Cross, Distance, Normalize,
                              Lerp, Min, Max, + - * / == (operators)   : IEquatable

System.Text                   StringBuilder (Append, AppendLine, Insert, Remove, Clear, Length)
System.Reflection             Type, MethodInfo, PropertyInfo, FieldInfo (`ys.md` §3)
System.Linq.Expressions       Expression<Func<…>> (`ys.md` §3)
```

Exceptions aren't Yog Script's way: what fails gives a `Result` or `T?`
(`TryGetValue`, `TryParse` give the value or `null`, no `out`), what can't
go on panics (`list[99]` out of range, `Dequeue` on an empty queue) — a
panic named as C#'s exception would be (`ArgumentOutOfRange`).

## On the JVM

- The collections are the runtime's, by hand (`[Intrinsic]`), in
  `dev.yog.script.collections.generic`: the interfaces Java interfaces of
  the same names and chain (`IList<T> extends ICollection<T>`),
  `IEnumerable<T>` also Java's `Iterable<T>` (so `foreach` and the rest of
  the JVM go through any of them); a property of an interface or of a
  runtime type is its getter there (`xs.Count` is `xs.Count()`), an
  indexer `get_Item` / `set_Item`.
- `List<T>`, `Dictionary<K, V>`, `HashSet<T>` are Java's `AbstractList`,
  `AbstractMap`, `AbstractSet` over what they hold — their own, or a Java
  list or map they're a view of: Yog's values' lists and maps are given to
  Yog Script as `List.view(…)` / `Dictionary.view(…)`, not copied, and a
  Yog Script list handed to Yog is a `java.util.List` as it is.
- `Dictionary` keeps C#'s order: insertion. `HashSet` too (C#'s is
  unordered; a fixed order keeps a game's runs the same).
- A collection expression is made as C#'s collection initializer does:
  `new List<T>()`, then `Add` each item (`Add(k, v)` each entry, every item
  of a spread); an interface wanted (`IEnumerable<T> xs = […]`) makes a
  `List<T>` (a `Dictionary` for entries).
- LINQ will be lazy as C#'s: `Where` gives an `IEnumerable<T>` that runs
  when it's gone through, on Java's iterators (no streams' one-shot rule).

Where Java's erasure makes .NET's shape impossible, the shape gives way:
`IDictionary<K, V>` is an `IReadOnlyCollection<KeyValuePair<K, V>>`, not
an `ICollection` of them (its `Remove(K)` and `ICollection`'s
`Remove(KeyValuePair)` erase alike), and a set's `Add` gives nothing
(`ICollection`'s `void Add` and `ISet`'s `bool Add` can't both be). Not
there yet: variance (`IEnumerable<out T>` is written, not yet applied),
`equals` / `hashCode` from a type's `Equals` / `GetHashCode` (a struct is
found in a set by identity until then), `IReadOnlyDictionary`,
`IReadOnlySet`.

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

0. The language under the library (done): indexers, `value` / `field`,
   the folders as namespaces, delegates declared and events of any
   delegate type, operators (`operator +`, `==`, `implicit` / `explicit`
   conversions), methods overloaded by their parameters. A library
   type's members are read in its own namespace wherever they're reached
   from; a type name nothing in reach declares is YS0035.
1. `System.Collections.Generic`'s interfaces and `List`, `Dictionary`,
   `HashSet`, `KeyValuePair` (done: the checker reads their members off
   the declarations, `foreach` goes through any `IEnumerable<T>`, the
   built-in list and map types and the JVM tables for them are gone);
   `System`'s `IEquatable`, `IComparable`, `IDisposable`, `Math` (the
   runtime's, .NET's set) and `String`'s members (`[Host]`) — done: no
   table of the library's names is left in `yog-script-java`.
2. `System.Linq` (extensions on `IEnumerable<T>`), `Queue`, `Stack`,
   `SortedSet`, `SortedDictionary`, `PriorityQueue`, `LinkedList`,
   comparers.
3. The numbers' and `char`'s members, `Random`, `StringBuilder`,
   `Lazy`, `TimeSpan` (done), `DateTime`, `System.Numerics`' vectors
   (done; matrices, quaternions next); query syntax.
4. `System.Reflection`, `System.Linq.Expressions`.
