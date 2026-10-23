# Yog Script — a language for Minecraft mods

Status: design draft. Nothing is implemented.

Yog Script (`.ys`) is a language a whole mod is written in, with no Rust
and no Java. Yog Nanite Script (`.yns`) is the same language plus the words
that reach into other code (patches). Both compile through one IR, either
to JVM bytecode injected into the game or to Rust built into the mod's
library (`docs/design/nanites.md`, `docs/design/bus.md`).

It isn't a copy of Rust, C#, Kotlin or a scripting language. It takes what
modders praise in each, drops what they complain about, and adds
mechanisms only a language that *knows it is a Minecraft mod* can have.

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

1. **A mod is declarations.** Blocks, items, recipes, state, screens,
   commands, patches are words of the language, not API calls.
2. **The compiler knows the game.** Every id, class, method, field and
   version is checked at build time against the index of each targeted
   MC version. "Works on 1.20.1, breaks on 1.21" is a build error with the
   line, not a crash.
3. **Sides are types.** Server, client and shared code are told apart by the
   type system: a dedicated server can't reach a client class, because such a
   program doesn't compile.
4. **State is declared, not plumbed.** Where it lives (block, item stack,
   entity, player, world, session), whether it's saved and whether a client
   sees it are modifiers; codecs, packets and mirrors are generated.
5. **No memory management, no nulls by surprise, no exceptions in the
   game loop.** Values and objects; `?` for absent; failures are values.
6. **Hot by default.** Everything can be reloaded in a running game; state
   survives because it's declared.
7. **High level, never less capable.** Anything the Rust API or Mixin can
   do, Yog Script can (`never reduce capabilities`); the low level is still
   there through `native` and `patch`.

## 3. The language core

Braces and expressions; no semicolons; types inferred and written at
boundaries.

```ys
let name = "charger"            // constant
var count = 0                   // variable
count += 1

fn area(w: Int, h: Int) -> Int = w * h

fn greet(p: Player) {
    p.say("Hello, {p.name}!")   // interpolation: {expr}
}

let doubled = [1, 2, 3].map { it * 2 }.where { it > 2 }
let hp = target?.health ?? 0    // ? absent, ?? otherwise

let tier = match tool.tier {
    Wood | Stone -> 1
    Iron -> 2
    else -> 3
}
```

### Types

| Kind | Examples |
|---|---|
| Numbers | `Int` (64-bit), `Int32`, `Float`, `Double`, `Byte` |
| Text, truth | `Text` (translatable: `t"charge.tooltip"`), `Str`, `Bool` |
| Units | `Ticks` (`20t`), `Seconds` (`5s`, convertible to ticks), `Blocks` (`3b`), `Energy` (a mod declares its own units) |
| Bounded | `Int in 0..100` — a range is a type: proven where it can be, clamped where it can't |
| Content | `ItemRef` (`@item:minecraft:stick`), `BlockRef`, `TagRef` (`#minecraft:logs`) — checked against the registries the mod targets |
| Optional | `Player?` — only optional types can be absent |
| Results | `Ok` / `Fail(reason)`: `try` passes a failure up, like Rust's `?` |
| Records, unions | `record Pos(x: Int, y: Int, z: Int)`, `union Shape { Circle(r: Float), Box(w: Float, h: Float) }` |
| Collections | `List<T>`, `Map<K, V>`, `Set<T>` |

Game objects (`Player`, `Level`, `ItemStack`, any MC class) are plain
objects: no ownership, no lifetimes. A game object held past its life
(an entity removed) reads as absent, not as a crash: such references
are weak by type (`Entity?` when stored in state).

### Extension members

Any type, the game's included, can gain members in a mod. They're
visible where the extension is imported:

```ys
extend Player {
    val isCharged: Bool => inventory.any { it is Battery && it.charge > 0 }
    fn zap(amount: Int) { hurt(Damage.lightning, amount) }
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
component Charge(capacity: Energy) {
    saved synced var energy: Energy in 0..capacity = 0
    val room => capacity - energy

    fn push(into: Charge) {
        let moved = min(energy, into.room)
        into.energy += moved
        energy -= moved
    }
}

component Owned {
    saved var owner: Player? = none
    on place(p) { owner = p }          // the owner's events reach its components
}

block Charger has Charge(res.charger.capacity), Owned {
    hardness = res.charger.hardness
    slots inventory[4] of Battery
    on use(p) { p.open(ChargerScreen(self)) }
}

item Battery has Charge(res.battery.capacity) {
    stack = 1
}

extend Player has Charge(res.suit.capacity)   // the game's objects carry them too

server system Solar(c: Charge, b: Block) every 1t {
    if b.level.isDay { c.energy += res.solarRate }
}

server system Feed(c: Charge, s: Slots) every 1t {
    for battery in s.of<Battery> { c.push(battery.Charge) }
}
```

- **Reaching a component.** Its members read on the owner when the name is
  unambiguous (`charger.energy`); else through its name
  (`charger.Charge.energy`). `x has Charge` tests, `x.Charge?` reads it
  when `x`'s type doesn't promise it.
- **Components are types.** A function taking `Charge` takes any owner's
  charge (`push(into: Charge)`); a parameter `c: Charge & Owned` wants both.
- **Systems are queries.** Their parameters are the components (and the
  object types) an object must have; `every` is how often, its side the
  modifier. The compiler compiles a system to the game's indexed
  lookups and ticking of exactly the objects that match — blocks with
  block entities, entities by section — not a scan. A system that takes
  only an object type (`system Glow(m: Monster) every 5t`) is a query too.
- **State is a component's.** `saved` / `synced` (4.2) apply to component
  fields as to any object's: a component's codec and mirror are generated
  once and reused by every owner.
- **Hot reload keeps components.** A component gained or lost by an object
  in a reload is added with its defaults or dropped; its fields migrate by
  `was` (4.2).

## 4. Own mechanisms

### 4.1 Sides are part of a function's type

Every function is `shared` (the default: pure, no side), `server` or
`client`. A `client` function can call only `client` and `shared` code; a
`server` one only `server` and `shared`. The game's own classes carry their
side from the class index (`net.minecraft.client.*` is `client`), so a
client class used in server code is a **build error**, not a dedicated
server crash.

A call across sides is a **message**, generated: the compiler writes the
packet, its codec and both handlers.

```ys
client fn showCharge(level: Int) {
    toast(t"charge.level", level)
}

server fn onCharged(p: Player, level: Int) {
    p.client.showCharge(level)      // server → that player's client: a packet
}

client fn requestDump(b: Charger) {
    b.server.dump()                 // client → server: a packet, validated
}
```

A call from a client to the server checks who calls: `b.server.dump()`
arrives with the sender, and a `server fn` taking a `Sender` can refuse
it (reach, permissions) in one line.

### 4.2 State: placement, saving, sync

State is declared where it belongs, with modifiers instead of code:

```ys
block Charger {
    saved synced var energy: Energy in 0..res.capacity = 0
    saved var owner: Player? = none
    var cooldown: Ticks = 0t          // neither saved nor synced: server memory
}

extend Player {                       // per player: state the game's object carries
    saved var visits = 0
}

extend World {                        // per world
    saved var grid: Map<Pos, Int> = {}
}
```

State added by `extend` is an unnamed component of that type (3,
Objects); a named `component` is the same, shared by several.

- `saved`: in the world's save, in Yog Data (`docs/design/storage.md`
  scopes); the codec is generated.
- `synced`: mirrored to the clients that can see it (F3 mirrors: a client
  reads it without a message per read).
- **Schema changes are part of the language:** a renamed or retyped field
  says what it was, and old saves load.

```ys
saved var energy: Energy = 0 was power: Int     // old saves' `power` read as energy
```

### 4.3 Time and searches are library, saved and indexed

