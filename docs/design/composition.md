# Design: Composition instead of inheritance (behaviours, prototypes, capabilities)

Status: **design only, nothing built.** Measured 2026-10-13 against HexMod-Yog (port in
progress — line numbers drift), Yog-Pipes (+ `examples/copper-pipes`), Yog-Modlist and the
mod-facing crates (yog-api, yog-registry, yog-kit, yog-ui, yog-cli content compiler).

Companion docs — this one does **not** repeat them, it plugs into them:
[boilerplate-reduction.md](boilerplate-reduction.md) (F1..F13, I1..I4, D1..D7),
[mvvm-v2.md](mvvm-v2.md) (V*, M*, §12), [materials-and-vanilla-ui.md](materials-and-vanilla-ui.md)
(D10..D14, §4.6 block behaviours, S0a..S9), [ybook.md](ybook.md), [yog-kit.md](yog-kit.md).

Ground rules (same as every other design doc): everything is generic — the loader, runtime and
Java hosts never learn about a specific mod; no backward compatibility — superseded APIs and
content keys are deleted, not deprecated.

**Capability rule (the user, 2026-10-13): nothing mods could do before may become impossible.**
Deleted builders, keys and registries below are API *shapes*; each keeps an equal or stronger
replacement — content `behaviors` by default, plus a public Rust path for code-registered objects
(§3.2, §8). Global events stay (Q10) for anything per-object hooks don't cover.

---

## 0. Decisions at a glance

| # | Decision | Replaces |
|---|---|---|
| C1 | **Behaviours** are *the* unit of reuse for blocks, items, entities: a named, parameterised, optionally stateful bundle of hooks, attached in content (`"behaviors": {...}`). Vanilla-material behaviours from materials §4.6 are just behaviours with a host class and no Rust. | singular `"behavior"` key (§4.6), `BlockDef::connect_groups/inventory(..)` builders, hand-made registries (`pipe::PIPES`), id-match dispatch |
| C2 | Rust side: **traits with default methods** (`BlockBehavior`, `ItemBehavior`, `EntityBehavior`) + one attribute `#[yog_api::behavior("ns:id", state = T, provides(..))]` that computes the hook mask and self-registers. | "virtual methods" emulated with `match id` |
| C3 | **Per-object dispatch** in the hosts (`YogBlock`/`YogItem` call native only if the hook bit is set). Order: global `before::<E>` → behaviours in declaration order (`Flow::Handled` stops) → global `on::<E>`. Hooks come from the I1 `yog_events!` table with a new *scope* column. | global handlers that string-filter by id |
| C4 | **Typed per-instance state**: generic `YogBlockEntity` (only where needed), item component bytes, entity attachments. State is `Codec`, owned by the behaviour's mod, dropped with the object. | `Storage::open_chunk` KV as a block-entity substitute, Pipes `persist.rs` |
| C5 | **Capabilities** = "interfaces": `behavior::<B>()`, `has::<Marker>()`, `provides(dyn Trait)` within a mod; exported capability traits across mods. | Java `instanceof IotaHolderItem`-style checks done via id lists; Pipes `bind_handler(mod, symbol)` |
| C6 | **Systems**: batch hooks (`tick(all: &mut Instances<Self>, ..)`), one native call per behaviour type per tick. Unity-style authoring, Bevy-style execution for hot paths. Pure ECS rejected. | per-position ticking in global `on_server_tick` |
| C7 | **Content prototypes** (yog-cli only): `"based_on"`, `"abstract": true`, directory `_defaults`, deterministic cascade. | F5 `"extends"` (renamed), copy-pasted `.yblock`/`.yitem` |
| C8 | **No inheritance macros in Rust.** Closed sets stay `enum` + `match`; per-variant constants via a generic `#[derive(Props)]`; mod-local `macro_rules!` for trivially repeated impls. | — |

---

## 1. Audit: where the missing inheritance hurts

In Java, inheritance does **five different jobs** in a typical mod. Yog currently has *data-only*
defs (`BlockDef`/`ItemDef`) plus *global* events, so every job is re-implemented by hand:

| Java job | Java mechanism | What Yog mods do today |
|---|---|---|
| J1 object hooks | `class FocusItem extends Item { use(..) }` | global `on_use_item` + `if id == ..` / `starts_with(..)` |
| J2 capabilities | `interface IotaHolderItem`, `ADMediaHolder`, `AkashicFloodfiller` | id lists + `match stack.id.as_str()` tables |
| J3 per-instance state | `BlockEntity` subclass with NBT | chunk `Storage` KV keyed by pos, hand-rolled save files |
| J4 internal abstract classes | `abstract class Mishap`, `CastingEnvironment`, `Arithmetic` | enum + N parallel `match`es |
| J5 content families | `new StaffItem(props)` ×11, block registration helpers | copy-pasted JSON files |

### 1.1 HexMod-Yog

