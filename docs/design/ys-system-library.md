# Yog Script's System library

.NET's base class library as Yog Script's own (`ys.md` §3, The System
library): the same namespaces, types, interfaces and their chain, members
by C#'s names — what a C# programmer reaches for is there, and acts the
same. Declared in Yog Script (`yog-script-analyze/src/system/*.ys`, one
file a namespace); what runs is the runtime's, in Java (`java-api`'s
`dev.yog.script`).

## One truth, no tables

- Every name the tools know Yog Script by — the library's types,
  attributes and namespaces, the CLR's member names, units, where the
  runtime is on the JVM, the members a tool runs by hand — is
  `yog-script-ids`'s, written once there.
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
  ValueTuple<T1…T7>           (a, b), (X: 1, Y: 2), names inferred, Item1…, `let (a, b) = t` (done)
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
                              Lerp, Min, Max, Transform, TransformNormal, + - * / ==   : IEquatable
  Quaternion                  Identity, CreateFromAxisAngle, CreateFromYawPitchRoll, Dot,
                              Conjugate, Inverse, Normalize, Slerp, * -
  Matrix4x4, Matrix3x2        Identity, Translation, CreateTranslation/Scale/Rotation*,
                              CreateFromQuaternion, CreateLookAt, CreatePerspectiveFieldOfView,
                              Transpose, Invert, GetDeterminant, * + - ==   (row vectors, as .NET)

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
- LINQ is lazy as C#'s: `Where` gives an `IEnumerable<T>` that runs when
  it's gone through, every time anew (a Java stream made per pass: no
  one-shot rule). Overloads Java's erasure can't tell apart in one type
  are numbered (`Sum`, `Sum$1`, … in their declared order).

Where Java can't hold .NET's shape as it is, the names give way, never
the shape: a method Java's erasure can't tell from one it inherits, or
one hiding it (`ISet<T>`'s `new bool Add(T)` over `ICollection<T>`'s
`void Add(T)`, `IDictionary<K, V>`'s `Remove(K)` over
`ICollection<KeyValuePair<K, V>>`'s `Remove(T)`), is named after its type
in Java (`ISet$Add`, `IDictionary$Remove`); what implements it takes that
name, explicit implementations theirs. `IReadOnlyDictionary<K, V>` and
`IReadOnlySet<T>` as .NET's (`Dictionary`'s `Keys` an `ICollection<K>`,
its read-only one an `IEnumerable<K>` by explicit implementation); what
only reads a collection is `readonly`, so it's called through `in`.

## Delegates and events

As .NET's: a delegate is a type declared (`delegate void Action<in T>(T
obj);`), `System`'s among them — `Action<…>`, `Func<…, out TResult>`,
`Predicate<in T>`, `Comparison<in T>`, `Converter<in TIn, out TOut>`,
`EventHandler<TEventArgs>` (with `EventArgs`) — each a file of its own,
none known to the checker by name. An `event` is a member of any
delegate type: `+=` / `-=` subscribe and leave, `Invoke` (or `?.Invoke`)
raises it. Yog's events (`Player.Join += e => …`) are `event`s of the
surface's delegates.

## Reflection and expression trees

As .NET's: `System.Type` (`typeof(T)`, `x.GetType()`), and in
`System.Reflection` `MemberInfo`, `FieldInfo`, `PropertyInfo`,
`MethodInfo`, `ConstructorInfo`, `ParameterInfo`, `BindingFlags`, the
attributes read by `GetCustomAttributes<T>()`; in
`System.Linq.Expressions` `Expression<TDelegate>` and its nodes.

**What a type is comes from its declaration, never from Java.** Java's
classes know Java's names (`Integer`, `ValueTuple2`, `ISet$Add`), not Yog
Script's (`Int32`, `ValueTuple<T1, T2>`, `Add`), and erase generics. So
the compiler writes, beside every type it compiles, its description: a
`$TYPE` (`dev.yog.script.Type`) of its name, namespace, kind, type
parameters, base and interfaces, and — built when first asked for — its
members, each in Yog Script's terms (its name, its types as `Type`s, its
attributes as the objects they are), each with what reads, writes or
calls it (a lambda written next to the member, so `private` ones are
reached as C#'s `BindingFlags.NonPublic` reaches them). The library's
types get theirs the same way; the runtime's hand-written ones
(`[Intrinsic]`, `[Host]`) from a generated companion (`*Type` in
`dev.yog.script.reflection.library`), made from their `.ys` declarations
— one truth, no tables.

At run time `Type.Of(x)` finds a value's `Type` by its class (its
`$TYPE`, or the library's companion: `Integer` is `Int32`). A Java class
Yog Script doesn't declare is seen by its Java name, without members.

On the JVM an object doesn't keep its generic arguments, so every generic
class made by `new` keeps them (`Typed`) — the compiler's and the
runtime's collections alike: `new Box<i32>(…)`, `new List<string>()`,
`[1, 2]` keep their arguments and `GetType()` gives `Box<Int32>`,
``List`1[System.String]``, as .NET's. Each list of arguments is a constant
of the source's class, worked out once; generic code not knowing its `T`
makes one without (its `GetType()` the definition).
A value type's `T?` is `Nullable<T>` (`typeof(i32?)`); a reference's `?`
is the type itself. A generic method's `MethodInfo` is
called erased (`MakeGenericMethod` gives the same method).

