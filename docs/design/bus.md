# The Yog bus — one channel instead of a function per feature

## Why

Today every feature crosses the boundary as its own function, several times:

| Where | Count (ABI 3.10, MC 26.2) |
|---|---|
| `YogServer` function pointers (actions on the world) | 116 |
| `YogApi` function pointers (subscriptions, registrations) | 82 |
| `#[repr(C)]` event structs | ~20 |
| `NativeBridge.java` public statics (per MC version, ×3) | 216 |
| `NativeBridge.java` natives (per MC version, ×3) | 87 |
| JNI entry points in `yog-runtime` | ~90 |

A new event or action means: an ABI field (an ABI bump), a runtime
function, a JNI export, a Java static in every host version, a `yog-api`
wrapper, and a `bridge_methods` test entry. Mods talk to each other through
a third mechanism (`interop_export`/`import`, raw pointers + rkyv), and to
Java mods through a fourth (`java_call`, strings). None of these can be
used by a Java mod to reach a Yog mod.

## The idea

Everything that crosses a boundary is a **message**: an **address** (an id,
`namespace:path`, open vocabulary — anyone declares their own) and a
**payload** in `yog-data` (the binary form both Rust and Java already read
and write: `YogData.encode`/`decode`, `yog_data::binary`). Three verbs
cover every case we have:

| Verb | Shape | Covers |
|---|---|---|
| **call** | `call(address, payload) -> reply` | world actions and queries (`YogServer`), Java calls, a mod's exported functions |
| **emit** / **on** | `emit(address, payload) -> reply`, `on(address, handler, priority)` | events (`on_block_break` …); a pre-event's reply cancels or changes it |
| **provide** | `provide(address, handler)` | who answers a call: the host (Java, per MC version), the runtime, a Yog mod, a Java mod |

Nothing is special about the host: `yog:world/set_block` is a service the
Java host *provides*, `yog:event/block_break` an event it *emits*, exactly
as a mod would. So a Yog mod can call a Java mod's service, a Java mod can
subscribe to a Yog mod's event, and two Yog mods share functions — through
the same three verbs, with no ABI change, ever, for a new feature.

## The ABI that's left

```rust
#[repr(C)]
pub struct YogBus {
    pub abi_version: u32,
    pub size: u32,
    pub ctx: *mut c_void,
    /// An address's handle — resolved once, so hot calls don't hash strings.
    pub resolve: unsafe extern "C" fn(ctx, address: YogStr) -> YogAddress,
    pub call: unsafe extern "C" fn(ctx, to: YogAddress, payload: YogBytes, reply: *mut YogBytesSink) -> YogStatus,
    pub emit: unsafe extern "C" fn(ctx, event: YogAddress, payload: YogBytes, reply: *mut YogBytesSink) -> YogStatus,
    pub on: unsafe extern "C" fn(ctx, event: YogAddress, priority: i32, ud: *mut c_void, handler: YogHandlerFn),
    pub provide: unsafe extern "C" fn(ctx, address: YogAddress, ud: *mut c_void, handler: YogHandlerFn) -> YogStatus,
    pub log: unsafe extern "C" fn(ctx, level: u32, message: YogStr),
    /// Direct tables for what can't afford a message per call (see below).
    pub gfx: *const YogGfx,
}
pub type YogHandlerFn = unsafe extern "C" fn(ud: *mut c_void, payload: YogBytes, reply: *mut YogBytesSink) -> YogStatus;
```

Seven entries instead of ~200. Java's side is the same: one native
`YogBus.nativeDispatch(int address, byte[] payload) -> byte[]` and one
static `YogBus.dispatch(int address, byte[] payload) -> byte[]` the runtime
calls; the per-version host code becomes a table of handlers
(`provide("yog:world/set_block", p -> …)`), not JNI signatures.

**What stays direct.** The low-level GPU pipeline (`YogGfx`, ABI 14) and
other per-frame paths keep their function tables — a message per vertex
buffer bind would cost too much, and capabilities never shrink
(redesigns never drop a capability). They hang off the bus as `gfx` and are
reached only by the code that needs them.

## Typed on top

The generic channel is for the boundary; nobody writes payloads by hand.
Each action is described once (`actions!`, below) and becomes:

- a **function** of its domain — `world::get_block(dim, pos)`,
  `player::give_item(name, item, count)`; what no one answers reads as the
  reply's default (`false`, `None`, empty);
- a **method on a handle** when its first field binds one — `uuid` an
  `Entity`, `player` a `Player`, `dim` a `World`. A handle only holds its
  key; its methods are the functions with the key put in first:

```rust
let me = Player::new(&e.player_name);          // or player::give_item(&e.player_name, …)
me.give_item("minecraft:diamond", 1u32);
if let Some(body) = me.entity() {               // its uuid, asked of the server
    body.teleport((0.5, 100.0, 0.5));
}
World::new(&e.dimension).set_block(pos, "minecraft:torch");
r.before::<Break, _>(|e, _| Flow::Continue);    // events the same way
```

There is no `Server` trait: the functions reach the bus through the caller
`yog-api` (in a mod) or the runtime (in itself) sets once
(`yog_event::set_caller`). Replies are typed records (`EntityInfo`,
`BlockHit`, `Stack`…, Yog Data schemas in `yog-core`) the host builds as
`YogTypes` records — no tab-separated lines anywhere.

The address constants live in `yog-ids` (one source; Java and TypeScript get
them generated), the payload schemas in Rust (`yog-data` `Schema`); Java
gets a typed record per message generated from the same schema, so a Java
mod writes `new YogServices.World.GetBlock(dim, pos).call()`.

## Actions: services the host provides (stage 3)

Every action on the game (`YogServer`'s ~116 functions) is a service the
**Java host** provides — described **once**, in Rust, one line each:

```rust
actions! {
    world::SetBlock { dim: String, pos: BlockPos, block: String } -> bool = "setBlock";
    entity::Health { uuid: String } -> Option<f64> = "entityHealth";
}
```

From that line come the request struct and its `Service` impl (address in
`yog-ids`), the reply type, Java's record (`YogServices`) and the host
method that answers it (`"setBlock"`: a `NativeBridge` method;
`"YogQueries.entityInfo"`: another class's). The host answers **every** action with one generic
provider (`YogHostServices`): the request read as its record, its
components laid out in order (a nested record or a list spreads into its
parts: a `BlockPos` is `x, y, z`), each converted to the type of the host
method's parameter (found by `MethodHandle` — `float` or `double` is the
method's own business), the method called, its result encoded as the
reply (a `YogTypes` record or a list of them as its data; `NaN`,
`Long.MIN_VALUE`, `Integer.MIN_VALUE` and `null` read as "none" for an `Option`).
No per-action code in the runtime or in Java; the runtime's `YogServer`
table and its ~116 functions go. A Java mod calls the same actions through
`YogServices`. The same machinery — typed message → reflective call — is
what nanite reflection is built on.

## Describing the bus

Every address carries a **descriptor**: whether it's a service or an
event, the shape of its payload and reply (`yog-data` `Shape`, the schemas
the formats already use), docs, provider, the interfaces it implements.
The bus answers about them — itself through bus calls, so no ABI:

| Address | Does |
|---|---|
| `yog:bus/declare` | adds a descriptor (done by the derives / `YogBus.declare`) |
| `yog:bus/describe` | an address's descriptor |
| `yog:bus/list` | every address (a prefix narrows it: `pipes:`) |
| `yog:bus/find` | the addresses implementing an interface (`yog:energy/storage`) |

It buys discovery (who stores energy? — a Yog mod's or a Java mod's,
called through one typed interface), checking (a payload against its
declared shape, at the boundary), and bindings (Rust, Java records, the
IDE's TypeScript and a live browser of a running game's addresses and
nanites).

## Shared memory between Rust and Java

Rust and Java live in one process, so a message needn't be copied into a
`byte[]`, boxed into a `Map`, or dispatched by reflection.

* **An arena per thread.** The runtime owns it (64 KB to start, grown with
  a version number in its header); Java sees it once per thread as a direct
  `ByteBuffer` (a `MemorySegment` on Java 22+). A call writes its payload
  there and crosses with primitives only — address id, offset, length —
  the cheapest JNI call there is; the reply is written in place and read
  in place (views, see *The format* below). Nested calls (Java → Rust →
  Java) take frames like a stack, its top in the header: no region is
  written while someone still reads it.
* **Generated dispatch.** Java answers through a generated `switch` on the
  address id calling the host method directly — no `MethodHandle`, no
  record spread by reflection, and javac checks every host method.
* **JNI natives with primitives** on every version. (FFM critical
  downcalls were considered and dropped: a critical call may not call back
  into Java, and a Rust handler calls the host's actions all the time; a
  plain FFM call costs what the JNI one does.)
* **Atoms.** Names that repeat (`minecraft:stone`, a dimension, an entity
  type) go as `u32` through one table a process — the runtime's, which
  every mod's `yog-data` and the host's `YogAtoms` share; Java keeps one
  `String` per atom. Addresses are atoms too: a message type resolves its
  address once and keeps the number (no string looked up per call). Live
  entities' uuids aren't atoms: unbounded, and atoms are never freed.
* **No allocation on the hot path**: reused buffers; SPSC rings between
  the render, client and server threads.
* **Batches.** Changes (`set_block`, particles, effects) go into a ring the
  host drains once a tick — `ChunkWriter`'s way, for every kind.

### Mirrors: hot data, declared

Data read every tick (entities' positions, a mod's pipe flows) needn't be
asked for at all: a **mirror** keeps it in shared memory, written by its
provider at the rate asked, read by anyone in a few nanoseconds. Nothing is
written that no one declared.

```rust
#[yog_mirror(keys = EntityKeys)]            // yog:mirror/<mod>/threat
pub struct Threat {
    #[fact(yog_ids::fact::ENTITY_POS)]      // the host writes it already: linked, not copied
    pos: Point,
    level: f32,                             // its own
}

registry.provide_mirror::<Threat>(|w| for e in hostiles() { w.row(e.id).level(|| e.threat()); });

let near = Threat::declare(ThreatFields::POS | ThreatFields::LEVEL, Rate::Ticks(1));
for t in near.iter() { draw(t.pos(), t.level()); }   // drop(near) stops what only it asked for
```

* **Columns are inodes.** A column is one fact (`yog_ids::fact::*`, one
  source) in one key space (entities by network id, players, blocks): one
  buffer, one writer, a reference count. A mirror is a directory — field →
  column, plus its rows (a bitmap its writer keeps). Declaring links each
  field to the column of its fact if there is one, else makes it; the last
  release frees it and stops its writing. However many mods look at a fact,
  it's stored and written once.
* **One writer a column**: the host first, then the first provider; a
  provider's writer isn't called for a field someone else writes or no one
  asked for (its values are closures). A provider leaving hands its columns
  on or leaves them waiting; a declaration made before its provider loads
  waits too (optional integrations).
* **Demand is merged per column**: fields OR, the most frequent rate, the
  scopes listed (a mirror's own `scope` type; the provider decides what to
  write). Rate: every tick, every *n*, or on change (a changed-since bitmap).
* **Layout**: columns, a shared key → row index per key space, a seqlock
  (small) or two buffers flipped (big); readers never block the writer.
* **Generated** by `#[yog_mirror]`: layout, lazy writer, field flags, view,
  handle released on `drop`, bus descriptor, yog-exports entry (other mods
  `import!` it typed), Java record, writer and view. The host's mirrors
  are declared the same way: one mechanism for host, mods and Java mods.
* Key spaces inherit (a player is an entity): `player.pos` links to
  `entity.pos` through the player → entity map.

### The format on the bus

Yog Data's binary form, made for the bus (see the `versus` and `messages`
benches in `yog-data`): records positional and untagged (both sides are
generated from one schema; fields only appended, a shape fingerprint
catches reordering; files keep tagged self-describing records), frames
without the `YOGD` header, a string table on the stack, `Arc<str>` /
`Box<str>` reused by table index, int lists bit-packed in AVX2-sized
blocks (`frame::lanes`), float lists in `f32` when exact, other lists with
their byte length (stepped over), repeated names as atoms, zero-copy views
(`XxxView<'a>`, lazy lists) derived next to `Schema`, typed Java codecs and
views per record. The goal: beat rkyv, postcard and
serde_json in size and speed.

## Versions without ABI bumps

A payload is a `yog-data` map: a new optional field is a minor change no
one notices, an address no one provides answers `YogStatus::NO_HANDLER`
(the caller decides: a feature not on this host / this MC version). The
ABI major bumps once, for this; after it, features are data.

## Performance

Estimates on paper (±50 %), to be replaced by the benches at each step
(`yog-data/benches/{messages,versus}.rs`, `yog-runtime/benches/{jni_calls,dispatch}.rs`):

| Path | Before | Format (F1) | + transport (F2) | + mirrors, batches (F3) |
|---|---|---|---|---|
| Host action, Rust → Java → reply | 1.5–2 µs | 0.6–0.8 µs | 100–130 ns | same |
| Event, Java → Rust | ~1 µs | ~0.4 µs | 30–50 ns (26.2), 45–60 ns | same |
| Hot read (a position) | 1.5–2 µs | ~0.7 µs | ~110 ns | 1–5 ns |
| Bulk changes | 1.5 µs × N | 0.7 µs × N | 110 ns × N | 10–20 ns × N + one call |
| Allocations a call | 4–6 | 2–3 | 0 | 0 |

A mod making 1000 queries and getting 200 events a tick: ~3.9 % of the
tick before, ~0.26 % after F2, ~0.03 % with mirrors.

## The plan

* **F0** (when builds are allowed): commit the bench and decoder work; take
  the baselines.
* **F1 — format**: positional untagged records; headerless frames; stack
  string table, `Arc<str>`/`Box<str>` reuse; bit-packed / raw int lists,
  `f32` float lists, `f32` bitmask; short-string hash and per-field cache;
  zero-copy views; Java typed codecs (with F2.1); a decoder by `Shape` for
  tools.
* **F2 — transport**: generated Java dispatch (done); per-thread arenas
  with stacked frames (done); no allocation on a call (done: frames packed
  by pooled packers straight into the arena and read there on both sides,
  replies sent to the caller's sink, a payload passed on read where it
  is); batches; atoms (`#[data(atom)]`
  fields: ids sent as numbers, one Java `String` each). SPSC rings between
  threads go with F3.
* **F2.7 — rkyv parity** (the goal: Rust ↔ Java as fast as rkyv's
  in-place access): number lists bit-packed in blocks of 256 (8 lanes of
  32-bit words, a row one AVX2 vector; packed and unpacked with AVX2 where
  the processor has it, the same bytes either way; item `i` in O(1)) and
  float lists narrowed / widened four at a time; lists carry their byte
  length, so views step over them and read them lazily (`List<'a, T>`, no
  `Vec`); Java records hold primitive arrays (`int[]`, `float[]`…), and
  each record has a generated `View` (fields read in place where the frame
  is; `serve` reads requests through it, `callView` replies); event
  dispatch enters a mod once for its handlers in a row (a borrowed guard,
  no reference count) and trades two reply buffers. Measured in `versus`
  (one value read, opening included, against rkyv) and `bus_calls` (whole
  calls on a real JVM: Rust → Java → reply, Java → Rust).
* **F3 — mirrors**: facts in `yog-ids`; the column store (inodes, key
  spaces, directories, merged demand, writer hand-over); `#[yog_mirror]`;
  its Java side; the host's mirrors (entities, players, block areas); Pipes'
  `Flow` as the first mod mirror; key-space inheritance.
* **F4** — the mods move: VLSI, Pipes (`bridge.rs`), Hex, Modlist, Argus;
  their hot data into mirrors.
* **F5** — stage 5 below (nanites); **F6** — stage 6; **F7** — stage 7, with
  the mods' weight and loading first: `yimage` (a palette and bit-packed
  pixels for pixel art, handed to the game in memory, no PNG on disk), each
  player getting only their platform's native, a release's debug symbols
  as an installer component. (A release `.yog` is 0.4–2.4 MB a platform
  today; assets are over half of the biggest.)