Waiting is `Task` (the standard library, not syntax): `Task.wait(5s)`
suspends a function and the game goes on; the compiler turns a function
that waits into a state machine run by its side's ticks (no `async` /
`await`). A task started from a `saved` object is **saved with it**: a
delay in progress survives a restart, which hand-written tick counters
rarely do.

```ys
block Charger {
    server on tick {
        if level.isDay { energy += res.solarRate }
    }

    server fn arm() {
        glow()
        Task.wait(5s)                    // still pending after a reload
        explode(power = 2.0)
    }
}

Task.every(10s) { … }
Task.until { door.isOpen }
```

Searching the world is a library chain the compiler knows, so it compiles
to the game's indexed lookups (chunk and entity sections), not a scan:

```ys
let threats = level.entities.near(pos, 16b).where { it is Monster && it.canSee(player) }
let nearest = level.blocks.near(pos, 8b).where { it is Charger }.orderBy { it.distance }.first()
```

### 4.5 Failures don't crash the world

A failing handler (a `Fail`, an out-of-range proven at run time, a game
exception thrown into it) is caught at its boundary: it is logged with the
`.ys` line, the handler is disabled for that object (or the mod is marked
faulty, by policy), and the game goes on. A crash report names the
`.ys` file and line, never a generated method.

### 4.6 Live values and tests

