# Yog Script — a language for Minecraft mods

Status: design draft. The parser, the resolver, the type checker and the class index exist
(`yog-script-analyze`, `yog-script-build`); typing and lowering don't.

Yog Script (`.ys`) is a language a whole mod is written in, with no Rust
and no Java. Yog Nanite Script (`.yns`) is the same language plus the words
that reach into other code (patches). Both compile through one IR, either
to JVM bytecode injected into the game or to Rust built into the mod's
library (`docs/design/nanites.md`, `docs/design/bus.md`).

Its syntax is close to C# — types first, `var`, `new` and primary
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
| Rust | the borrow checker, lifetimes, compile times for a modder | enums with data, `match`, `Result`, great errors, one build tool |

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
5. **No memory management, no nulls by surprise, no exceptions in the
   game loop.** Values and objects; `T?` for absent; failures are values.
6. **Hot by default.** Everything can be reloaded in a running game; state
   survives because it's declared.
7. **High level, never less capable.** Anything the Rust API or Mixin can
   do, Yog Script can (`never reduce capabilities`); the low level is still
   there through `native` and `patch`.

## 3. The language core

C#'s shape: types first, `var` for locals, `;` after statements, PascalCase
for types and members, camelCase for locals and parameters.

```ys
const string Name = "charger";
var count = 0;
count += 1;

int Area(int w, int h) => w * h;

void Greet(Player player) {
    player.Say("Hello, {player.Name}!");    // interpolation: {expr}
}

List<int> numbers = [1, 2, 3];
var doubled = numbers.Select { it * 2 }.Where(x => x > 2);
var health = target?.Health ?? 0;           // ?. null-conditional, ?? otherwise

var tier = tool.Tier switch {
    Tier.Wood or Tier.Stone => 1,
    Tier.Iron => 2,
    _ => 3,
};
```

### Types

| Kind | Examples |
|---|---|
| Numbers | `int` (32-bit), `long`, `float`, `double`, `byte` |
| Text, truth | `string`, `char`, `bool`, `Text` (translatable: `t"charge.tooltip"`) |
| Units | `Ticks` (`20t`), `Seconds` (`5s`, convertible to ticks), `Blocks` (`3b`), `Energy` (a mod declares its own units) |
| Bounded | `int Level in 0..100` — a field's range: proven where it can be, clamped where it can't |
| Content | `ItemRef` (`@item:minecraft:stick`), `BlockRef`, `TagRef` (`#minecraft:logs`) — checked against the registries the mod targets |
| Nullable | `Player?` — only nullable types can be `null` |
| Results | `Ok` / `Fail(reason)`: `try` passes a failure up, like Rust's `?` |
| Records, unions | `record Pos(int X, int Y, int Z);`, `union Shape { Circle(float R), Box(float W, float H) }` |
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
record Pos(int X, int Y, int Z);

class Timer(Ticks every) {
    Ticks Left = every;
    public Timer() : this(20t) { }          // another constructor
    public bool Tick() {
        Left -= 1t;
        return Left <= 0t;
    }
}

var spawn = new Pos(0, 64, 0);
var timer = new Timer(5s) { Left = 1t };
Timer daily = new(24000t);
var battery = new Battery { Count = 2 };    // content is made the same way
```

### Extension members

Any type, the game's included, can gain members in a mod. They're
visible where the extension is used:

```ys
extend Player {
    public bool IsCharged => Inventory.Any { it is Battery b && b.Charge.Energy > 0 };
    public void Zap(int amount) => Hurt(Damage.Lightning, amount);
}
```

### Objects: components and systems

A mod is objects. Blocks, items, entities and screens are declared
objects; the game's own (`Player`, `Level`, `ItemStack`, the world) are
objects too. What several of them share is a **component**: state and
behaviour written once, carried by any object that `has` it. What runs
over every object carrying some components is a **system**. There is no
class inheritance: an object is its own members plus its components
(composition), while the game's types keep their own subtyping
(`Monster` is an `Entity`).

```ys
component Charge(Energy capacity) {
    saved synced Energy Energy in 0..capacity = 0;
    public Energy Capacity => capacity;
    public Energy Room => capacity - Energy;

    public void Push(Charge into) {
        var moved = Math.Min(Energy, into.Room);
        into.Energy += moved;
        Energy -= moved;
    }
}

component Owned {
    saved Player? Owner = null;
    on Place(Player player) { Owner = player; }   // the owner's events reach its components
}

block Charger has Charge(Res.Charger.Capacity), Owned {
    Hardness = Res.Charger.Hardness;
    Slots<Battery> Inventory = new(4);
    on Use(Player player) { player.Open(new ChargerScreen(this)); }
}

item Battery has Charge(Res.Battery.Capacity) {
    StackSize = 1;
}

extend Player has Charge(Res.Suit.Capacity);     // the game's objects carry them too

server system Solar(Charge charge, Block block) every 1t {
    if (block.Level.IsDay) charge.Energy += Res.SolarRate;
}