| Where | Pattern | Count / size | Job |
|---|---|---|---|
| `src/casting/actions.rs:184-258` | `actions!` table (F1 **already landed** in the mod): kinds `konst/spell/arith/raw`, helpers `const_op`/`spell_op` :110-160; registry still hand-aggregated at :273-280 + `ops/mod.rs` | konst 79 (math 22, lists 3, rw 7, stack 8, query 39), arith 38 (math 28, lists 10), spell 3, raw 20 (meta 15, lists 1, stack 4) | J5 (solved) |
| `src/casting/arith.rs` (389) | each arith operator name repeated in table row, `arity()` :20-27 and `apply()` arms; stringly typed | 38 ops × 3 places | J4 |
| `src/item/holder.rs` (351, tests from :303) | `is_scroll` :21, `is_holder` :26-31 (incl. `minecraft:pumpkin_pie`), `read` :43-58, `writeable` :61-71, `can_write` :73-83, `write` :86-94, `write_datum` :96-~158 — **4 `match stack.id` tables over 6 kinds + pie**; entity delegation :160-221; spellbook pages :223-301; datum as text in NBT via `wire` | ~290 non-test lines | J1+J2+J3 |
| `src/item/media.rs` (173) | `STATIC` :24-30 (5 items: worth, priority), `is_packaged` :34, `draws_from_inventory` :42, `breaks_after_depletion` :47, `holds_media` :52, `== BATTERY` :108, :117 | 6 id lists | J2 (`ADMediaHolder` subclasses) |
| `src/item/pigment.rs` (256) | `DYES`/`PRIDE` tables :30-~65, `provider()` :80-97 parses id prefixes `dye_colorizer_`/`pride_colorizer_`; 35 pigment `.yitem` files | 2 tables + prefix parser | J2+J5 |
| `events/mod.rs:30`, `ui/spellcasting.rs:516` | `starts_with("hexcasting:staff/")`; `spellcasting.rs:584` spellbook\|abacus check | 3 id checks | J1 |
| `src/akashic.rs` (119) | bookshelf datum in chunk KV :33-63 (`store`/`key`/`shelf`/`set_shelf`); `is_floodfiller` :29 id list; `BOOKSHELF` checks :101, :116. **Bug: nothing clears the KV when a bookshelf is broken → stale datum resurfaces at that pos.** | ~35 lines plumbing | J2+J3 |
| `src/casting/mishap.rs` (426) | `enum Mishap` 27 variants :28-56; five per-variant matches :157-350 (`resolution_type`, `accent_color` = 20 lines of constants, `particle_spray`, `execute`, `error_message`) — 77 `Mishap::` arms; a new mishap touches 5 places | 5 parallel matches | J4 |
| `src/casting/env.rs` (871) | `enum Source { Staff, Packaged, Circle(CircleRun) }` :209-216 = Java `CastingEnvironment` subclasses; matched at :320, 365, 387, 445, 504, 522, 629 | 7 match sites | J4 (fine as is) |
| `src/casting/args.rs` (373) | 6 identical `impl Arg` :19-66, local `entity_kind!` :257-273, 5 identical `impl Ret` :298-330 | 11 impls | J4 |
| `src/casting/ops/rw.rs` (135) | `read_held`/`write_held`/`entity_holder`/`write_entity` ~:72-135 clone-and-simulate `can_write` over `holder::*`; `akashic/read|write` check `env.block_id(pos) != akashic::RECORD` | ~75 lines glue | J2 |
| `content/**/*.yblock` | 48 files / 1342 lines; **36 of 48** in 6 groups with identical `block` props (9 stone h2, 9 wood h3, 8 stone h4, 4 glass, 3 wood h2, 3 leaves) | ~36 near-duplicates | J5 |
| `content/**/*.yitem` | 71 files / 919 lines; **62** are exactly `{"stack_size":1}` | 62 duplicates | J5 |
| impetus/directrix (7 `.yblock`) | plain cubes: no facing, no BE, no logic — blocked on J1/J3 | — | J1+J3 |

### 1.2 Yog-Pipes

| Where | Pattern | Job |
|---|---|---|
| `src/lib.rs:62-108` | three global handlers (place/break/explosion) filtered with `pipe::is_pipe_block`, `normalize_dim` tuples; explosion revalidation exists only because there is no per-block `onRemove`; lifecycle wiring of persist/graph/handler :110-128 | J1 |
| `src/pipe.rs` (68) | static `PIPES` registry: `register_pipe`/`pipe_def`/`is_pipe_block`/`pipe_groups` — a hand-made behaviour registry | J1+J5 |
| `src/handler.rs` (97) | pos → `(mod_id, symbol)` map + unsafe interop dispatch :58-97 | J2 cross-mod |
| `src/transport.rs` (126) | per-node payload/handler, `bind_handler(pos, mod, symbol)` | J2+J3 |
| `src/persist.rs` (370, tests from :272) | guesses the world dir and saves per-position state | J3 |
| `src/graph.rs` (527) | `on_placed`/`on_removed`/`revalidate_area`/`ensure_known`/`rebuild_around` — real logic, but fed by global events | — |
| `examples/copper-pipes/src/lib.rs` (34) + `lib.designer.rs` (37, generated duplicate) | `register_pipe(PipeDef{block_id, kind: Item, link_groups: <copy of yblock connect_groups>, speed 2.0})` + a `ShapedRecipe` | J5 |