- `tune let solarRate = 4` — a constant IDLE can change in a running game
  (it's read through a mirror while tuning, inlined in a release build).
- `test` blocks run in a headless game (a GameTest-like world), on every
  targeted version, from `yog test`:

```ys
test "a charger fills a battery" {
    let w = testWorld()
    let c = w.place(Charger, at = origin)
    c.energy = 100
    let b = c.inventory.put(Battery())
    w.tick(1t)
    expect(b.charge == 100 && c.energy == 0)
}
```

### 4.7 Versions

A mod says which MC versions it targets; the build checks every line
against each. Code that differs per version is a `version` block, the
only place such branches can be:

```ys
version mc >= 1.21 {
    fn armor(p: Player) = p.getArmorValue()
} else {
    fn armor(p: Player) = p.armorValue
}
```

Content and data references are versioned already (`versioned
datapacks`): an id missing on a version is a build diagnostic per version.

## 5. Content

```ys
mod charge {
    version = "1.0.0"
    targets = mc 1.20.1..26.2
    resources = "charge.yres"         // sounds, numbers, text: no magic values
}

block Charger {
    hardness = res.charger.hardness
    tool = Pickaxe
    drops = self

    saved synced var energy: Energy in 0..res.charger.capacity = 0
    slots inventory[4] of Battery

    server on tick {
        if level.isDay { energy += res.solarRate }
        for b in inventory {
            let moved = min(energy, b.room)
            b.charge += moved
            energy -= moved
        }
    }

    on use(p) { p.open(ChargerScreen(self)) }
}

item Battery {
    stack = 1
    saved synced var charge: Energy in 0..res.battery.capacity = 0
    val room => charge.max - charge
    tooltip => t"charge.battery.tooltip"(charge, charge.max)
}

recipe Battery shaped {
    "RIR"
    "IGI"
    "RIR"
    R = @item:minecraft:redstone
    I = @item:minecraft:iron_ingot
    G = @item:minecraft:gold_ingot
}

on player.join(p) {
    p.visits += 1
    if p.visits == 1 {
        p.give(Battery())
        p.play(res.sounds.welcome)
    }
}

command "charge" {
    server fn fill(src: Sender, target: Player = src.player) requires op {
        target.inventory.each<Battery> { it.charge = it.charge.max }
    }
}
```

Screens are view models over `.yui` markup; bindings to `synced` state
are live; actions are `server fn`s (messages, 4.1):

```ys
screen ChargerScreen(block: Charger) ui "charger.yui" {
    val energy => block.energy
    val fill => energy / energy.max
    action dump() { block.energy = 0 }
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
    add saved var shield: Float = 0         // added state (saved, synced work here too)
    add is Chargeable                       // an added interface

    // Before the method. `skip` ends it with a value; nothing: it runs.
    before hurt(source: DamageSource, amount: Float) -> Bool {
        if shield >= amount {
            shield -= amount
            skip false
        }
    }

    // After it: `result` is its value, assignable.
    after getMaxHealth() -> Float {
        result += shield / 2
    }

    // Around a call inside it: `proceed` is the call as it was.
    inside tick() {
        around call aiStep() {
            if !level.isFrozen { proceed() }
        }
    }

    // An argument of a call, by its name in the callee.
    inside knockback(strength: Double, x: Double, z: Double) {
        at call Vec3.add(x) { x * 0.5 }
    }
}

patch MinecraftServer {
    // Locals by name (the class index keeps local names where the game has them);
    // by type and order where it doesn't. `var` writes back.
    after tickServer(hasTime: BooleanSupplier) uses local start: Long, var local spent: Long {
        if spent > res.limits.slowTick { warn("slow tick: {spent} ns") }
    }
}

patch Player {
    // A field's read and write.
    read foodData {
        if hasEffect(@effect:charge:overcharged) { FoodData.full } else { proceed() }
    }
    write experienceLevel(value) {
        if value >= experienceLevel { proceed(value) }
    }

    // A constant in a method.
    inside getDigSpeed() { constant 5.0 -> 3.0 }

    // The whole body. Exclusive: a second mod replacing it is a conflict
    // reported at launch with both mods' names.
    replace canEat(ignoreHunger: Bool) -> Bool {
        ignoreHunger || foodData.needsFood() || (self as Chargeable).shield > 0
    }
}
```

What this fixes:

- **No strings, no ordinals by default.** `aiStep()` and `Vec3.add` are
  resolved against the class index; `at call X` matching more than one
  call is a build error asking for `first`, `last`, or `where`
  (`at call add where arg x is strength`).
- **No magic names.** The receiver is `self` (members reachable directly),
  arguments by their names, `result` in `after`, locals declared by `uses`.
- **No bool to skip.** `skip value` ends the method; falling through runs
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
typed signatures, and Rust can call Yog Script functions. Neither is
required.

```ys
native "charge_grid" {
    fn simulate(nodes: List<Int>, edges: List<(Int, Int)>) -> List<Int>
}
```

## 8. How it compiles

```
.ys / .yns ─► parse ─► resolve (class index of each targeted MC version,
                        yog-ids, the mod's .yres) ─► type check (sides,
                        ranges, optionals) ─► Yog IR (Yog Data, in the .yog)
                                                   │
                         ┌─────────────────────────┴───────────────┐
                         ▼                                         ▼
              JVM: the host lowers IR to bytecode       Rust: `yog build` lowers IR to a
              at launch (ASM; run-time names from        generated crate (rustc): logic that
              the mapping table): patches, ticks,        isn't hot, or a mod's choice
              anything hot; no bus on those paths
```

- The compiler (`yog-script`, in yog-cli) is the only parser: IDLE and
  editors call it (`formats through the CLI`); its errors are
  `yog_data::Diagnostic` with YB codes.
- The class index is `yog mappings` grown to signatures, hierarchy,
  sides and local names, per MC version.
- A pure-JVM mod has no native library: no cross-compilation, builds in
  seconds. Rust lowering is chosen per mod or per function.
- Line numbers: bytecode carries `SourceFile` = the `.ys` and its lines.

## 9. Steps

1. This document; the grammar (`ys-grammar.md`); the IR.
2. Parser, resolver, diagnostics (`yog check`).
3. The class index (signatures, sides, locals) per version; type checker.
4. JVM lowering for patches (`.yns`) — replaces the bus on hot paths.
5. Content, state, sides as messages, `Task`.
6. Rust lowering; `native`.
7. Hot reload, `tune`, `test`; the LSP for IDLE.