## Migration (one major release, staged on master)

1. **Core.** `YogBus` in `yog-abi`; the router in `yog-runtime`
   (addresses, providers, subscribers by priority, Java bridge);
   `YogBus.java` in `java-common/src` with the dispatch native.
   Bus addresses in `yog-ids` (`bus` group).
2. **Events.** Each Java hook emits its event; `RuntimeHandlers`' per-event
   lists, the event structs, the `nativeOnX` exports and `api_on_X` go.
3. **Actions.** Each `YogServer` function becomes a host-provided service;
   the `NativeBridge` statics become handlers; `YogServer` goes.
4. **Interop.** `interop_export`/`import` and `java_call` become `provide`
   and `call`; `#[yog_export]` generates a provider. Bus descriptors.
5. **Nanites.** Mapping-driven generation, reflection (`call`/`get`/`set`),
   then injections (manifest → generated Mixin config → bus events).
6. **Java mods.** A public `dev.yog.api.YogBus` with typed messages.
   Alongside it, fonts brought to full quality:
   - the designers shape whole runs with `yog_font` (ligatures, kerning,
     contextual forms) instead of drawing a `.yfont` char by char;
   - a compact glyph form for pixel fonts (a 1-bit mask, not RGBA images),
     so copies of `yog:font/default` stay small and the whole of Unifont
     (`yog:font/uniform`) can be copied without `--chars`;
   - import, load and shaping profiled and benchmarked.
7. **Mods.** VLSI, Pipes, Modlist, Hex move to the new `yog-api`; release.

Each stage builds and passes its tests; the release is cut after 7.