### 1.3 Yog-Modlist

`mod_list.rs:26-45`: three matches over `ModSource` (label, color, badge) — J4, per-variant
constants. `lib.rs` 33 lines. The screen itself is covered by mvvm-v2 §15.3.

### 1.4 Mod-facing API

- yog-api `registry.rs:1706-1874`: 25 global `on_xxx` methods + tick/server lifecycle. **No
  per-object hooks, no block entities** (yog-inventory only mentions BE as future work).
- yog-registry `BlockDef`/`ItemDef` are data. `connect_groups`, `connects_to_neighbors`,
  `inventory(id)` are *de facto* attached behaviours with ad-hoc builders.
- `Storage::open_chunk/world/player` is used as a BE substitute (akashic, pipes-like code).
- yog-ui `Widget` = common struct + `WidgetKind` enum (widget.rs:7/138) — this *is* the right
  composition pattern for a closed set; no change.
- yog-cli `content.rs`: allowed keys :952-957, `"register": "manual"`, generated `Blocks::x()`
  + `register()`.

**Conclusion.** The boilerplate is not "Rust lacks `extends`". It is that the platform has no
place to *attach code and state to an object*, so every mod rebuilds virtual dispatch (id
matches), interfaces (id lists), and block entities (KV) by hand — each slightly differently and
some buggy (akashic stale data, pipes explosion revalidation, pipes world-dir guessing).

---

## 2. Options compared

| Option | Perf | Compile errors | IDE (rust-analyzer) | Hot reload | ABI | Verdict |
|---|---|---|---|---|---|---|
| A. Inheritance emulation macro (`#[extends(Base)]`, `#[delegate]`, Deref chains) | ok | poor (errors in generated code, "method not found" through Deref) | poor (invisible methods, macro-expanded impls) | fragile base-class problem across mods | needs layout agreements | **reject** |
| B. Status quo: global events + id filtering | every mod's handler is called for every event, string compares | ok | ok | ok | small | the problem |
| C. Pure ECS (Bevy-style world mirror) | great for batch | good | good | good | huge: mirror of vanilla world state in Rust, desync with vanilla's object/event model, every vanilla interaction becomes a sync problem | **reject** as foundation; borrow *systems* |
| D. Unity-style components (behaviours attached to objects, inspector params) | per-object dispatch, zero cost for objects without hooks | good if params are typed | good (plain traits) | good (state is data) | one generic vtable | **adopt (authoring)** |
| E. Trait objects + capability lookup (`dyn Trait`, `behavior::<B>()`) | one indirect call | good | good | good | in-mod only (TypeId unreliable across cdylibs) | **adopt within a mod**, exported traits across mods |
| F. Content prototypes (`based_on`, `_defaults`) | build-time only | yog-cli diagnostics | IDLE | n/a | none | **adopt** for J5 |
| G. `enum` + `match` (+ `Props` derive, `enum_dispatch`) | fastest | excellent (exhaustiveness!) | excellent | n/a | none | **keep** for closed internal sets (J4) |