server system Feed(Charge charge, Slots<Battery> slots) every 1t {
    foreach (var battery in slots) charge.Push(battery.Charge);
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
client void ShowCharge(int level) {
    Toast(t"charge.level"(level));
}

server void OnCharged(Player player, int level) {
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
    saved synced Energy Energy in 0..Res.Capacity = 0;
    saved Player? Owner = null;
    Ticks Cooldown = 0t;                 // neither saved nor synced: server memory
}

extend Player {                          // per player: state the game's object carries
    saved int Visits = 0;
}

extend World {                           // per world
    saved Dictionary<Pos, int> Grid = [];
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
saved Energy Energy = 0 was Power: int;      // old saves' `Power` read as Energy
```

### 4.3 Time and searches are library, saved and indexed

Waiting is `Task`, awaited as in C#: `await Task.Delay(5s)` suspends an
`async` method and the game goes on; the compiler turns it into a state
machine run by its side's ticks. A task started from a `saved` object is
**saved with it**: a delay in progress survives a restart, which
hand-written tick counters rarely do.

```ys
block Charger {
    server on Tick {
        if (Level.IsDay) Charge.Energy += Res.SolarRate;
    }

    server async void Arm() {
        Glow();
        await Task.Delay(5s);            // still pending after a reload
        Explode(power: 2.0f);
    }
}

Task.Every(10s) { … };
await Task.Until { door.IsOpen };
```

Searching the world is a library chain the compiler knows, so it compiles
to the game's indexed lookups (chunk and entity sections), not a scan:

```ys
var threats = Level.Entities.Near(pos, 16b).Where { it is Monster && it.CanSee(player) };
var nearest = Level.Blocks.Near(pos, 8b).OfType<Charger>().OrderBy { it.Distance }.FirstOrDefault();
```

### 4.5 Failures don't crash the world

A failing handler (a `Fail`, an out-of-range proven at run time, a game
exception thrown into it) is caught at its boundary: it is logged with the
`.ys` line, the handler is disabled for that object (or the mod is marked
faulty, by policy), and the game goes on. A crash report names the
`.ys` file and line, never a generated method.

### 4.6 Live values and tests

- `tune int SolarRate = 4;` — a value IDLE can change in a running game
  (it's read through a mirror while tuning, inlined in a release build).
- `test` blocks run in a headless game (a GameTest-like world), on every
  targeted version, from `yog test`:

```ys
test "a charger fills a battery" {
    var world = TestWorld();
    var charger = world.Place<Charger>(Origin);
    charger.Charge.Energy = 100;
    var battery = charger.Inventory.Put(new Battery());
    world.Tick(1t);
    Expect(battery.Charge.Energy == 100 && charger.Charge.Energy == 0);
}
```

### 4.7 Versions

A `.ys` reaches Yog only, so it has no versions of its own: it runs on
every version the mod does. A `.yns` says the versions its patches are
for, and the build checks every patch against each one's classes; code
that differs by version is a `version` block there:

```yns
targets mc 1.20.1..26.2;

version (mc >= 1.21) {
    patch Player {
        after int GetArmorValue() { result += Bonus; }
    }
} else {
    patch Player {
        read ArmorValue { return proceed() + Bonus; }
    }
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

block Charger has Charge(Res.Charger.Capacity) {
    Hardness = Res.Charger.Hardness;
    Tool = Tool.Pickaxe;
    Drops = this;

    Slots<Battery> Inventory = new(4);

    server on Tick {
        foreach (var battery in Inventory) Charge.Push(battery.Charge);
    }

    on Use(Player player) { player.Open(new ChargerScreen(this)); }
}

item Battery has Charge(Res.Battery.Capacity) {
    StackSize = 1;
    Tooltip = t"charge.battery.tooltip"(Charge.Energy, Charge.Capacity);
}

recipe Battery : Shaped {
    Pattern = ["RIR", "IGI", "RIR"];
    Keys = ['R' => @item:minecraft:redstone, 'I' => @item:minecraft:iron_ingot, 'G' => @item:minecraft:gold_ingot];
}

on Player.Join(Player player) {
    player.Visits += 1;
    if (player.Visits == 1) {
        player.Give(new Battery());
        player.Play(Res.Sounds.Welcome);
    }
}

command "charge" {
    server void Fill(Sender source, Player target = source.Player) requires source.IsOp {
        foreach (var battery in target.Inventory.OfType<Battery>()) battery.Charge.Energy = battery.Charge.Capacity;
    }
}
```

Screens are view models over `.yui` markup; bindings to `synced` state
are live; actions are `server` methods (messages, 4.1):

```ys
screen ChargerScreen(Charger block) : Ui("charger.yui") {
    Energy Energy => block.Charge.Energy;
    float Fill => Energy / block.Charge.Capacity;
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
    add saved float Shield = 0;              // added state (saved, synced work here too)
    add : IChargeable;                       // an added interface

    // Before the method. `skip` ends it with a value; nothing: it runs.
    before bool Hurt(DamageSource source, float amount) {
        if (Shield >= amount) {
            Shield -= amount;
            skip false;
        }
    }

    // After it: `result` is its value, assignable.
    after float GetMaxHealth() {
        result += Shield / 2;
    }

    // Around a call inside it: `proceed` is the call as it was.
    inside void Tick() {
        around call AiStep() {
            if (!Level.IsFrozen) proceed();
        }
    }

    // An argument of a call, by its name in the callee.
    inside void Knockback(double strength, double x, double z) {
        at call Vec3.Add (x) { x * 0.5 }
    }
}

patch MinecraftServer {
    // Locals by name (the class index keeps local names where the game has them);
    // by type and order where it doesn't. `var` writes back.
    after void TickServer(BooleanSupplier hasTime) uses local long start, var local long spent {
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
    inside float GetDigSpeed() { constant 5.0 => 3.0; }

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

## 7. Native code (optional)

A mod may have a Rust crate; its exported functions are callable with
typed signatures, and Rust can call Yog Script methods. Neither is
required.

```ys
native "charge_grid" {
    List<int> Simulate(List<int> nodes, List<(int, int)> edges);
}
```

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
- A pure-JVM mod has no native library: no cross-compilation, builds in
  seconds. Rust lowering is chosen per mod or per method.
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
