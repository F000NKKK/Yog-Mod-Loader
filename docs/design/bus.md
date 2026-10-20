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
`yog-api` keeps (and grows) typed wrappers, generated from one description
per message:

```rust
/// Sets a block. Provided by the host.
#[derive(Message)]
#[message(address = yog_ids::bus::world::SET_BLOCK, reply = bool)]
pub struct SetBlock { pub dim: Dim, pub pos: BlockPos, pub state: BlockState }

server.call(&SetBlock { dim, pos, state })?;       // what a mod writes
bus.on::<BlockBreak>(|e| …);                       // events the same way
```

The address constants live in `yog-ids` (one source; Java and TypeScript get
them generated), the payload schemas in Rust (`yog-data` `Schema`); Java
gets a typed record per message generated from the same schema, so a Java
mod writes `YogBus.call(SetBlock.of(dim, pos, state))`.

## Nanites — portals into the game's code

A Java mod reaches into Minecraft with Mixins and reflection: it hooks a
method of a game class, reads a private field, calls a method no API
exposes. A **nanite** gives a Yog mod the same reach — typed, checked, and
through the bus, so nothing new crosses the ABI.

A nanite is a typed handle to a **member of a game class** (or a Java
mod's): a method or a field, named the way the mod's code names it
(Mojang names), resolved by the host for the running game version.

```rust
// Generated from the game's mappings, per supported version:
use yog_api::nanite::net::minecraft::world::entity::LivingEntity;

// Reflection: typed reads, writes and calls on a live object.
let hp: f32 = LivingEntity::get_health(&entity)?;
LivingEntity::set_health(&entity, hp + 2.0)?;

// Injection: a hook into the method, like a Mixin's @Inject.
#[nanite::inject(LivingEntity::hurt, at = Head)]
fn armor_of_light(e: &mut Hurt) -> Flow {
    if e.source().is(DamageTypes::LIGHTNING) { e.cancel(false) } else { Flow::Continue }
}
```

* **Reflection** — `call`, `get`, `set` on a class, an object handle or a
  static, typed by the member's signature. The host resolves the member
  once (by name and descriptor, mapped for the running version) and keeps
  a handle; a member missing on this version is a clear `NO_HANDLER`, not
  a crash.
* **Injection** — hooks at the points a Mixin has: `Head`, `Return`,
  `Invoke(target)` (around a call inside the method), `FieldGet/Set`. A
  hook gets the arguments (and `this`, the return value at `Return`)
  typed, and can **continue**, **cancel** (with a return value), or
  **change** the arguments / return value. Under the hood the injection is
  a bus event — `nanite:<class>#<member>@<point>` — so several mods hook
  the same method in priority order, and a Java mod can listen to it too.
* **Declared up front.** Injections are woven into classes before they
  load, so a mod lists them (its `#[nanite::inject]`s, collected by
  `yog build` into the mod's manifest); the host turns the manifest into a
  generated Mixin config at startup, one tiny dispatching injector per
  point, each emitting its bus event. Reflection needs no declaring.
* **Typed by generation.** `yog-cli` reads the game's mappings for every
  supported version and generates the nanite modules (classes, members,
  signatures) — the same single-source rule as `yog-ids`: names live in
  the mappings, everything else is generated. A member that differs
  between versions is generated per version behind the mod's targets.

**Mixin parity.** A nanite does everything a Mixin does — no subset:

| Mixin | Nanite |
|---|---|
| `@Inject` (`HEAD`, `RETURN`, `TAIL`, `INVOKE`, `FIELD`, `NEW`, any `@At`; cancellable, set the return value, capture locals) | `#[nanite::inject(…, at = …)]`, the hook's event carries args, `this`, locals and the return value; cancel / change through the reply |
| `@Redirect`, `@WrapOperation`, `@WrapWithCondition` | `#[nanite::redirect]` / `wrap`: the call inside the method goes to the mod, which may run the original |
| `@ModifyArg(s)`, `@ModifyVariable`, `@ModifyConstant`, `@ModifyReturnValue` | changing hooks: the value is the payload, the handler's reply replaces it |
| `@Overwrite` | `#[nanite::overwrite]` (one per method; a second is an error at load) |
| `@Accessor`, `@Invoker`, `@Shadow` | typed reflection: `get` / `set` / `call` on any member |
| added fields and methods, `@Unique`, `@Implements` / interface injection | `#[nanite::extend(Class)]`: members and interfaces added to the class, reached typed |
| priorities among mods | bus priorities on the hook's event |

Nanites are the low level: everything Yog's own API offers is built from
the same pieces, and what it doesn't offer, a mod reaches itself.

## Actions: services the host provides (stage 3)

Every action on the game (`YogServer`'s ~116 functions) is a service the
**Java host** provides — described **once**, in Rust, one line each:

```rust
actions! {
    world::SetBlock { dim: String, #[data(with = "Pos")] pos: BlockPos, block: String } -> bool = "setBlock";
    entity::Health { uuid: String } -> Option<f64> = "entityHealth";
}
```

From that line come the request struct and its `Service` impl (address in
`yog-ids`), the reply type, Java's record (`YogServices`) and the host
method that answers it. The host answers **every** action with one generic
provider (`YogHostServices`): the request read as its record, its
components laid out in order (a nested record or a list spreads into its
parts: a `BlockPos` is `x, y, z`), each converted to the type of the host
method's parameter (found by `MethodHandle` — `float` or `double` is the
method's own business), the method called, its result encoded as the
reply (`NaN`, `Long.MIN_VALUE` and `null` read as "none" for an `Option`).
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
`byte[]`. Each thread gets a **shared buffer**: memory the runtime owns,
seen by Java as a direct `ByteBuffer` (`NewDirectByteBuffer`). A call
writes its payload there, passes offset and length, and the reply comes
back in the same buffer — no allocation, no copy on either side. Mod ↔ mod
calls pass pointers already. The transport changes underneath the same
`call`/`emit`; nothing above it notices.

## Versions without ABI bumps

A payload is a `yog-data` map: a new optional field is a minor change no
one notices, an address no one provides answers `YogStatus::NO_HANDLER`
(the caller decides: a feature not on this host / this MC version). The
ABI major bumps once, for this; after it, features are data.

## Performance

* Addresses resolve to a `u32` once (`resolve` at registration, cached in a
  `static` by the derive); dispatch is an array index.
* Payloads are the binary form with a string table — a few bytes for a
  typical call; replies reuse the caller's sink buffer.
* Hot reads that are called per tick per entity can batch (`call` with a
  list payload) or move to a direct table like `gfx` if measured to matter.

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
7. **Mods.** VLSI, Pipes, Modlist, Hex move to the new `yog-api`; release.

Each stage builds and passes its tests; the release is cut after 7.