**Recommended mix:** D+E for J1/J2/J3 (behaviours + capabilities + state), F for J5, G for J4,
with Bevy-style *systems* (C's good part) only where batch execution matters (ticks).

Compared with Unity: same authoring model (components on objects, params in an inspector, many
per object), but hooks are statically known per type (mask), so there is no reflection and no
per-frame `Update()` call for objects that don't need it. Compared with Bevy: we don't own the
world, vanilla does; behaviours are "components whose storage is the vanilla object" and systems
iterate only the instances the host told us are loaded.

---

## 3. Behaviours (C1–C6)

### 3.1 Content form

```jsonc
// content/blocks/akashic/bookshelf.yblock
{
  "based_on": "hexcasting:_wood_h3",
  "behaviors": {
    "hexcasting:akashic_bookshelf": {},
    "hexcasting:floodfiller": {}
  }
}

// content/blocks/copper_pipe.yblock (copper-pipes)
{
  "behaviors": {
    "minecraft:transparent": {},                       // host class, materials §4.6
    "yog:connect":   { "groups": ["pipe_item"] },     // was connect_groups
    "yog-pipes:pipe": { "kind": "item", "speed": 2.0 }
  },
  "states": { "connect": "arm" }
}
```

- **Object keyed by id**, not a list: merge-friendly for prototypes (§4), params merge per field,
  `"ns:id": null` removes an inherited behaviour. Declaration order (JSON object order, which
  yog-cli preserves) is the dispatch order.
- Replaces materials §4.6's singular `"behavior"`: `"behavior": "minecraft:transparent"` becomes
  `"behaviors": {"minecraft:transparent": {}}`; `{type, color}` becomes
  `{"minecraft:stained_glass": {"color": "red"}}`.
- **At most one host-class behaviour per block** (`minecraft:*` materials or a mod behaviour
  declared with `host_class`); two is a build error naming both.
- Built-in generic behaviours: `yog:connect` (connect groups / neighbour arms), `yog:inventory`
  and `yog:menu` (bind to mvvm `<Menu>` §12.7 and BE state). These replace the
  `BlockDef::connect_groups/connects_to_neighbors/inventory` builders.
- **Patch files** attach behaviours to foreign or vanilla ids:
  `content/patches/minecraft/pumpkin_pie.yitem` →
  `{"behaviors": {"hexcasting:iota_holder_pie": {}}}`. Host-class behaviours cannot be patched
  onto foreign ids (the Java class is fixed at registration) — build error.
  Foreign/vanilla objects carry no per-object hook bit, so the runtime dispatches hooks of
  patched behaviours from the host's global event hooks, filtered by id through the patch
  table in Rust (no extra JNI per mod). The behaviour's code is the same as on own ids.

Items and entities use the same key (`.yitem`, later `.yentity`).

### 3.2 Rust API

```rust
/// "virtual methods" — every hook has a default no-op body.
pub trait BlockBehavior: Behavior {
    fn on_use(&self, ctx: &mut BlockCtx<Self>, hit: &BlockHit) -> Flow { Flow::Pass }
    fn on_placed(&self, ctx: &mut BlockCtx<Self>, by: Option<&Entity>) {}
    fn on_removed(&self, ctx: &mut BlockCtx<Self>, cause: RemoveCause) {}
    fn on_neighbor_changed(&self, ctx: &mut BlockCtx<Self>, from: BlockPos) {}
    fn random_tick(&self, ctx: &mut BlockCtx<Self>) {}
    fn signal(&self, ctx: &BlockCtx<Self>, side: Direction) -> u8 { 0 }
    // generated from the I1 table, scope = block
}
pub trait ItemBehavior: Behavior { /* on_use, on_use_on_block, on_hit_entity, inventory_tick, tooltip, .. */ }
pub trait EntityBehavior: Behavior { /* stage C5 */ }
```

A behaviour in a mod:

```rust
#[derive(Codec, Default)]
pub struct AkashicBookshelf;                     // params (none here)

#[derive(Codec, Default)]
pub struct Shelf { pub datum: Option<Iota> }     // per-instance state

#[yog_api::behavior("hexcasting:akashic_bookshelf", state = Shelf)]
impl BlockBehavior for AkashicBookshelf {
    fn on_removed(&self, ctx: &mut BlockCtx<Self>, _: RemoveCause) {
        akashic::flood_fill_invalidate(ctx);     // state itself is dropped by the platform
    }
}
```

The attribute:
- reads which trait methods are overridden → `const HOOKS: HookMask` (the host never calls
  absent hooks);
- emits `impl Behavior { const ID; type Params = Self; type State = Shelf; }` (state is named in
  the attribute because associated type defaults are unstable);
- emits ctor registration (same mechanism as `#[yog_export]`);
- `provides(dyn IotaHolder)` registers a capability vtable (§3.6).

Params: the behaviour struct *is* its params (`#[derive(Codec, Default)]`, fields public).
yog-cli's content codegen emits them as **Rust struct literals**:

```rust
// .yog-build/content.rs (generated)
blocks.attach::<yog_pipes::Pipe>("copper-pipes:copper_pipe",
    yog_pipes::Pipe { kind: PipeKind::Item, speed: 2.0, ..Default::default() });
```

so rustc type-checks every param; errors are remapped to `.yblock:line` the same way mvvm V8
remaps `.yui`. The literal is Codec-encoded and passed to runtime
`attach(kind, object_id, behavior_id, bytes)`, which routes it to the owner mod. Same path for
same-mod and cross-mod behaviours (owner resolved through `yog_exports` of dependencies).

`attach` is also **public mod API** (capability rule): a block/item registered from Rust
(`"register": "manual"` or a `BlockDef`/`ItemDef` built in code, e.g. in a loop) attaches
behaviours the same way — `BlockDef::new(..).behavior(yog::Connect { groups: vec![..] })` or
`blocks.attach::<B>(id, params)` before registration freezes. Content is the default, code stays
possible; this is what replaces the deleted `connect_groups`/`connects_to_neighbors`/`inventory`
builders.

yog-cli also syn-scans `#[behavior]` attributes (id index → unknown-id errors with did-you-mean,
JSON schema for editors, IDLE inspector field list).

### 3.3 Runtime, ABI, hosts

- Runtime: `behavior_id → (mod, vtable)` and `object_id → [(behavior, params, mask)]` indexes,
  built at registration; the union mask per object is pushed to the host.
- One generic `YogBehaviorVTable` (create params, drop, `hook(id, ctx, args) -> Flow`,
  `system(id, batch)`, `save_state`/`load_state`/`drop_state`). It generalises materials'
  `YogBlockBehaviorVTable`; the per-frame declarative rules of §4.6 (skip-rendering, shape) stay
  in Java and are just behaviour metadata.
- Hosts: `YogBlock`/`YogItem` (+ the per-material subclasses of §4.6, which keep only the
  vanilla-class difference) override `use`, `onPlace`, `onRemove`, `neighborChanged`,
  `randomTick`/`isRandomlyTicking`, `getSignal`, … and call native **only when the object's mask
  has the bit**. Objects without hooks cost zero JNI — unlike global events, where every mod's
  handler is invoked and string-filters.
- Hook list is not hand-written: the I1 `yog_events!` table gets a **scope column**
  (`global | block | item | entity`). One row generates the global event *and* the scoped hook
  (ABI struct, runtime dispatch, JNI glue, Rust trait method).

### 3.4 Dispatch order

```
global before::<E>   (cross-cutting, can cancel)
→ behaviours of the object, in declaration order; Flow::Handled stops the chain
→ global on::<E>     (observers)
```

Prototype-inherited behaviours come first (base before derived), patch-attached ones last.

### 3.5 Per-instance state

| Object | Storage | Lifetime |
|---|---|---|
| Block | one generic `YogBlockEntity` per host, created **only** for blocks whose behaviours declare `state` or ticking systems. Decoded state lives in the owner mod's arena keyed by `DimPos`; the BE holds a handle and calls `save_state`/`load_state` (Codec bytes in NBT) on chunk save/load. `ctx.state_mut()` sets dirty; `#[behavior(.., sync)]` sends state to clients (feeds mvvm `MenuSync`/`Synced<T>` §12.4). | dropped with the block — **any** removal cause (player, explosion, piston, `/setblock`) |
| Item | `yog:<behavior id>` bytes in item components (NBT on 1.20.1); decoded on access, cached by content hash | lives with the stack |
| Entity | attachments (C5); players use F4 `PlayerData` until then | with the entity |

Hot reload: state is Codec data, serialised before unload and decoded after; a failed decode
(changed layout) falls back to `Default` with a warning — never a crash.

### 3.6 Capabilities ("interfaces")

```rust
if let Some(focus) = stack.behavior::<Focus>() { .. }            // concrete behaviour
if world.block(pos).has::<Floodfiller>() { .. }                   // marker behaviour
if let Some(h) = stack.capability::<dyn IotaHolder>() { h.read(&stack) }  // provides(..)
```

- In-mod: `provides(dyn Trait)` registers a vtable; lookup is by behaviour list on the object.
- Cross-mod: `TypeId` is not reliable across cdylibs, so capability traits that other mods use
  are **exported** — `#[yog_export] pub trait PipeEndpoint { fn on_payload(&self, ctx, p: PipePayload) -> Flow; }`
  — and invoked through the runtime with Codec args (same machinery as F11 `Interop::call`).
  An addon implements it with `#[behavior("addon:sink", provides(yog_pipes::PipeEndpoint))]`.
  This replaces Pipes' `bind_handler(pos, mod, symbol)` and the unsafe dispatch in handler.rs.

### 3.7 Systems (orchestration)

Hooks are per instance. For hot paths a behaviour may declare a batch system instead:

```rust
#[yog_api::behavior("yog-pipes:pipe", state = Node)]
impl BlockBehavior for Pipe {
    #[system(tick)]
    fn tick(all: &mut Instances<Self>, srv: &dyn Server) { /* move payloads */ }
}
```

The runtime calls it once per behaviour type per tick with all loaded instances (host reports
load/unload of BEs). One native call per type instead of one per block; data is contiguous
in the mod's arena. This is the Bevy part; authoring stays Unity-like.

### 3.8 Validation and tooling

- yog-cli: unknown behaviour id (did-you-mean), param type errors (rustc, remapped),
  two host classes, host class on a patch, `null` on a behaviour not inherited.
- IDLE inspector (C6): object → list of behaviours with editable params, generated from the
  syn index + schema; shows which hooks each behaviour overrides.

---

## 4. Content prototypes (C7)

All in yog-cli, zero runtime/ABI.

- `"based_on": "<ns:id>"` — inherit another (possibly abstract) definition. Renames F5's
  `"extends"` to match `.ytheme`'s `based_on` (materials §6.6). ybook keeps `"extends"` for its
  different meaning (addon merges into a base book, ybook §3.8).
- `"abstract": true` — file is a prototype only, not registered (convention: `_name` stem).
- Directory `_defaults.yblock`/`_defaults.yitem` (F5) — applies to every file in the dir and
  subdirs.
- **Cascade:** dir defaults (outer → inner) → `based_on` chain (root → leaf) → the file itself.
  Deep merge per key; `behaviors` merged by id, params merged per field; `null` deletes.
- Cycles and missing bases are build errors with the chain printed.

Hex effect: the 36 grouped `.yblock` files lose their duplicated `block` section (6 abstract
prototypes instead), and 62 `{"stack_size":1}` items become empty or vanish under a
`_defaults.yitem` (`{"stack_size":1}`) — combined with D5 (one file per entry) and Q14 (id from
stem).

Optional `"variants"` (one file describing 16 dye pigments / 11 staffs with per-variant params)
is open question Q3 — it conflicts with D5's recommendation.

---

## 5. Rust-internal patterns (C8)

Inside a mod, closed sets are best served by plain Rust:

- **`enum` + `match`** for closed sets (Mishap, `env::Source`, `ModSource`). Exhaustiveness checks
  are a feature, not boilerplate. `env.rs`'s 7 `Source` matches stay.
- **`#[derive(Props)]`** (generic, in yog-macros, sibling of F6 `Tr`): per-variant constants as
  attributes so one variant = one place:

  ```rust
  #[derive(Props, Tr)]
  #[props(accent: u32, resolved: ResolutionType)]
  pub enum Mishap {
      #[props(accent = 0xFF_80_80, resolved = Error)] #[tr("hexcasting.mishap.not_enough_args")]
      NotEnoughArgs { needed: usize, got: usize },
      ..
  }
  // generates fn accent(&self) -> u32, fn resolved(&self) -> ResolutionType
  ```

  Mishap: `accent_color` (20 lines) + `resolution_type` + most of `error_message` collapse into
  attributes; `execute`/`particle_spray` stay real matches (real logic). Modlist's 3 matches →
  attributes (−~17 lines).
- Trait default methods and blanket impls for shared logic; crates.io `enum_dispatch` if a mod
  wants trait-object ergonomics over an enum.
- Mod-local `macro_rules!` for 1-line repetitive impls (Hex `Arg`/`Ret` ×11).
- Hex arith: arity becomes a column of the `actions!` row (mod-local), so an operator lives in
  the row + its `apply` arm.

Explicitly **not** provided: `#[extends]`, `#[delegate]`, Deref-inheritance. Reasons: fragile
base across hot-reloadable cdylibs, methods invisible to rust-analyzer, errors pointing into
generated code, and they solve J4 which is already cheap with enums.

---

## 6. Before / after

Numbers are non-test, non-blank lines; "after" are estimates.

### 6.1 Hex: IotaHolder item family + rw action family

Before: `holder.rs` ~290 (4 id-match tables, NBT text plumbing, entity delegation, spellbook
pages), `ops/rw.rs` held/entity glue ~75, staff filter in `events/mod.rs` 10, `media.rs` id
lists ~35, pigment prefix parsing ~20.

After:

```rust
pub trait IotaHolder {
    fn read(&self, s: &ItemCtx) -> Option<Iota>;
    fn write(&self, s: &mut ItemCtx, iota: Option<Iota>) -> Result<(), Mishap> { Err(Mishap::BadItem) }
}

#[derive(Codec, Default)] pub struct FocusData { pub iota: Option<Iota>, pub sealed: bool }

#[yog_api::behavior("hexcasting:focus", state = FocusData, provides(dyn IotaHolder))]
impl ItemBehavior for Focus {}
impl IotaHolder for Focus {
    fn read(&self, s: &ItemCtx) -> Option<Iota> { s.state::<Self>().iota.clone() }
    fn write(&self, s: &mut ItemCtx, iota: Option<Iota>) -> Result<(), Mishap> {
        let d = s.state_mut::<Self>();
        if d.sealed { return Err(Mishap::Sealed) }
        d.iota = iota; Ok(())
    }
}
// + ThoughtKnot (write-once), Scroll (params: { readonly }), Slate, Abacus, Pie (patch file),
//   Spellbook (keeps its page logic as state { pages, idx })
```

rw ops become `stack.capability::<dyn IotaHolder>()` (and the same for entities/blocks via
`world.block(pos).capability::<dyn IotaHolder>()`, which also replaces `!= akashic::RECORD`).
Staff: `#[behavior("hexcasting:staff")] fn on_use(..)` on a `_staff` prototype (11 staffs
`based_on` it) → opens the screen via mvvm `open_on=` (§12.5).
Media: `"hexcasting:media_source": {"worth": 10000, "priority": 2}` params replace `STATIC` and
the 4 id lists. Pigments: `"hexcasting:pigment": {"colors": [..]}` params replace `DYES`/`PRIDE`
+ prefix parsing.

| | Before | After |
|---|---|---|
| holder.rs | ~290 | ~120 (trait + 7 small behaviours; spellbook pages stay) |
| rw.rs glue | ~75 | ~45 |
| staff handler | 10 | ~6 |
| media/pigment id lists + prefix parsing | ~55 | ~0 (params in content) |
| **Total** | **~430** | **~170** |

And: an addon can add an iota holder without touching Hex.

### 6.2 Pipes: a pipe type

Before: copper-pipes `lib.rs` 34 (+ generated `lib.designer.rs` 37) duplicating `connect_groups`
from the `.yblock`, Yog-Pipes `lib.rs` 132 with three id-filtered global handlers and explosion
revalidation, `pipe.rs` 68 hand registry, `handler.rs` 97 symbol dispatch, `persist.rs` 370.

After: copper-pipes has **no Rust** (recipe via F13 `.yrecipe`; the `.yblock` in §3.1 carries
`"yog-pipes:pipe": {"kind":"item","speed":2.0}`; groups read from the sibling `yog:connect`).
Yog-Pipes:

```rust
#[yog_api::behavior("yog-pipes:pipe", state = Node)]
impl BlockBehavior for Pipe {
    fn on_placed(&self, ctx: &mut BlockCtx<Self>, _: Option<&Entity>) { graph::on_placed(ctx) }
    fn on_removed(&self, ctx: &mut BlockCtx<Self>, _: RemoveCause) { graph::on_removed(ctx) }
    #[system(tick)] fn tick(all: &mut Instances<Self>, srv: &dyn Server) { transport::tick(all, srv) }
}
```

| File | Before | After |
|---|---|---|
| copper-pipes `lib.rs` (+ designer) | 34 (+37) | 0 |
| Yog-Pipes `lib.rs` | 132 | ~40 |
| `pipe.rs` | 68 | ~25 (params struct + accessors) |
| `handler.rs` | 97 | ~30 (`PipeEndpoint` exported capability) |
| `persist.rs` | 370 | 0 (BE state) |
| `graph.rs` | 527 | ~500 (drops `revalidate_area`, `normalize_dim` callers) |

`on_removed` fires for every cause, so explosion revalidation and world-dir guessing go away.

### 6.3 Block with a block entity: akashic bookshelf

Before: `akashic.rs` 119 — KV plumbing ~35 (`store`/`key`/`shelf`/`set_shelf`), `is_floodfiller`
id list, `BOOKSHELF`/`RECORD` id checks, flood fill ~30; stale-data bug.
After: `Shelf` state (§3.2) + `hexcasting:floodfiller` marker behaviour on the record, bookshelf,
ledger, etc. via a prototype; flood fill uses `has::<Floodfiller>()`. ~119 → ~60, bug fixed
because state dies with the block.

### 6.4 Screen

Modlist and Hex spellcasting are covered by mvvm-v2 §15.3 (Modlist Rust 235 → ~120, Hex
non-grid ~150 → ~45) — not repeated. Composition adds: `ModSource` via `Props`+`Tr` (−~17),
staff `on_use` → `open_on=`. A block that opens a menu (crucible-style) becomes
`"yog:menu": {"ui": "mymod:crucible"}` with `<Menu>`/`<Slot>` bound to the block's behaviour
state through `MenuSync` (mvvm §12.4/§12.7) — no hand-written open/sync code.

---

## 7. Overlaps with other docs (amendments)

| Doc | Amendment |
|---|---|
| materials §4.6, D12, S3a | `"behavior"` → `"behaviors"` object; `minecraft:*` materials are host-class behaviours; `YogBlockBehaviorVTable` generalises into `YogBehaviorVTable`; `BlockBehavior::new(..).extends(..)` builder replaced by `#[behavior]` + content `based_on`. Declarative per-frame rules unchanged. S3a merges with C1. |
| materials D14 | migration step: `behavior` → `behaviors`, `connect_groups`/`inventory` → `yog:connect`/`yog:inventory`, `extends` → `based_on`. |
| boilerplate F5 | `"extends"` renamed `"based_on"`; cascade defined in §4 here. |
| boilerplate F4/D6 | `WorldData`/`PlayerData` stay for global/per-player data; **per-position data goes to behaviour state**, not chunk `Storage`. |
| boilerplate F11 | `Interop::call` stays as the transport; Pipes `PipeDef` builder / `bind_handler` superseded by behaviour params + exported capability. |
| boilerplate I1 | `yog_events!` table gains a scope column (§3.3). |
| boilerplate D4/D5 | unaffected; `variants` (Q3) must agree with D5. |
| boilerplate F13 | copper-pipes' recipe is the canonical "zero-Rust content mod" example. |
| mvvm §12.4–12.7 | `<Menu>` binds to `yog:menu` + behaviour state; `open_on=` used by item behaviours. |
| ybook §3.8 | keeps `"extends"` (addon merge) — different word on purpose. |

## 8. Deletions

- yog-registry: `BlockDef::connect_groups`, `connects_to_neighbors`, `inventory(id)` builders
  (→ `yog:connect`, `yog:inventory` in content, or `.behavior(..)`/`attach` from Rust for
  code-registered blocks, §3.2 — capability rule); `"register": "manual"` only where still needed.
- Content: singular `"behavior"` key, `"connect_groups"` top-level key, F5 `"extends"`.
- yog-api: materials' `BlockBehavior::new(..)` builder; `Storage::open_chunk` — Q8, replaced
  by chunk attachments (§8a), not removed without a replacement.
- Yog-Pipes: `register_pipe`, `pipe_def`, `is_pipe_block`, `pipe_groups`, `PipeDef`,
  `bind_handler(mod, symbol)`, `handler.rs` unsafe dispatch, `persist.rs`, explosion
  revalidation handler.
- Pattern (docs/templates): "global handler + `if id == ..`" for object-specific logic.


### 8a. Chunk attachments (replaces `Storage::open_chunk`)

Per-object state belongs to behaviours (C4). Data that has no own block — on vanilla blocks,
in air, per-chunk metadata (pollution, claimed area, a scan cache) — goes into **chunk
attachments**:

```rust
#[derive(Codec, Default)] struct Aura { level: u16 }
let aura = chunk.data::<Aura>();                     // one value per chunk
let marks = chunk.at::<Mark>(pos);                    // sparse per-position values
marks.set(Mark { by: player.uuid() }); marks.remove();
for (pos, m) in chunk.all::<Mark>() { … }
```

- **Saved with the chunk**, in the chunk's own data (Fabric/NeoForge attachments, Forge
  capability), so they load, unload, copy with structures and survive exactly like the
  chunk — no separate KV files, no orphaned entries (the akashic bug class goes away).