**Expression trees**: a lambda given where an `Expression<Func<…>>` is
wanted is compiled twice — as the tree of its nodes (`ParameterExpression`,
`ConstantExpression`, `MemberExpression`, `MethodCallExpression`,
`BinaryExpression`, `UnaryExpression`, `ConditionalExpression`,
`NewExpression`, `LambdaExpression`, each with its `NodeType` and `Type`,
members as `MemberInfo`s) and as the lambda itself, which is what
`Compile()` gives. Trees made by hand (`Expression.Add(…)`) are compiled by
an interpreter of the nodes.

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
1a. (done) Arrays, as C#'s. `T[]` written and made (`new
   i32[n]`, `new[] { … }`, `i32[] xs = [1, 2];`), jagged `T[][]` and
   rectangular `T[,]`; `Length`, `Rank`, the indexer, `foreach`; an array
   is an `IList<T>` / `IReadOnlyList<T>` (`System.Array`'s statics:
   `Sort`, `IndexOf`, `BinarySearch`, `Reverse`, `Resize`, `Fill`,
   `Copy`, `Empty<T>()`); C#'s covariance of reference arrays. On the JVM
   Java's arrays (`int[]` for `i32[]`, no boxing), seen as an `IList<T>`
   by a view where one is wanted (`Array.AsList`). `Split`, `ToArray`
   and the game's arrays give arrays, as .NET's. What changes an array
   takes it `ref` (`Array.Sort(ref xs)`), what reads one `in`; on the JVM
   a runtime method's `ref` gives the value back (`xs = Array.Sort(xs)`),
   and a generic `T[]` result is made the array wanted (`Array.Of`).
   Calls take type arguments (`Array.Empty<i32>()`).
1b. (done) Right after arrays, what the collections still missed:
   - variance applied: `IEnumerable<out T>`, `IComparer<in T>`, a
     delegate's `in` / `out` — a `List<Derived>` goes where an
     `IEnumerable<Base>` is wanted (Java's wildcards where it lowers);
   - `equals` / `hashCode` on the JVM from a type's `Equals` /
     `GetHashCode` (a struct's from its fields by default, as .NET's
     `ValueType`), so sets and dictionary keys find a struct by value;
   - a class's property implementing an interface's (`Count`) lowered as
     the getter Java's interface wants (beside its field);
   - `System.Object` declared (`Equals`, `GetHashCode`, `ToString`,
     virtual), every type's base: `override` finds it, `5.ToString()` and
     `x.Equals(y)` are there; what a type doesn't answer itself runs on
     the runtime's companion (`host.Object`).
1c. (done) Explicit interface implementations, as C#'s
   (`void ICollection<T>.Add(T item) { … }`): a member reached only
   through its interface. On the JVM each member has a Java name: its
   own, unless Java couldn't tell it from one it inherits (the same
   parameters once generics are erased, yet another signature — `ISet`'s
   `new bool Add(T)` over `ICollection`'s `void Add(T)`, `IDictionary`'s
   `Remove(TKey)` over `ICollection<KeyValuePair<…>>`'s `Remove(T)`), then
   one of its type's (`ISet$Add`); an implementation takes the name of
   what it implements, an explicit one too. The checker notes which
   method a call chose, so the compiler writes that one's name.
1d. (done) `ISet<T>.Add` gives its `bool` (`new bool Add(T item)`), `HashSet<T>`
   implements `ICollection<T>.Add` explicitly — as .NET's.
1e. (done) `IDictionary<TKey, TValue>` is an
   `ICollection<KeyValuePair<TKey, TValue>>` again; `Dictionary` implements
   its `Remove(KeyValuePair<…>)` (and `Add`, `Contains`, `IsReadOnly` of
   pairs) explicitly — as .NET's.
2. `System.Linq` (done: extension methods, C#'s, on `IEnumerable<T>` —
   `Where`, `Select`, `OrderBy`/`ThenBy`, `GroupBy`, `Join`, set
   operators, `First`…`Single`, `Sum`/`Average`/`Min`/`Max` of every
   width, `Aggregate`, `To*`, `Range`/`Repeat`/`Empty` — a generic
   method's type parameters inferred from its lambdas' results as C#'s;
   `OfType`, `Cast` and `Chunk` are `[Reified]`: on the JVM handed their
   type parameters as classes, first, their results made the call's own
   types),
   `Queue`, `Stack`, `SortedSet` (live `GetViewBetween`),
   `SortedDictionary`, `SortedList`, `PriorityQueue`, `LinkedList` /
   `LinkedListNode`, `IComparer` / `Comparer<T>` (`Default`, `Create`),
   `IEqualityComparer` / `EqualityComparer<T>.Default` (done). A generic
   type's statics take its arguments (`Comparer<i32>.Default`); a runtime
   type's property is set by its setter (`set_Value`), an own computed
   one's too.
3. The numbers' and `char`'s members (done: `System.Int32`… `Char`,
   `[Host]` — `i32.Parse`, `TryParse` giving `T?`, `MaxValue`, `char.IsDigit`…,
   a type's keyword before its statics as C#'s `int.Parse`; `decimal` is
   Java's `BigDecimal`, its arithmetic methods, numbers made one through
   its implicit conversion), `Random`, `StringBuilder` (`System.Text`, an
   implicit `using`), `Lazy` (done; numbers and `bool`s put in text are
   written as .NET writes them: `2`, `1E+20`, `True`, `∞`), `TimeSpan`, `DateTime` with `DayOfWeek`, `Void` (done), `System.Numerics`' vectors
   (done, matrices and quaternions too); query syntax (done: every clause, more
   than one range variable carried as a tuple, C#'s transparent identifiers).
4. `System.Reflection`, `System.Linq.Expressions`: `System.Type`
   (done: `typeof`, `GetType()`, names as the CLR writes them, generics
   and arrays made, what a type inherits, `$TYPE` beside each compiled
   type, companions of the runtime's); members (done: `FieldInfo`,
   `PropertyInfo`, `MethodInfo`, `ConstructorInfo`, `ParameterInfo` —
   `GetValue`, `SetValue`, `Invoke` by what's written beside each; a
   library member's types read with its own file's `using`s, as .NET's
   metadata is); attributes (done: `GetCustomAttribute<T>()`,
   `GetCustomAttributes`, `IsDefined` — each made anew from what's
   written, a type's base's inherited; `[Derive(…)]`, naming macros, has
   no value); expression trees (done: a lambda given where an
   `Expression<TDelegate>` is wanted is written as its tree — constants,
   parameters, members, calls by the method chosen, operators, `?:`,
   casts, an array's item — and as itself, which `Compile()` gives; a
   hand-made tree's `Compile()` runs its nodes; `new` by the constructor
   chosen, a string's parts, a lambda inside one — run by the tree, the
   lambdas around it seen — and a type's own operators by their methods
   (`MakeBinary`, `MakeUnary`), described as .NET's `op_Addition`);
   `System.Enum`
   (done: one model for every enum — `ys.md` §3 — its runtime class of
   numbers and data, `GetValues`, `GetNames`, `Parse`, `TryParse`,
   `IsDefined`, `[Flags]`); `BindingFlags` (done: public or not, instance
   or static, declared only, flattened, case ignored — .NET's rules).
