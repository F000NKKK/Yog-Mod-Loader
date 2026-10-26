# Yog Script — a language for Minecraft mods

Status: design draft. The parser, the resolver, the type checker and the class index exist
(`yog-script-analyze`, `yog-script-build`); typing and lowering don't.

Yog Script (`.ys`) is a language a whole mod is written in, with no Rust
and no Java. Yog Nanite Script (`.yns`) is the same language plus the words
that reach into other code (patches). Both compile through one IR, either
to JVM bytecode injected into the game or to Rust built into the mod's
library (`docs/design/nanites.md`, `docs/design/bus.md`).

Its syntax is close to C# — types first, `let`, `new` and primary
constructors, properties, LINQ, `switch` expressions, `async`/`await`, C#'s
naming — without being a copy: it drops what modders complain about and
adds mechanisms only a language that *knows it is a Minecraft mod* can
have. The exact syntax is `ys-grammar.md`.

## 1. What hurts today, what people like

| Where | Complaint | Praise |
|---|---|---|
| Java + loader APIs | registration boilerplate (deferred registers, events, JSON datagen); client classes reached on a server crash it; network sync is hand-written packets on both sides; save codecs written by hand; porting to every MC version | the whole game is reachable; a real IDE |
| Mixin | targets are strings and ordinals that break silently on a new version; two `@Redirect`s on one call: one mod fails; crash reports name generated methods; no hot reload | it reaches anything; injection points are precise |
| Harmony (C# games) | magic parameter names (`__instance`, `__result`); a prefix returns `bool` to say "skip the original"; transpilers are raw IL; overloads ambiguous | prefix/postfix are simple; several mods patch one method side by side |
| KubeJS, Skript | dynamic types: errors only in game; little documentation; Skript slow, wordy | instant reload; readable; no build step |
| Kotlin, C# | compile speed (Kotlin) | null safety, properties, extension members, lambdas, LINQ, pattern matching, attributes, string interpolation |
| Rust | lifetimes everywhere, compile times for a modder | enums with data, `match`, `Result`, great errors, one build tool |

Sources: the Fabric wiki on clashing redirects and Mixin inheritance;
Harmony's wiki and guides on `__instance`/`__result` and transpilers;
KubeJS/ProbeJS reviews; Skript forums.

## 2. Principles

1. **A mod is objects and declarations.** Blocks, items, recipes, state,
   screens, commands, patches are words of the language, not API calls;
   shared behaviour is components (3).
2. **Built on Yog, not on Minecraft.** A `.ys` reaches the game only
   through Yog — its objects, services, events, mirrors and storage, the
   same surface the Rust API has, described once (yog-event) — so it runs
   on every version the mod does, unchanged. The game's own classes are
   reached only from `.yns` (nanites), which says the versions it's for
   (`targets mc …;`) and is checked against the class index of each:
   "works on 1.20.1, breaks on 1.21" is a build error with the line.
   Running code inside the game without the bus (hot paths) is a general
   Yog mechanism too; nanites are for what's specific to a game class, or
   needs Mixin-level access.
3. **Sides are types.** Server, client and shared code are told apart by the
   type system: a dedicated server can't reach a client class, because such a
   program doesn't compile.
4. **State is declared, not plumbed.** Where it lives (block, item stack,
   entity, player, world, session), whether it's saved and whether a client
   sees it are modifiers; codecs, packets and mirrors are generated.
5. **Memory under control, no nulls by surprise, no exceptions in the
   game loop.** Ownership, borrows and lifetimes checked as Rust's, a GC
   beneath as the safety net; `T?` for absent; failures are values
   (`Result`), bugs are panics — Rust's model.
6. **Hot by default.** Everything can be reloaded in a running game; state
   survives because it's declared.
7. **High level, never less capable.** Anything the Rust API or Mixin can
   do, Yog Script can (`never reduce capabilities`); the low level is still
   there through `native` and `patch`.

## 3. The language core

C#'s shape: types first, `let`/`let mut` for locals (Rust's), `;` after statements, PascalCase
for types and members, camelCase for locals and parameters.

```ys
const string Name = "charger";
let mut count = 0;
count += 1;

i32 Area(i32 w, i32 h) => w * h;

void Greet(Player player) {
    player.Say("Hello, {player.Name}!");    // interpolation: {expr}
}

List<i32> numbers = [1, 2, 3];
let doubled = numbers.Select { it * 2 }.Where(x => x > 2);
let health = target?.Health ?? 0;           // ?. null-conditional, ?? otherwise

let tier = tool.Tier switch {
    Tier.Wood or Tier.Stone => 1,
    Tier.Iron => 2,
    _ => 3,
};
```

### Types

| Kind | Examples |
|---|---|
| Numbers | `i32` (32-bit), `i64`, `f32`, `f64`, `u8` |
| Text, truth | `string`, `char`, `bool`, `Text` (translatable: `t"charge.tooltip"`) |
| Units | `Ticks` (`20t`), `Seconds` (`5s`, convertible to ticks), `Blocks` (`3b`), `Energy` (a mod declares its own units) |
| Bounded | `i32 Level in 0..100` — a field's range: proven where it can be, clamped where it can't |
| Content | `ItemRef` (`@item:minecraft:stick`), `BlockRef`, `TagRef` (`#minecraft:logs`) — checked against the registries the mod targets |
| Nullable | `Player?` — only nullable types can be `null` |
| Results | `Result<T, E>`: `Ok(v)` / `Err(e)`; `try` passes an `Err` up, like Rust's `?` |
| Records, enums | `record Pos(i32 X, i32 Y, i32 Z);`, `enum Shape { Circle(f32 R), Box(f32 W, f32 H) }` |
| Collections | `List<T>`, `Dictionary<K, V>`, `HashSet<T>`; collection expressions `[1, 2, 3]` |

Yog's objects (`Player`, `Level`, `Entity`, `ItemStack`, `Block`) are
plain objects: no ownership, no lifetimes. One held past its life (an
entity removed) reads as `null`, not as a crash: such references are weak
by type (`Entity?` when stored in state). They're Yog's, the same on every
game version; the game's own classes are a `.yns`'s only (6).

### Objects and constructors

Types of one's own are made with `new`; a type's constructor is its header
(C# 12's primary constructors), its parameters in scope in the body.
Object initializers set members after the constructor; `new(…)` alone
takes the type from where it goes.

```ys
record Pos(i32 X, i32 Y, i32 Z);

class Timer(Ticks every) {
    Ticks Left = every;
    public Timer() : this(20t) { }          // another constructor
    public bool Tick() {
        Left -= 1t;
        return Left <= 0t;
    }
}

let spawn = new Pos(0, 64, 0);
let timer = new Timer(5s) { Left = 1t };
Timer daily = new(24000t);
let battery = new Battery { Count = 2 };    // content is made the same way
```

### Types: classes, structs, interfaces, records, enums

C#'s type system, with Rust's enums:

- **Classes** inherit one class and implement any interfaces
  (`class Machine : Block, ITickable, IChargeable`); members are
  `virtual`/`abstract` and `override`n, a class `sealed` or `abstract`;
  `base` reaches the base class (`base.Tick()`, `: base(…)`).
- **Structs** (`struct Pos(i32 X, i32 Y, i32 Z)`) are values: copied, no
  inheritance, interfaces allowed.
- **Interfaces** inherit interfaces (any number); members may have bodies
  (default implementations), as in C#.
- **Records** are `readonly` classes (`record`, `record class`) or
  `readonly struct`s (`record struct`): their constructor's parameters are
  their properties, set once; every member is `readonly`; compared by
  value; `with` gives a copy with some properties changed (`pos with { Y =
  64 }`). Nothing changes one, so sharing it is safe: a record is `Copy`
  — a `record class` shares its object (as `string` does), a `record
  struct` copies its bytes. A `readonly class` (every member `readonly`,
  its fields set only where it's made) is shared the same way, and what
  inherits it is `readonly` too: `System.Type`, Reflection's members,
  expression trees.
- **Enums** are one model, C#'s and Rust's at once: every case has its
  number (`enum Tier : u8 { Wood = 1, Stone, Iron }`, else one more than
  the last, from 0) and may carry data (`enum Mode { Idle, Charging(i32
  Rate), Full }`, `Mode.Charging(4)`). Every enum is a `System.Enum`: its
  values' numbers are cast (`(i32)t`, `(Tier)5`), combined (`A | B`, `&`,
  `^`, `~`, `HasFlag`, `[Flags]` naming the bits in its text) and ordered
  (`<`). A value made of a number carries no data — a number naming a case
  that does is a panic. A `switch` covers every named case (a value of a
  number no case names, met by none, panics); a case without data is
  matched as a value, one with by its pattern (`Mode.Charging(let r)`,
  `Maybe<i32>.Just(let v)`, its fields typed by what's matched).
- **Generics**: types and methods (`class Box<T>`, `T Max<T>(T a, T b)`),
  constraints (`where T : class, struct, new(), IComparable<T>, Base`),
  type arguments inferred from a call's arguments; variance on interfaces
  and delegates (`in T`, `out T`).
- **Delegates**: `Action<…>`, `Func<…, R>`, and `Expression<Func<…>>` — a
  lambda as code to read (a query to compile, a binding to follow), as C#'s
  expression trees.
- `typeof(T)` and `x.GetType()` give a `System.Type`; `nameof(x)` a name.

Components stay the way content shares state and behaviour (3, below):
inheritance is for a mod's own classes, components for what several
objects carry.

### Memory: ownership, borrows, lifetimes — and a GC beneath

Rust's guarantees with C#'s ways of writing them. Every value has one
**owner**; the compiler checks who owns, who borrows and for how long, so a
mod controls its memory and resources explicitly. A tracing **garbage
collector** stays beneath as the safety net (the JVM's on the JVM, the
native runtime's own): whatever the checks let through — a cycle of
`Shared`s, a value a host still holds — is still collected, never leaked
for good.

- **Locals**: `let x = …` never changes; `let mut x = …` may. Only a `mut` local is lent by `ref` or assigned. A `foreach` item is read only; `foreach (let mut x in items)` lends each one to be changed (Rust's `iter_mut`).
- **Moves are explicit**: handing an owned value on is written `move x`
  (in an argument or an assignment; returning a local needs none — it
  ends there anyway); after it `x` is gone — using it is a build error. Copies are implicit only for
  `Copy` values: numbers, `bool`, `char`, enums without data, structs and
  `record struct`s made of copies. Anything else is moved, borrowed or
  `Clone()`d — never silently aliased.
- **Passing**, written at both ends as C# does:

  | Parameter | Call | Rust |
  |---|---|---|
  | `T x` | `F(x)` — a copy, or a `move x` | `T` |
  | `in T x` | `F(in x)` or `F(x)` | `&T` |
  | `ref T x` | `F(ref x)`, `x` a `let mut` | `&mut T` |
  | `out T x` | `F(out let x)` | the method must assign it |
  | `move T x` | `F(move x)` | `T`, ownership taken |

- **The borrow rules**, Rust's, checked per path of the code
  (non-lexical): any number of `in` borrows or one `ref` at a time; no
  move or change of what's borrowed while a borrow lives; a borrow never
  outlives its owner.
- **Lifetimes** are inferred inside a method and elided at its signature
  (one borrowed input, or `this`, gives the output's); written where it's
  ambiguous or a type keeps a borrow, Rust's way: `in 'a Item First<'a>(in
  'a List<Item> items)`, `ref struct Cursor<'a>(in 'a List<Item> items)`.
  Only a `ref struct` (C#'s) holds borrows; it never goes to the heap.
- **Dropping**: an owner leaving its scope drops its value at that point —
  `IDisposable.Dispose()` runs, as Rust's `Drop` (C#'s `using` is
  implicit). The GC frees the memory; the drop frees everything else.
- **Shared ownership** is asked for: `Shared<T>` (several owners, cloned
  cheaply — Rust's `Rc`/`Arc`, a cycle collected by the GC), `Weak<T>`
  (doesn't keep it), `Cell<T>`/`Lock<T>` for changing what's shared. The
  game's objects (players, entities, worlds) are handles: `Copy`, owned by
  the game.

#### Values and objects: what a move moves

The same words, two kinds of storage:

| | Value types: `struct`, `record struct`, enums, numbers | Heap types: `class`, `record`, interfaces, delegates, `List<T>`… |
|---|---|---|
| Where it lives | inline: in its local, in the object or array holding it | on the heap, under the GC; the local holds a reference |
| `Copy`? | yes when every field is `Copy` (the compiler sees it, as C# copies every struct) — then `=` and passing copy, and `move` is allowed but changes nothing; a `record struct` always | never: one owner at a time — except a record, which never changes and so is shared |
| Plain `b = a` | a copy (a `Copy` struct); a build error for a struct holding an owned field — write `move a` or `a.Clone()` | a build error — write `move a` (hand the object on), `in a` (lend it), `a.Clone()`, or make it `Shared<T>` |
| `move a` | the bytes go to the new place; `a` is gone | the reference goes, the object stays where it is (O(1)); `a` is gone |
| `in a` | lent without a copy, read-only: no field written, only `readonly` members called | the object lent read-only: no field written, only `readonly` methods called |
| `ref a` | the caller's own value changed in place | the object changed, and the caller's reference may be replaced |
| Drop | when its owner's scope ends (its owned fields first) | when its owner's scope ends, if it wasn't moved: `Dispose()`; the GC frees the memory later |

- **Methods take `this` as C#'s `readonly` says**: a `readonly` method
  (and every member of a `readonly struct`) borrows `this` read-only —
  Rust's `&self`; any other method borrows it to change — `&mut self`, so
  it's called only on a `let mut` or through a `ref`. A `move` method
  (`public move Stream Into() …`) consumes `this` — Rust's `self`.
- **Fields**: an object owns its fields' values. Moving one out leaves a
  hole, so it's written as an exchange: `let old = Replace(ref this.Slot,
  new Item())`, `Take(ref this.Cache)` (leaves `default`) — Rust's
  `mem::replace`/`mem::take`. A local struct may be moved out of field by
  field (`move pos.Item`); it's whole again once that field is assigned.
- **Lambdas** borrow what they capture, as long as they live (a stored
  lambda can't outlive what it borrows); `move x => …` takes ownership of
  its captures, as Rust's `move ||`.
- **Collections own their items**: `list.Add(move item)`; reading
  `list[i]` lends it (`in`), `list.Remove(i)` gives it back by value.
- **Things that aren't owned**: the game's handles (`Player`, `Entity`,
  `World` — ids the game owns), `string` (immutable, shared) and content
  references are `Copy`: passed freely, never moved.
- **A boxed value** (a struct as an `object` or an interface) is a heap
  object of its own: boxing copies a `Copy` struct, moves any other.

### Failures: Result and panics

Rust's model, not exceptions:

- **Expected failures are values**: `Result<T, E>` (`Ok(v)` / `Err(e)`,
  `E` by default `Error`), matched, mapped (`Map`, `MapErr`, `AndThen`,
  `UnwrapOr`, `OkOr` on `T?`) or passed up by `try F()` (Rust's `F()?`) —
  which converts the error when the caller's `E` has a conversion from it
  (`From`). A `Result` dropped unused is a warning (`#[must_use]`).
- **Bugs panic**: `panic!("…")`, `Unwrap()`/`Expect("…")` on an `Err` or a
  `null`, `unreachable!()`, `todo!()`, `assert!(…)`, an index out of range,
  integer overflow in a checked build. A panic unwinds to the handler's
  boundary (4.5), dropping what it owns on the way; there's no
  `try`/`catch`.
- **Panic handlers**: `Panic.SetHook(info => …)` sees every panic of the
  mod (message, `.ys` location, trace) before the boundary reports it;
  `Panic.Catch(() => …)` runs code and gives a `Result<T, PanicInfo>`
  (Rust's `catch_unwind`) for the rare place that must survive a bug —
  a mod's own plugin host, a test.

### Pattern matching

`switch` and `is` match by shape, as in C# and as Rust's `match`:
variants and records taken apart (`Mode.Charging(let rate)`, `Pos(0, let
y, _)`), properties (`{ Health: < 5 }`), lists (`[let first, .., let
last]`), relations and ranges (`> 3`, `1..10`), `and`/`or`/`not`, guards
(`when rate > 3`). A `switch` over an enum or a sealed hierarchy must
cover every case, or say `_`: a case left out is a build error, not a
warning.

```ys
let label = mode switch {
    Mode.Idle => "idle",
    Mode.Charging(let rate) when rate > 3 => "fast",
    Mode.Charging(_) => "slow",
    Mode.Full => "full",
};
```

### Attributes and macros

C#'s attributes and Rust's macros, one system: an attribute is data on a
declaration, a macro is code that runs at build time and writes code, and
an attribute can *be* a macro.

**Attributes** are C#'s: `[Obsolete("use Charge")]`, `[Range(0, 100)]`, on
a type, a member, a parameter, a return value (`[return: NotNull]`), a
generic parameter, the whole mod (`[module: …]`). An attribute is a class
inheriting `System.Attribute`, its uses limited by `[AttributeUsage(…)]`;
its arguments are constants. At run time `System.Reflection` reads them
(`typeof(Charger).GetCustomAttributes<Range>()`); at build time macros
read the very same objects.

**Macros** are Rust's, called with `!` (`name!(…)`, `name![…]`,
`name!{…}`; `!` right after a name and before a bracket, which `x!` —
"not null" — never is):

- **Declarative** — rules matched against the call's tokens, as
  `macro_rules!`: fragments `$e:expr`, `$t:ty`, `$n:ident`, `$p:pat`,
  `$s:stmt`, `$b:block`, `$l:literal`, `$x:tt`, repetitions `$(…),*`
  `$(…);+` `$(…)?`; hygienic (a name the macro makes never meets one at
  the call).

  ```ys
  macro Square {
      ($x:expr) => { ($x) * ($x) };
  }
  macro MaxOf {
      ($x:expr) => { $x };
      ($x:expr, $($rest:expr),+) => { Math.Max($x, MaxOf!($($rest),+)) };
  }
  ```

- **Procedural** — written in Yog Script, compiled and run by the build
  (no game, no files, no network: a sandbox): a method taking the call's
  syntax and giving syntax back through `System.Meta` (trees for
  declarations, expressions, types; `quote { … $name … }` to write them,
  as Rust's `quote!`; `Meta.Error(span, "…")` reporting where the user
  wrote it). Three kinds, as Rust's: function-like (`Sql!(…)`), derive and
  attribute macros — and the last two are attributes:

**Where they meet**:

- A **derive** is an attribute: `[Derive(Clone, Equals, Saved)]` — each
  argument names a derive macro that adds code next to the type (as
  Rust's `#[derive]`); `record`s and enums derive `Equals`/`Hash`/`Clone`
  this way, by the library.
- An **attribute macro** is an attribute class marked as one:
  `[Macro] class Command : Attribute { … static Syntax Expand(Syntax item,
  Command self) … }`. Where `[Command("charge")]` stands, the build runs
  `Expand` on the item and puts its output in its place; the attribute
  stays on what it gives back unless the macro drops it — so Reflection
  still sees it at run time.
- **Helper attributes** are plain attributes a macro reads: a derive
  `[Derive(Saved)]` reads `[Rename("power")]` and `[Skip]` on the fields
  (Rust's `#[serde(rename)]`), declared by the macro's library as
  ordinary attribute classes, so editors complete and check them.
- Macros **write attributes** too (a derive marking what it made
  `[Generated]`), and function-like macros take attribute-typed values.
- **Order**: the preprocessor (`#if`), then macros — expanded until none
  is left (a depth limit stops a loop) — then names, types, borrows. A
  macro's output is checked as if written by hand; a problem in it points
  at the call, with the expansion shown.
- Built in: `panic!`, `assert!`, `assert_eq!`, `todo!`,
  `unreachable!`, `dbg!`, `matches!`, `include_str!`, `nameof` stays an
  operator (C#'s). Yog's own declarations (`saved`, `synced`, content
  kinds) stay keywords; a mod adds its own forms as macros and
  attributes, not new keywords.

### The System library

What every C# program has, as Yog Script's own library: `System` (the
built-in types — `i32` is `System.Int32`, `string` `System.String` with its
members — `Math`, `Action`, `Func`, `Nullable`, `Result`, `Error`,
`Panic`, `IDisposable`), `System.Collections.Generic` (`List<T>`, `Dictionary<K, V>`,
`HashSet<T>`, `Queue<T>`, …), `System.Linq` (LINQ over any `IEnumerable<T>`),
`System.Linq.Expressions`, `System.Text`, `System.Reflection` (`Type`,
`MethodInfo`, `PropertyInfo`, `FieldInfo`, attributes). It's declared in
Yog Script itself (signatures; what runs is the host's or the runtime's),
reachable from every part of Yog. `System`, `System.Collections.Generic`,
`System.Linq` and `Yog` are every file's `using`s, as .NET's implicit
usings; the rest (`System.Reflection`, …) are `using`'d where wanted.

### Extension members

Any type, the game's included, can gain members in a mod. They're
visible where the extension is used:

```ys
extend Player {
    public bool IsCharged => Inventory.Any { it is Battery b && b.Charge.Energy > 0 };
    public void Zap(i32 amount) => Hurt(Damage.Lightning, amount);
}
```

### Objects: components and systems

A mod is objects. Blocks, items, entities and screens are declared
objects; the game's own (`Player`, `Level`, `ItemStack`, the world) are
objects too. What several of them share is a **component**: state and
behaviour written once, carried by any object that `has` it. What runs
over every object carrying some components is a **system**. Content
objects are their own members plus their components (composition); a
mod's own classes inherit as in C# (above).

```ys
component Charge(Energy capacity) {
    public saved synced Energy Energy in 0..capacity = 0;
    public Energy Capacity => capacity;
    public Energy Room => capacity - Energy;

    public void Push(Charge into) {
        let moved = Math.Min(Energy, into.Room);
        into.Energy += moved;
        Energy -= moved;
    }
}

component Owned {
    saved Player? Owner = null;
    Owned() { Place += player => Owner = player; }   // the owner's events reach its components
}

block Charger has Charge(Res.Charger.Capacity), Owned {
    public Slots<Battery> Inventory = new(4);
    Charger() { Use += player => player.Open(new ChargerScreen(this)); }
}

item Battery has Charge(Res.Battery.Capacity) { }

extend Player has Charge(Res.Suit.Capacity);     // the game's objects carry them too

server system Solar(Charge charge, Block block) every 1t {
    if (block.Level.IsDay) charge.Energy += Res.SolarRate;
}

server system Feed(Charge charge, Slots<Battery> slots) every 1t {
    foreach (let battery in slots) charge.Push(battery.Charge);
}
```

- **Reaching a component.** Through its name on the owner
  (`charger.Charge.Energy`); its members read on the owner directly when
  the name is unambiguous (`charger.Energy`). `x has Charge` tests,
  `x.Charge?` reads it when `x`'s type doesn't promise it.
- **Components are types.** A method taking `Charge` takes any owner's
  charge (`Push(Charge into)`); a parameter `Charge & Owned c` wants both.
- **Systems are queries.** Their parameters are the components (and the
  object types) an object must have; `every` is how often, its side the
  modifier. The compiler compiles a system to the game's indexed
  lookups and ticking of exactly the objects that match — blocks with
  block entities, entities by section — not a scan. A system that takes
  only an object type (`system Glow(Monster monster) every 5t`) is a
  query too.
- **State is a component's.** `saved` / `synced` (4.2) apply to component
  fields as to any object's: a component's codec and mirror are generated
  once and reused by every owner.
- **Hot reload keeps components.** A component gained or lost by an object
  in a reload is added with its defaults or dropped; its fields migrate by
  `was` (4.2).

## 4. Own mechanisms

### 4.1 Sides are part of a method's type

Every method is `shared` (the default: pure, no side), `server` or
`client`. A `client` method can call only `client` and `shared` code; a
`server` one only `server` and `shared`. Yog's objects and services
carry their side, and in a `.yns` the game's classes theirs (from the
class index: `net.minecraft.client.*` is `client`), so client code reached
from server code is a **build error**, not a dedicated server crash.

A call across sides is a **message**, generated: the compiler writes the
packet, its codec and both handlers.

```ys
client void ShowCharge(i32 level) {
    Toast(t"charge.level"(level));
}

server void OnCharged(Player player, i32 level) {
    player.Client.ShowCharge(level);   // server → that player's client: a packet
}

client void RequestDump(Charger charger) {
    charger.Server.Dump();             // client → server: a packet, validated
}
```

A call from a client to the server checks who calls: `charger.Server.Dump()`
arrives with the sender, and a `server` method taking a `Sender` can refuse
it (reach, permissions) in one line.

### 4.2 State: placement, saving, sync

State is declared where it belongs, with modifiers instead of code:

```ys
block Charger {
    public saved synced Energy Energy in 0..Res.Capacity = 0;
    saved Player? Owner = null;
    Ticks Cooldown = 0t;                 // neither saved nor synced: server memory
}

extend Player {                          // per player: state the game's object carries
    public saved i32 Visits = 0;
}

extend World {                           // per world
    saved Dictionary<Pos, i32> Grid = [];
}
```

State added by `extend` is an unnamed component of that type (3,
components); a named `component` is the same, shared by several.

- `saved`: in the world's save, in Yog Data (`docs/design/storage.md`
  scopes); the codec is generated.
- `synced`: mirrored to the clients that can see it (F3 mirrors: a client
  reads it without a message per read).
- **Schema changes are part of the language:** a renamed or retyped field
  says what it was, and old saves load.

```ys
saved Energy Energy = 0 was Power: i32;      // old saves' `Power` read as Energy
```

### 4.3 Time and searches are library, saved and indexed

Waiting is `Task`, awaited as in C#: `await Task.Delay(5s)` suspends an
`async` method and the game goes on; the compiler turns it into a state
machine run by its side's ticks. A task started from a `saved` object is
**saved with it**: a delay in progress survives a restart, which
hand-written tick counters rarely do.

```ys
block Charger {
    server Charger() {
        Tick += () => { if (Level.IsDay) Charge.Energy += Res.SolarRate; };
    }

    server async void Arm() {
        Glow();
        await Task.Delay(5s);            // still pending after a reload
        Explode(power: 2.0f32);
    }
}

GameTime.Every(10t, () => { … });     // game time: Yog
await GameTime.Until(() => door.IsOpen);
```

Searching the world is a library chain the compiler knows, so it compiles
to the game's indexed lookups (chunk and entity sections), not a scan:

```ys
let threats = Level.Entities.Near(pos, 16b).Where { it is Monster && it.CanSee(player) };
let nearest = Level.Blocks.Near(pos, 8b).OfType<Charger>().OrderBy { it.Distance }.FirstOrDefault();
```

### 4.5 Failures don't crash the world

A panicking handler (a `panic`, an `Unwrap` of an `Err`, an index out of
range, a game exception thrown into it) unwinds to its boundary: it is logged with the
`.ys` line, the handler is disabled for that object (or the mod is marked
faulty, by policy), and the game goes on. A crash report names the
`.ys` file and line, never a generated method.

### 4.6 Live values and tests

- `tune i32 SolarRate = 4;` — a value IDLE can change in a running game
  (it's read through a mirror while tuning, inlined in a release build).
- `test` blocks run in a headless game (a GameTest-like world), on every
  targeted version, from `yog test`:

```ys
test "a charger fills a battery" {
    let world = TestWorld();
    let charger = world.Place<Charger>(Origin);
    charger.Charge.Energy = 100;
    let battery = charger.Inventory.Put(new Battery());
    world.Tick(1t);
    Expect(battery.Charge.Energy == 100 && charger.Charge.Energy == 0);
}
```

### 4.7 Versions

A `.ys` reaches Yog only, so it has no versions of its own: it runs on
every version the mod does. A `.yns` says the versions its patches are
for (`targets`) and is compiled once each — one binary a version, as the
hosts are built. Code that differs by version is preprocessor
directives, as in C#: the compiler declares `MC` (the version being
compiled for) and the preprocessor — a general one, which knows nothing
of the game — keeps the branch that holds:

```yns
targets mc 1.20.1..26.2;

using Player = System.Game.World.Entity.Player.Player;

patch Player {
#if MC >= 1.21
    after i32 GetArmorValue() { result += Bonus; }
#else
    read ArmorValue { return proceed() + Bonus; }
#endif
}
```

Content and data references are versioned already (`versioned
datapacks`): an id missing on a version is a build diagnostic per version.

## 5. Content

```ys
mod Charge {
    Version = "1.0.0";
    Targets = mc 1.20.1..26.2;
    Resources = "charge.yres";           // sounds, numbers, text: no magic values
}

// What the block is (strength, tool, drops, model, name) is said once, in
// `content/charger.yblock`; the declaration is its behaviour, bound to it
// by id (`Charger`: `charge:charger`). A property here is an error
// (YS0037), a declaration without its file too (YS0038).
block Charger has Charge(Res.Charger.Capacity) {
    public Slots<Battery> Inventory = new(4);

    Charger() {
        Tick += () => { foreach (let battery in Inventory) Charge.Push(battery.Charge); };
        Use += player => player.Open(new ChargerScreen(this));
    }
}

item Battery has Charge(Res.Battery.Capacity) { }   // content/battery.yitem

recipe Battery : Shaped {
    Pattern = ["RIR", "IGI", "RIR"];
    Keys = ['R' => @item:minecraft:redstone, 'I' => @item:minecraft:iron_ingot, 'G' => @item:minecraft:gold_ingot];
}

// In the mod's constructor: `Player.Join += Welcome;`
server void Welcome(Player.JoinArgs e) {
    let player = new Player(e.PlayerName);
    player.Visits += 1;
    if (player.Visits == 1) {
        player.Give(new Battery());
        player.Play(Res.Sounds.Welcome);
    }
}

command "charge" {
    server void Fill(Sender source, Player target = source.Player) requires source.IsOp {
        foreach (let mut battery in target.Inventory.OfType<Battery>()) battery.Charge.Energy = battery.Charge.Capacity;
    }
}
```

Screens are view models over `.yui` markup; bindings to `synced` state
are live; actions are `server` methods (messages, 4.1):

```ys
screen ChargerScreen(Charger block) : Ui("charger.yui") {
    Energy Energy => block.Charge.Energy;
    f32 Fill => Energy / block.Charge.Capacity;
    server void Dump() { block.Charge.Energy = 0; }
}
```

## 6. Patches (`.yns`)

A patch reaches into a class the mod doesn't own (the game's or a Java
mod's). It addresses code **by meaning**: a method by its signature, a
point by what happens there; the build checks every patch against every
targeted version and reports one that no longer matches, with the
nearest candidate.

```yns
patch LivingEntity {
    add public saved f32 Shield = 0;              // added state (saved, synced work here too)
    add : IChargeable;                       // an added interface

    // Before the method. `skip` ends it with a value; nothing: it runs.
    before bool Hurt(DamageSource source, f32 amount) {
        if (Shield >= amount) {
            Shield -= amount;
            skip false;
        }
    }

    // After it: `result` is its value, assignable.
    after f32 GetMaxHealth() {
        result += Shield / 2;
    }

    // Around a call inside it: `proceed` is the call as it was.
    inside void Tick() {
        around call AiStep() {
            if (!Level.IsFrozen) proceed();
        }
    }

    // An argument of a call, by its name in the callee.
    inside void Knockback(f64 strength, f64 x, f64 z) {
        at call Vec3.Add (x) { x * 0.5 }
    }
}

patch MinecraftServer {
    // Locals by name (the class index keeps local names where the game has them);
    // by type and order where it doesn't. `mut` writes back.
    after void TickServer(BooleanSupplier hasTime) uses local i64 start, mut local i64 spent {
        if (spent > Res.Limits.SlowTick) Log.Warn("slow tick: {spent} ns");
    }
}

patch Player {
    // A field's read and write.
    read FoodData {
        return HasEffect(@effect:charge:overcharged) ? FoodData.Full : proceed();
    }
    write ExperienceLevel {
        if (value >= ExperienceLevel) proceed(value);
    }

    // A constant in a method.
    inside f32 GetDigSpeed() { constant 5.0 => 3.0; }

    // The whole body. Exclusive: a second mod replacing it is a conflict
    // reported at launch with both mods' names.
    replace bool CanEat(bool ignoreHunger) {
        return ignoreHunger || FoodData.NeedsFood() || (this as IChargeable)?.Shield > 0;
    }
}
```

What this fixes:

- **No strings, no ordinals by default.** `AiStep()` and `Vec3.Add` are
  resolved against the class index; `at call X` matching more than one
  call is a build error asking for `first`, `last`, or `where (…)`.
- **No magic names.** The receiver is `this` (members reachable directly),
  arguments by their names, `result` in `after`, `value` in `write`,
  locals declared by `uses`.
- **No bool to skip.** `skip value;` ends the method; falling through runs
  it.
- **Patches compose.** `around` is a wrap (MixinExtras' `@WrapOperation`),
  so two mods around one call chain in priority order instead of one
  failing. Only `replace` is exclusive, and its conflict names both mods.
- **Errors in `.yns` terms.** A patch that can't apply on a version is a
  build diagnostic; at launch a failure names the `.yns` line.
- **Hot.** A patch's body is reached through an indirection
  (`MutableCallSite`): a reload swaps the body without touching the class.

## 7. Rust code beside Yog Script (optional)

A mod may have a Rust crate written as Rust mods are — `yog-api`, the
bus — and use it from Yog Script, or the other way. They meet on the bus,
declared by hand on both sides (no generated glue between them): an
address, and a payload that is a `record` with the Rust struct's fields
in the same order (it packs to the same frame).

```ys
using Yog.Native;

// Rust: #[yog::service("charge:simulate")] fn simulate(q: Simulate) -> i64
[Service("charge:simulate")]
static extern i64 Simulate(i64 nodes, i64 edges);

// Rust: yog::emit("charge:overloaded", Overloaded { pos, energy })
record Overloaded(Pos Pos, i64 Energy);
[Event("charge:overloaded")]
public static event Action<Overloaded> Overloaded;

// Rust: yog::call::<Double, i64>("charge:double", Double { x })
[Provide("charge:double")]
static i64 Double(i64 x) => x * 2;
```

- `extern` (C#'s) says the body is elsewhere: a `[Service]` method is a
  call on the bus, its parameters the request's fields, its result the
  reply. A call no one provides panics (`NO_HANDLER`).
- `[Event]` on an `event`: `+=` listens to what Rust (or anyone) emits
  there; `Overloaded.Invoke(…)` emits it.
- `[Provide]` on a method: Rust (or another mod) calls it at that address.
- The attributes are `Yog.Native`'s (`Service`, `Event`,
  `Provide`): `using Yog.Native;`.
- Run by the interpreter (`yog test`), an `extern` call panics unless the
  test provides the address itself (`[Provide]` in the test's source).

## 8. How it compiles

```
.ys / .yns ─► parse ─► resolve (Yog's surface; a .yns also the class
                        index of each version it targets; yog-ids, the
                        mod's .yres) ─► type check (sides, ranges,
                        nullables) ─► Yog IR (Yog Data, in the .yog)
                                                   │
                         ┌─────────────────────────┴───────────────┐
                         ▼                                         ▼
              JVM: the host lowers IR to bytecode       Rust: `yog build` lowers IR to a
              at launch, running in the host through     generated crate (rustc): logic that
              Yog's own hooks (events, ticks, mirrors,   isn't hot, or a mod's choice
              services): no bus on hot paths; a .yns
              through nanites (Mixin-level)
```

- `yog-script-analyze` reads a source (lexer, parser, resolver);
  `yog-script-build` is the compiler; `yog-cli` builds a mod's scripts
  through it (it alone knows projects), and `yog-script` is the
  stand-alone tool. IDLE and editors call the CLI (`formats through the
  CLI`); errors are `yog_data::Diagnostic` with YS codes.
- Yog's surface (objects, services, events, mirrors) is yog-event's
  own description — what Java's and the Rust API's are made from.
- The class index is the game's jars read per MC version: classes,
  signatures, hierarchy, sides and local names. Only a `.yns` is checked
  against it, on each version its `targets` takes.
- Every mod has a native: the generated Rust of its content and `.yui`
  design, with a crate root `yog build` writes when the mod has no
  `src/lib.rs`. Its Yog Script reaches it through generated bindings
  (`ys-bindings.md`). Rust lowering is chosen per mod or per method.
- Line numbers: bytecode carries `SourceFile` = the `.ys` and its lines.

## 9. Steps

1. This document; the grammar (`ys-grammar.md`); the IR.
2. Parser, resolver, diagnostics (`yog check`).
3. The type checker: Yog's surface for `.ys`; the class index (signatures,
   sides, locals) of each targeted version for `.yns`.
4. JVM lowering for patches (`.yns`) — replaces the bus on hot paths.
5. Content, state, sides as messages, `Task`.
6. Rust lowering; `native`.
7. Hot reload, `tune`, `test`; the LSP for IDLE.