- **Fast:** values live Rust-side in a per-chunk slot map keyed by type id; per-position data
  is a sparse map keyed by the packed section-local position (`u16`), so lookups are a hash
  probe without JNI. Decoding is lazy (first access after load); writes only set a dirty
  bit, and encoding (`Codec`, `zero_copy` views for large blobs) happens once at chunk save.
  Unloaded chunks drop their slots after save.
- **Events:** `on_chunk_load::<T>` / `on_chunk_unload::<T>` for mods that index attachments;
  removal of the block at a position does not clear `at::<T>(pos)` (that data is not owned by
  a block) unless the mod registers `clear_on_block_change`.
- Typed, namespaced by the mod's type (two mods can't collide), versioned through `Codec`.

## 9. Staged plan

| Stage | Content | Depends on |
|---|---|---|
| C0 | Prototypes in yog-cli (`based_on`, `abstract`, `_defaults`, cascade, diagnostics); `#[derive(Props)]` in yog-macros. No ABI. | F5, F6 |
| C1 | Behaviour core: `Behavior`/`BlockBehavior`/`ItemBehavior` traits, `#[behavior]` attribute + mask, runtime index, `attach`, `YogBehaviorVTable`, host per-object dispatch, I1 scope column, content `behaviors` + param literals + error remap. Merge with materials S3a. | F2 (Codec), D2, I1 |
| C2 | Block entity state (`YogBlockEntity`), item state, capabilities (`behavior::<B>`, `has`, `provides`), `yog:connect`/`yog:inventory`/`yog:menu`. | C1 |
| C3 | Systems (`#[system(tick)]`, `Instances<Self>`). | C2 |
| C4 | Cross-mod exported capability traits. | C2, F11 |
| C5 | Entity behaviours + attachments. | C2 |
| C6 | IDLE inspector (behaviour list, params, hooks). | C1 |
| Migrations | Yog-Pipes + copper-pipes (first real consumer, after C3/C4) → Hex (handed to the porting agent: holders, media, pigments, akashic, staff, impetus/directrix) → Modlist (`Props`). | — |

## 10. Open questions (with recommendations)

| # | Question | Recommendation |
|---|---|---|
| Q1 | `behaviors` as object vs list — **decided: yes, as recommended** | **Object** keyed by id (merge, `null` removal); order = declaration order. A list only if the same behaviour must appear twice — not seen in any audited mod. |
| Q2 | Params as generated Rust literals vs JSON schema validated at runtime — **decided: yes, as recommended** | **Rust literals** (rustc checks types, no runtime parse); schema generated only for editors/IDLE. |
| Q3 | `"variants"` for data families vs D5 one-file-per-entry — **decided: no `variants`.** | Sources keep one file per id (D5): variations of one id (block states / properties) stay in its file, different ids never share a file; families share data through `based_on` / `_defaults`. Expanding families is only a build-output concern, never an authoring format. |
| Q4 | Convert `connect_groups`/`inventory` to `yog:*` behaviours — **decided: yes, as recommended** | **Yes** — one mechanism; no special keys. |
| Q5 | Cross-mod capabilities via exported traits — **decided: yes, as recommended** | **Yes**, through the runtime with Codec args; no raw symbol binding. |
| Q6 | Item state format | One `yog:<behavior id>` component per stateful behaviour (Codec bytes); 1.20.1 → NBT tag of the same name. |
| Q7 | When to do entities (C5) | After Hex/Pipes migrations; players use F4 `PlayerData` meanwhile. |
| Q8 | Delete `Storage::open_chunk`? — **decided: replace with chunk attachments (§8a)** (capability rule: data on vanilla blocks, in air and per-chunk metadata must stay possible; user: keep every capability, as fast as possible) | — |
| Q9 | Hook ordering beyond declaration order (priorities) | **No** priorities; declaration order + `before`/`on` globals is enough. |
| Q10 | Keep global events at all? — **decided: yes, as recommended** | **Yes**, for cross-cutting concerns (logging, protection, "any item use"); object-specific logic must use behaviours (lint in yog-cli: global handler whose body starts with an id compare → hint). |
| Q11 | Mod-defined host classes (custom Java superclass) | Only via the generic built-in set (§4.6 table) + declarative rules; mods never ship Java. |
