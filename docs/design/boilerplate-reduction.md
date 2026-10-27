# Boilerplate reduction: audit and plan

Status: proposal (read-only audit, 2026-10-13; nothing was built or run).
Scope, in priority order:

1. Mod code: `Yog-Modlist`, `Yog-Pipes`, `HexMod-Yog` (`src/**`, `content/**`, `yog.toml`).
2. The public surface those mods use: `yog-api`, `yog-kit`, `yog-ui`, `yog-book`,
   `yog-interop` (proc macros), `yog-network`, and the code `yog-cli` generates.
3. Internal loader code (`yog-runtime`, `yog-abi`, `yog-api/src/registry.rs`, Java hosts):
   only the worst repetition.

The project has no backward compatibility, so every proposal below replaces the old API and
deletes it. There are no deprecation shims. Every published crate whose public API changes
gets a breaking version bump. Where `yog-abi` changes, the ABI version changes too, and all
mods must be rebuilt (they are rebuilt by `dev.sh` anyway).

Line counts are measured on the current tree (`wc -l`, `grep -c`). Savings are estimates
of hand-written lines removed from mods. Lines of generated code are not counted.

---

## 0. Ranked summary

| # | Finding | Where today | Proposal | Saves now | Saves as mods grow |
|---|---|---|---|---|---|
| 1 | One HexMod action is spread over 5 places: enum, key table, VM `match` arm, name `match`, `.yres` | `protocol.rs`, `casting/vm.rs`, `ui/casting_screen.rs` | one `#[yog_api::register(ACTIONS)]` fn per action (D4) + typed stack args (`FromIota`/`IntoIota`) | ~60 of 67 lines in `vm::execute`, plus about 40 in the tables | **largest**: about 15 lines per action × the ~250 Hex Casting patterns ≈ 3–4k lines |
| 2 | Hand-written byte and text codecs (3 already, plus 1 being written now) | Pipes `persist.rs` (~100), Hex `protocol.rs` (~75), `iota/mod.rs` (~60), the new `wire.rs` (191) | `#[derive(Codec)]` in yog-api; it replaces `packet!` / `PacketField` | ~420 | every mod that saves or sends anything |
| 3 | Untyped network channels: `&str` constants, `&[u8]` payloads, decode inside each handler | Hex `protocol.rs`, `events/mod.rs`, `casting_screen.rs` | typed `C2S<P>` / `S2C<P>` channel constants + `registry.on(CHANNEL, …)` | ~25 | ~6 lines per packet |
| 4 | World and player state kept by hand: statics, poisoned-lock helpers, clear-on-stop, a world-dir finder, autosave, a versioned blob | Pipes `persist.rs` (370 incl. tests), `graph.rs` / `handler.rs` / `pipe.rs` statics; Hex `vm.rs` STACKS / NAME2UUID | ready-made `WorldData<T>`, `PlayerData<T>`, `ServerState<T>`; API gaps `Server::world_dir`, player uuid on packets | ~300 | any mod with saved state (Hex stacks are not saved today) |
| 5 | Content JSON says the same thing in every file (`name`, `tooltip`, model texture = file path, identical block props) | Hex `content/**`: 119 files, 2261 lines | conventions plus directory defaults (`_defaults.yitem` / `.yblock`) | ~1400 JSON lines | linear in content |
| 6 | `match` from enum to localized text, written by hand | Modlist `ModSource` ×2, Hex `action_name` / `mishap_text` / `iota_kind`, the new `text.rs` (150) | `#[derive(Tr)]`, which compiles against the `res::Strings` keys; a ready-made `yog_api::Text` (translatable and network-safe) | ~200 | +1 line per variant instead of 3 places |
| 7 | Event handlers repeat `if phase == Post { … } true` | 5 of 5 handlers in mods | `registry.on::<E>()` (after the action) and `registry.before::<E>()` (can cancel); `DimPos` on events | ~25 | ~4 lines per handler |
| 8 | The mod entry point is the same ritual in each mod: struct + `impl Mod` + `export_mod!` + `resources!` + `ui_components!` + `content!` + `screen::register` calls | 3 × `lib.rs` | `#[yog_api::main]`; screens register themselves (as `content!` does); `<Screen menu_entry=…>` | ~15 per mod | every new mod and every new screen |
| 9 | `[run.*]` blocks copied 6× per mod | 3 × `yog.toml` (18 blocks, ~126 lines) | `[run.matrix]` that derives JAVA_HOME from `<loader>/versions/<mc>.properties` | ~110 | — |
| 10 | Book written as struct literals plus page-key lists | Hex `book.rs` (124) | `BookEntry` / `BookCategory` builders + numbered-key arrays from `.yres` (or a `.ybook` content file) | ~80 | ~10 lines per entry |
| 11 | Pipes duplicates parts of yog-api: recipes, models, unsafe C-ABI interop calls, struct literals | `recipe.rs` (110), `handler.rs::dispatch` (40), `model.rs`, examples | `yog_api::Recipe` enum (exportable), `Interop::call`, `PipeDef` builder | ~200 | — |
| 12 | View-model glue: `if let Some(h) = handle()`, trivial getters, `TestHost` setup | `casting_screen.rs`, `mod_list.rs`, tests | generated `update()`, binding fields directly, `TestHost::opened` | ~35 | small |
| 13a | Recipes written in Rust, because raw `data/` JSON differs between MC versions | Pipes `recipe.rs`, copper-pipes example | `.yrecipe` content files generated per MC version + build-time registration snapshot (F13) | ~50 | every recipe becomes a small content file |
| 13 | Internal: one new event is ~24 files and ~30 edit sites; one host call is 4 Rust layers + 6 Java copies; one def field is ~10 sites + 6 Java files | `yog-abi`, `yog-runtime`, `registry.rs`, the Java hosts | `yog_events!` / `server_api!` tables, defs as a serialized string, a shared Java tree (§3) | ~6k Rust + ~9-12k Java | each new event becomes 1 table row |

Also found during the audit (bugs and dead code, not boilerplate). These need an owner and
should be coordinated with the agent porting HexMod gameplay:

- `HexMod-Yog/src/iota/mod.rs`: `IotaTypeId`, `IotaType`, `BoolIotaType` / `DoubleIotaType` /
  `PatternIotaType`, `IotaTypeRegistry` and `register_builtin_types` are never used (~100 lines).
- `HexMod-Yog/src/casting/actions.rs`: `Action` trait, `ActionRegistry`, `CastResult`.
  `casting/mishaps.rs`: `Mishap`, `MishapContext`. Both are unused. They also duplicate
  `protocol::Action` / `protocol::Mishap`.
- `Yog-Pipes/src/model.rs:94` `apply()` is a no-op (`let _ = &_model.texture; block`).
  `PipeDef::model` is accepted and its textures, elements and faces are ignored. Only the
  bounding box affects anything, through `resolve_shape`.

---

## 1. Mod code

### F1. HexMod actions: one table instead of five places

**Today.** Adding one pattern touches:

1. `protocol.rs:24` the `enum Action` variant
2. `protocol.rs:68` the `Action::ALL` key table (`(Action::Reveal, "reveal")`)
3. `casting/vm.rs:57` an arm in `execute()`, matched on the angle signature
4. `ui/casting_screen.rs:135` an arm in `action_name()` (`Action::Reveal => Strings::action_reveal()`)
5. `resources/Strings.yres` the `action.<key>` text

Each new mishap adds three more places: the `protocol::Mishap` variant, its
`encode` / `decode` arms (`protocol.rs:86-113`) and `mishap_text()` (`casting_screen.rs:146`).

The VM arms repeat the same stack handling: pop, check the type, put the wrong iota back,
report a mishap.

```rust
// casting/vm.rs:67-82 (Compass' Purification). "wa" at :85-98 is the same code.
"aa" => match stack.pop() {
    Some(Iota::Entity(u)) => match Entity::new(srv, u.to_string()).position() {
        Some((x, y, z)) => {
            stack.push(Iota::Vec3(x, y, z));
            Outcome::Ok(Action::CompassPurification)
        }
        None => Outcome::Mishap(Mishap::EntityGone),
    },
    Some(other) => {
        stack.push(other);
        Outcome::Mishap(Mishap::ExpectedEntity)
    }
    None => Outcome::Mishap(Mishap::StackEmpty),
},
```

So 6 patterns cost 67 lines of VM, about 15 lines of tables and 6 `.yres` lines. Hex Casting
has about 250 patterns. Most of them take 1–3 typed arguments and push 0–2 results (upstream
expresses this as `ConstMediaAction { argc; execute(args) }`). At the current density the
port would come to about 4–5k lines of `match` arms.

**Proposal: a table-driven declaration with typed arguments.** It lives in the mod (a
`macro_rules!` macro in `casting/actions.rs`), so it needs no loader release and can start
now.

```rust
// casting/actions.rs
pub trait FromIota: Sized { const KIND: IotaKind; fn from_iota(i: Iota) -> Result<Self, Iota>; }
pub trait IntoIotas { fn push_onto(self, stack: &mut Vec<Iota>); } // (), Iota, T: Into<Iota>, tuples

actions! {
    /// Mind's Reflection: push a reference to the caster.
    MindsReflection     "qaq"   (cx)                    { cx.caster().map(Iota::Entity) }
    /// Compass' Purification: entity -> its position.
    CompassPurification "aa"    (cx, e: EntityRef)      { e.position(cx).ok_or(Mishap::EntityGone) }
    AlidadePurification "wa"    (cx, e: EntityRef)      { e.look(cx).ok_or(Mishap::EntityGone) }
    Reveal              "de"    (cx, top: Peek<Iota>)   { Ok(()) }
    JestersGambit       "aawdd" (cx, a: Iota, b: Iota)  { Ok((b, a)) }
    ClearStack          "dsw"   (cx)                    { cx.stack.clear(); Ok(()) }
    Add                 "waaw"  (cx, a: f64, b: f64)    { Ok(a + b) }
}
```

The macro generates:

- `#[derive(Clone, Copy, PartialEq, Eq, Debug, Codec, Tr)] #[tr(prefix = "action")] pub enum Action { … }`.
  This replaces `protocol::Action`, `Action::ALL`, `key()` / `from_key()` and `action_name()`.
  A missing `.yres` key becomes a compile error, because `Tr` calls `Strings::action_<snake>()` (F6).
- `Action::signature(self) -> &'static str` and `Action::from_signature(&str) -> Option<Action>`
  (a `match` on string literals, which the compiler turns into a jump table).
- `fn run(self, cx: &mut CastCx) -> Result<(), Mishap>`. It pops `argc` arguments with
  `FromIota`. If the stack is too short it returns `Mishap::NotEnoughArgs { need, got }`. If a
  type does not match it **restores the stack** and returns
  `Mishap::BadIota { index, expected: T::KIND }`. Then it pushes the result with `IntoIotas`.

`vm::execute` shrinks to:

```rust
fn execute(sig: &str, cx: &mut CastCx) -> Outcome {
    match Action::from_signature(sig) {
        Some(a) => a.run(cx).map_or_else(Outcome::Mishap, |()| Outcome::Ok(a)),
        None => Outcome::Mishap(Mishap::UnknownPattern(sig.into())),
    }
}
```

The typed mishaps (`BadIota`, `NotEnoughArgs`) replace the per-pattern ones (`ExpectedEntity`,
`StackEmpty`, `NeedTwo`, `NothingToReveal`). This also matches upstream Hex Casting, which
words mishaps from the argument index and the expected type.

**Decided (D4, 2026-10-13): cross-file registration through a universal
`#[yog_api::register]`.** Actions live where they belong (`actions/math.rs`,
`actions/spells/*.rs`), one attributed function each; no central table to edit:

```rust
// casting/actions.rs: the collection, declared once by the mod
yog_api::collection!(pub ACTIONS: ActionDef);

// casting/actions/nav.rs
/// Compass' Purification: entity -> its position.
#[yog_api::register(ACTIONS, sig = "aa")]
fn compass_purification(cx: &mut CastCx, e: EntityRef) -> Result<Vec3, Mishap> {
    e.position(cx).ok_or(Mishap::EntityGone)
}
```

The attribute is **generic for any registrable item** and knows nothing about Hex: it works on
a `static`, a `const`, a `fn` or a type, and adds the item to a named, typed collection
(`collection!(NAME: T)`). For a `fn`, the extra attribute arguments and the function are
handed to `T`'s constructor (`impl Register<F> for ActionDef`, the same pattern as axum
handlers), so the typed-argument extraction (`FromIota`, `NotEnoughArgs`, stack restore,
`BadIota`), the `Action` enum, `signature()` / `from_signature()` and the `Tr` names are
mod-side code, generated from the collection by the mod's own `ActionDef` impl. Collection
happens **at build time**: `yog build` sees every `#[register]` in the crate and emits the
collection as a `&'static [T]` (and, for enum-like collections, a generated id enum and a
`match`-based lookup), so there is no runtime `ctor` and no per-load scan. The attribute is a
proc-macro in `yog-macros`, so mods don't need their own proc-macro crate (they still can't
have one, since `yog build` generates a single cdylib). The same attribute serves other
registrable things (screens, events, commands, recipes) wherever an explicit
`registry.xxx(...)` call is boilerplate; the explicit calls stay for run-time decisions
(capability rule). Until `#[register]` ships, the `actions!` table above is the mod-local
stop-gap, and it is written so each row moves 1:1 into an attributed fn.

**Savings.** `vm.rs::execute` 67 → ~8 lines. Tables and name `match`es: −40. Each future
action: ~1–4 lines instead of ~15–20.
**ABI/semver.** None (mod-local). It depends on F2 (`Codec`) and F6 (`Tr`) for the derives.
Until those exist, the macro can generate the `key`/`from_key` and `name()` bodies itself.

### F2. `#[derive(Codec)]`: one binary codec for packets, saves and interop

**Today.** There are three hand-written encoders, and a fourth is being written while this
audit runs:

- `Yog-Pipes/src/persist.rs:173-270`: `encode` / `decode` / `put_u32` / `put_bytes` /
  `put_pos` / `Reader` (~100 lines). It is a versioned length-prefixed binary format.
- `HexMod-Yog/src/protocol.rs:86-143`: `Mishap::encode` / `decode` and `Report::encode` /
  `decode`, tab- and newline-separated text (~60 lines).
- `HexMod-Yog/src/iota/mod.rs:95-155`: `Iota::serialize` / `deserialize`, text with
  prefixes (~60 lines). It breaks on strings that contain `,` or newlines inside lists.
- `HexMod-Yog/src/wire.rs` (new, untracked, 191 lines): a third, "self-delimiting" text
  codec that adds `Writer` / `Reader` to fix the previous one.

`yog-network` had `trait Packet` + `packet!{}` + `PacketField` — flat structs of
primitives only, so none of the mods used it. **Done (ABI 5.0):** they're gone; `Packet` is
implemented for every `yog_data::Schema` type, so a mod's packet is a Yog Data record
(enums, `Option`, `Vec<T>`, nesting included). `#[yog_export]` derives
rkyv for interop types, which is a third serialization mechanism.

**Proposal.** One trait and one derive in yog-api. The proc macro lives in `yog-interop`,
which should be renamed `yog-macros` since it hosts every derive.

```rust
pub trait Codec: Sized {
    fn encode(&self, w: &mut Writer);
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError>;
    fn to_bytes(&self) -> Vec<u8> { … }            // provided
    fn from_bytes(b: &[u8]) -> Result<Self, DecodeError> { … } // provided; rejects trailing bytes
}
// impls: bool, ints (varint), f32/f64, String, Vec<T>, Option<T>, Box<T>, tuples, arrays,
// HashMap/BTreeMap (sorted on write: equal state -> equal bytes), uuid::Uuid (feature)
#[derive(Codec)]                 // struct: fields in order. enum: varint tag + fields
#[codec(version = 1)]            // optional: leading version byte, checked on decode
```

Rules carried over from Pipes' hand-written code: every read checks the length first, and
an oversized length prefix is an error, never an allocation. Its tests
(`persist.rs:276-298`) become the tests of the derive.

After:

```rust
// Pipes: persist.rs: encode/decode/Reader/put_* (100 lines) are gone
#[derive(Codec, Default)] #[codec(version = 1)]
pub struct Snapshot { payloads: Vec<(DimPos, PipePayload)>, handlers: Vec<(DimPos, Binding)> }

// Hex: protocol.rs: 60 lines of encode/decode are gone; iota/mod.rs: serialize/deserialize gone;
// wire.rs is not needed
#[derive(Codec)] pub enum Iota { Boolean(bool), Double(f64), …, List(Vec<Iota>), Null }
#[derive(Codec)] pub struct Report { pub outcome: Outcome, pub stack: Vec<Iota> }
```

Item NBT and slates (the reason `wire.rs` uses text) can store `iota.to_bytes()` as a byte
array tag, or as base64 if an SNBT string is required.

**Decision for the user (D1).** What should the interop layer use? (a) Keep rkyv for
`#[yog_export]` and use `Codec` for packets and saves. (b) Move `#[yog_export]` onto `Codec`
and drop the rkyv dependency, so one format serves every purpose. (b) is simpler for mod
authors: one derive, and `#[yog_export]` structs can be sent and saved as they are. It
breaks the export ABI between mods, which is acceptable under the no-compat rule. The
recommendation is (b).

**Savings:** ~420 lines in mods now (including `wire.rs`). `packet!` / `PacketField` are
deleted. (Capability rule: `Codec` is a public trait, so a hand-written `impl Codec` remains the
way to keep any custom byte layout; `to_bytes`/`from_bytes` interoperate with raw `&[u8]` payloads.)
**ABI/semver:** yog-network and yog-api get a major bump. With (b), the interop wire format
changes and every mod that exports anything must be rebuilt.

### F3. Typed channels

**Today** (Hex `protocol.rs:17-20`, `events/mod.rs:39-56`, `casting_screen.rs:73,200-211`,
`hex_grid.rs:109`):

```rust
pub const CH_STACK: &str = "hexcasting:stack";
srv.send_to_player(player, CH_STACK, &report.encode());
registry.on_client_packet(CH_STACK, |ev, _srv| casting_screen::on_report(&ev.payload));
pub fn on_report(payload: &[u8]) { let Some(report) = Report::decode(payload) else { return }; … }
```

The direction (client to server, or back) appears only in the comments. Encoding and
decoding are done at every call site. Unit packets are sent as `b""`.

**Proposal.**

```rust
// yog-api
pub struct C2S<P>(&'static str, PhantomData<P>);   // client -> server
pub struct S2C<P>(&'static str, PhantomData<P>);   // server -> client
impl<P: Codec> S2C<P> { pub const fn new(id) -> Self; pub fn send(&self, srv: &dyn Server, to: &PlayerRef, p: &P); pub fn send_all(…) }
impl<P: Codec> C2S<P> { pub fn send(&self, p: &P) /* no-op without a client connection */ }
impl Registry {
    pub fn on<P: Codec>(&mut self, ch: C2S<P>, f: impl Fn(P, &PacketCx) + …);   // PacketCx: player(), server()
    pub fn on_client<P: Codec>(&mut self, ch: S2C<P>, f: impl Fn(P) + …);
}

// HexMod protocol.rs
pub const CAST:  C2S<HexPattern> = C2S::new("hexcasting:cast");
pub const RESET: C2S<()>         = C2S::new("hexcasting:reset");
pub const OPEN:  S2C<()>         = S2C::new("hexcasting:open_grid");
pub const STACK: S2C<Report>     = S2C::new("hexcasting:stack");

// events/mod.rs
registry.on(CAST,  |p, cx| vm::cast(cx.player(), &p, cx.server()));
registry.on(RESET, |(), cx| vm::reset(cx.player(), cx.server()));
registry.on_client(OPEN,  |()| casting_screen::open_session());
registry.on_client(STACK, casting_screen::on_report);   // fn on_report(report: Report)
```

`PacketCx::player()` returns a `PlayerRef { name, uuid }`. The runtime knows the sender's
`ServerPlayer`, so the uuid costs nothing to pass. That removes `NAME2UUID`,
`remember_player` and the `on_player_join` handler (`vm.rs:18-33`, `events/mod.rs:18-23`),
which exist only because `PacketEvent` (`yog-network/src/lib.rs:30`) carries the name and
nothing else.

`on_packet`, `on_client_packet`, `on_typed_packet` and `PacketEvent` are deleted from the
public API. **Every packet is typed; there is no separate raw API.** As in IP, where the
protocol number says how to read the payload, the channel id is the type tag and `P` is its
codec. Untyped bytes are just one more payload type: `yog_api::net::Raw` (a `Vec<u8>` whose
`Codec` is the identity), so `C2S<Raw>` / `S2C<Raw>` go through the same `registry.on(..)` /
`on_client(..)` and `send` as everything else. Capability rule: nothing is lost. A foreign
format (a non-Yog mod's channel, a vanilla custom payload) is either a hand-written
`impl Codec` (preferred: the layout is described once) or `Raw` on any channel id when the
mod really wants the bytes. `on_raw`, `on_client_raw` and the `&[u8]` senders are not kept.

**Savings:** ~25 lines now and ~6 per future packet. The class of bugs where a handler is
registered on the wrong side goes away.
**ABI:** a uuid field is added to the packet callback (`yog-abi` bump).

### F4. Ready-made state containers: `ServerState<T>`, `WorldData<T>`, `PlayerData<T>`

**Today.** The statics are shaped differently in each place:

```rust
// Yog-Pipes/src/handler.rs:18
static HANDLERS: LazyLock<Mutex<HashMap<NodeKey, (String, String)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
// graph.rs: its own `lock()` helper that ignores poisoning, reused by pipe.rs and handler.rs
// HexMod-Yog/src/casting/vm.rs:11-14
static STACKS: Mutex<Option<HashMap<String, Vec<Iota>>>> = Mutex::new(None);
static NAME2UUID: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
… .lock().unwrap().get_or_insert_with(HashMap::new) …
```

There are 9 such statics across the mods. Pipes then clears them by hand when the server
stops (`lib.rs:166-170`: `persist::unload(); graph::clear(); handler::clear();`). It
persists them with 370 lines (`persist.rs`), of which ~200 are mechanism:

- `find_world_dir` guesses the world as the newest `session.lock`, because "the yog-api
  exposes no world-save hook and no world directory" (`persist.rs:22-29`).
- Autosave counter, dirty check, unreadable-blob quarantine, format version,
  capture/restore glue.

HexMod keeps per-player casting stacks in memory only. They are lost on restart, keyed by
player *name*, and never cleared.

**Proposal** (yog-api, plus two runtime hooks):

```rust
/// Lives while a server runs; reset to Default when it stops. Poison-tolerant.
pub struct ServerState<T>;      // static GRAPH: ServerState<PipeGraph> = ServerState::new();
                                // GRAPH.with(|g| …), GRAPH.with_mut(|g| …)
/// Saved with the world (<world>/yog-data/<mod>/<name>.bin), loaded on server start,
/// written on world save + stop. Uses Codec with #[codec(version)]; an undecodable blob
/// is set aside as <name>.unreadable (Pipes' current policy, generalized).
pub struct WorldData<T: Codec + Default>;   // static NET: WorldData<Network> = WorldData::new("network");
/// Same, one value per player uuid, loaded on join, saved on leave/world save.
pub struct PlayerData<T: Codec + Default>;  // static STACK: PlayerData<Vec<Iota>> = PlayerData::new("stack");
                                            // STACK.with_mut(&cx.player(), |s| s.push(…))
```

Registration is automatic, with the same `ctor` registrar as `content!`. There is no
`registry.persist(&X)` call to forget. Runtime and ABI additions:

- `on_world_save`, which fires on vanilla's autosave and `/save-all`.
- `Server::world_dir()`.

Both make the guessing in `find_world_dir` unnecessary, along with its test.

After, Pipes `persist.rs` is deleted (370 → 0; tests move into yog-api), and the payloads and
bindings live directly in a `WorldData<Network>`:

```rust
#[derive(Codec, Default)] #[codec(version = 1)]
struct Network { payloads: BTreeMap<DimPos, PipePayload>, handlers: BTreeMap<DimPos, Binding> }
static NET: WorldData<Network> = WorldData::new("network");
static GRAPH: ServerState<PipeGraph> = ServerState::new();   // topology is rediscovered, not saved
```

`lib.rs` loses `on_server_started(persist::load)`, `on_server_stopping(...)` and
`on_tick(persist::tick)` (−12 lines). HexMod `vm.rs`: `STACKS` becomes
`PlayerData<Vec<Iota>>`, and `NAME2UUID` disappears together with F3.

**Savings:** ~300 lines in Pipes and ~20 in Hex. It also fixes Hex stacks not being saved,
and Pipes guessing the wrong world when several saves were opened.
**ABI:** two new hooks and one new `Server` method, so `yog-abi` gets a minor bump.

### F5. Content files: conventions and directory defaults

**Today.** `HexMod-Yog/content/` has 119 files (48 `.yblock`, 71 `.yitem`) and 2261 lines.
Most of each file restates the path:

```json
// content/items/staff/oak.yitem: the id, name, tooltip and texture all repeat the path
{ "id": "staff/oak", "name": "@res:Items.staff.oak", "tooltip": "@res:Items.staff.oak.desc",
  "item": { "stack_size": 1 },
  "model": { "parent": "handheld", "textures": { "layer0": "item/staff/oak.png" } } }
```

Block properties after removing `name`, `tooltip` and `model`, measured across the files:
12 files are exactly `{"item":{"stack_size":1}}`, 9 are the same stone block, 9 the same
wood block, 4 the same glass block, and so on. Of 119 models, 56 are `generated` and 27 are
`cube_all` with the texture at the conventional path.

The id defaults to the file stem (`content.rs:758`), not the path. That is why
`staff/oak.yitem` needs `"id"`.

**Proposal** (yog-cli content compiler only, no runtime change):

1. **The id is the path** under `content/items|blocks/`: `staff/oak`. `"id"` overrides it.
2. **`name` / `tooltip` default** to `@Items.<id with / → .>` / `….desc` (and `@Blocks…`)
   when that key exists in `resources/*.yres`. A missing key stays an error only if no
   literal name exists anywhere.
3. **The model defaults** by convention. An item gets `generated` with
   `item/<id>.png` if that texture exists. A block gets `cube_all` with `block/<id>.png`, or
   `cube_column` if `block/<id>_top.png` exists.
4. **Directory defaults:** `content/blocks/amethyst/_defaults.yblock` is deep-merged under
   every file in that directory and its subdirectories. `"extends": "../_stone.yblock"` is
   allowed for the cases where directory layout does not match. Only `name`, `tooltip`, the
   model and the differing properties remain in each file.

After:

```json
// content/items/staff/_defaults.yitem
{ "item": { "stack_size": 1 }, "model": { "parent": "handheld" } }
// content/items/staff/oak.yitem: one file per id (D5); an empty object is enough
{}
// content/items/staff/spruce.yitem, birch.yitem, …: likewise `{}`
```

**Savings:** ~1400 of 2261 JSON lines (estimate: about 70% of each file is restated path,
name and model). `yog import` should produce the short form.
**Coordination.** The agent designing Materials/Textures touches model and texture
resolution. Rule 3 (conventional texture paths) and the `_defaults` merge must be agreed
with that design, not done twice.
**Semver:** a yog-cli minor bump. Old files keep compiling, because explicit fields still
win.

### F6. `#[derive(Tr)]`: enum to localized text, checked at compile time; `yog_api::Text`

**Today.** Hand-written `match`es that map each variant to `Strings::<x>()`:

```rust
// Yog-Modlist/src/mod_list.rs:27-44: label() and badge(), two matches over the same enum
// HexMod-Yog/src/ui/casting_screen.rs:135-170: action_name, mishap_text, iota_kind (26 arms)
fn mishap_text(m: &Mishap) -> String { match m {
    Mishap::NoCaster => Strings::mishap_no_caster(),
    …
    Mishap::UnknownPattern(sig) => Strings::mishap_unknown_pattern(sig),
} }
```

In addition, the gameplay port is adding `HexMod-Yog/src/text.rs` (150 lines). It is a
server-built, client-worded translatable text (`Key(key, args)` / `Lit` / `Join`), that is,
Minecraft's `Component.translatable`. Any mod that sends messages needs this.

**Proposal.**

```rust
#[derive(Tr)]
#[tr(prefix = "mishap")]               // key = <prefix>.<variant in snake_case>
pub enum Mishap {
    NoCaster,                          // -> Strings::mishap_no_caster()
    UnknownPattern(String),            // tuple fields -> args: Strings::mishap_unknown_pattern(sig)
    #[tr(key = "number")] Double(f64), // override; #[tr(skip)] for no text
}
// generates: impl Mishap { pub fn tr(&self) -> String; pub fn text(&self) -> yog_api::Text }
// Several sets on one enum (Modlist ModSource has label + badge):
#[tr(fn label = "source", fn badge = "badge")]
```

The derive emits calls to the generated accessors in `crate::res::Strings` (configurable
with `#[tr(set = Book)]`). A variant without a `.yres` key is therefore a **compile error**.
Today it shows up as an untranslated key in game.

`yog_api::Text` (`Lit | Key(key, Vec<Text>) | Join`) is `Codec`. It provides `render()` on
the client through `tr` / `tr_args`, and `Server::send_message(player, Text)`, which the
Java side turns into a real `Component.translatable`. Hex `text.rs` is then deleted.

**Savings:** ~60 lines of `match` now, plus `text.rs` (150). Each new action or mishap is one
`.yres` line plus one variant, instead of three places.
**Semver:** yog-api minor (new). `send_message` adds a `yog-abi` entry.

### F7. Events: `on` (after the action) and `before` (can cancel)

**Today.** Every phased event takes `Fn(&E, EventPhase, &dyn Server) -> bool`, and fires
for both phases (`registry.rs:1680-1860`). All 5 handlers in the mods want Post only and
return `true`:

```rust
// Yog-Pipes/src/lib.rs:112-123 (and :125-135, :142-154; HexMod events/mod.rs:18-23, :28-36)
registry.on_block_break(|e, phase, _srv| {
    if phase == yog_api::EventPhase::Post && pipe::is_pipe_block(&e.block_id) {
        graph::on_removed(&(graph::normalize_dim(&e.dimension), e.pos.x, e.pos.y, e.pos.z));
    }
    true
});
```

**Proposal.** One generic pair replaces the 25 `on_xxx` methods. Each event type implements
`Event`, with an associated ABI slot.

```rust
registry.on::<BlockBreak>(|e, srv| …);                       // Post; no return value
registry.before::<BlockBreak>(|e, srv| if bad { Flow::Cancel } else { Flow::Continue });
// events carry a DimPos { dim: ResourceLocation, x, y, z } (Codec, Ord, Hash), normalized
registry.on::<BlockBreak>(|e, _| if pipe::is_pipe_block(&e.block_id) { graph::on_removed(&e.pos) });
```

`DimPos` replaces Pipes' `NodeKey` tuple, `PipePos` and `normalize_dim` calls (`transport.rs:13-28`,
`lib.rs:113-118, 127-132`).

**Decision (D2):** a generic `on::<E>()` (one method; the IDE completes over the `events::`
module) or named `after_block_break` / `before_block_break` (two × 25 methods, generated by
the same internal table, §3). The recommendation is generic.
**Savings:** ~25 lines in mods now. Internally it collapses `registry.rs`'s per-event
methods and trampolines (§3).
**ABI:** the runtime registers a phase-specific callback. `yog-abi` gets a major bump if the
slots change.

### F8. Mod entry point: `#[yog_api::main]`, screens that register themselves

**Today.** Each `lib.rs` has the same ritual (Modlist `lib.rs`, 33 lines; Hex `lib.rs`, 46
lines):

```rust
yog_api::ui_components!();
yog_api::resources!();
yog_api::content!();
pub struct HexMod;
impl Mod for HexMod { fn register(registry: &mut Registry) { info!("…initializing"); …; info!("…ready") } }
yog_api::export_mod!(HexMod);
// plus, per screen: mod_list::register(registry); casting_screen::register(registry);
// plus: registry.register_menu_entry(Strings::MENU_BUTTON_KEY, mod_list::ID);
```

If one of the macros is forgotten, the result is a confusing build or link error. All three
macros exist only to `include!` files that `yog build` already generates.

**Proposal.**

```rust
#[yog_api::main]
fn register(r: &mut Registry) {
    r.register_book(&book::hexbook());
    events::register(r);
}
```

`#[main]` expands to the struct, `impl Mod`, `export_mod!` and the three includes. `yog build`
always writes the three generated files, empty if there is nothing to include. The
`resources!()` / `ui_components!()` / `content!()` macros are deleted from the public API.
The scaffolding template (`yog-cli/src/main.rs:462`) shrinks to match.

Screens: the generated `register()` (`yui_vm.rs:1588`) also self-registers through the
`ctor` registrar, using `Default` for the view model. `register_with(vm)` remains for custom
construction. A `<Screen menu_entry="@res:Strings.menu_button">` attribute replaces
`register_menu_entry` as the default; `registry.register_menu_entry(label, ui_id)` stays for
entries decided at run time (conditional, computed label, a screen not declared in `.yui`)
(capability rule). The "initializing / ready" `info!` lines are dropped; the runtime
already logs each mod load.

**Savings:** ~15 lines per mod, plus 1–3 per screen.
**Semver:** a yog-api major bump. Moving `export_mod!` behind an attribute requires a
proc-macro, which goes into `yog-macros`.

### F9. `yog.toml` run matrix

**Today.** 18 `[run.<loader>-<mc>]` blocks of 7 lines each. Every mod has the same 6.
Only the loader, the MC version and the JDK path differ, and the JDK is determined by the
MC version.

**Proposal:**

```toml
[run.matrix]
loader_dir = "../Yog-Mod-Loader"           # export_dir/cwd/command derived per loader
loaders    = ["forge", "fabric", "neoforge"]
mc         = ["1.21.1", "1.20.1"]          # JAVA_HOME from <loader>/versions/<mc>.properties, like build.sh
```

This generates `forge-1.21.1` and the other targets. Explicit `[run.x]` blocks remain for
anything custom.
**Savings:** ~110 lines across 3 mods. **Semver:** yog-cli minor.

### F10. The book

**Today** (`HexMod-Yog/src/book.rs`, 124 lines). There are struct literals with six fields and
`..Default::default()`. Every page key is listed by hand (31 `page(Book::BOOK_ENTRY_…_PAGEn_KEY)`
lines). `priority` repeats the declaration order.

**Proposal.**

1. `.yres` codegen also emits arrays for numbered key families:
   `book.entry.media.page1..5` → `pub const BOOK_ENTRY_MEDIA_PAGES: &[&str]`.
2. Builders: `BookCategory::new(id, name).desc(..).icon(..)` and
   `BookEntry::new(id, name).icon(..).text_pages(KEYS)`. `add_category` / `add_entry`
   assign `sortnum` / `priority` from insertion order unless set explicitly.

```rust
Book::new("hexcasting:thehexbook", B::BOOK_NAME_KEY).author("F000NK, Yog Team").i18n(true)
    .category(BookCategory::new("hexcasting:basics", B::BOOK_CATEGORY_BASICS_KEY)
        .desc(B::BOOK_CATEGORY_BASICS_DESC_KEY).icon("hexcasting:staff/oak")
        .entry(BookEntry::new("hexcasting:media", B::BOOK_ENTRY_MEDIA_KEY)
            .icon("hexcasting:amethyst_dust").text_pages(B::BOOK_ENTRY_MEDIA_PAGES))
        …)
```

**Decision (D3):** builders (124 → ~40 lines of Rust), or a `content/books/*.ybook` file
compiled like `.yblock` (0 Rust, and the IDE could edit it). The recommendation is builders
plus key arrays now, and `.ybook` once Yog-IDLE has a book editor.
**Semver:** yog-book minor (additive builders). Removing the public struct literals would be
a major bump; do that. (Capability rule: whatever D3 makes the authoring path, the builders stay
public, so a mod can still build or extend a book in code at run time, e.g. from config or
loaded content.)

### F11. Yog-Pipes duplicates yog-api

- **`recipe.rs` (110 lines)** mirrors `ShapedRecipe` / `ShapelessRecipe` / `FurnaceRecipe` as
  `RecipeDef` because API types cannot cross interop. Fix: a single
  `#[derive(Codec)] pub enum yog_api::Recipe { Shaped(..), Shapeless(..), Smelting(..) }`
  with `registry.add_recipe(r)`. It generates the id (`<output>_<kind>[_n]`, the policy now
  hand-written at `recipe.rs:40-55`). `PipeDef.recipe: Vec<yog_api::Recipe>`. That saves
  ~100 lines and deletes `add_shaped_recipe` / `add_shapeless_recipe` / `add_furnace_recipe`.
  (Capability rule: `Recipe` covers every type `.yrecipe` does (F13), plus
  `Recipe::Json { id, json }` passed through to the host's existing `register_recipe_json`, so
  code can register any recipe type, including other mods' types; today only the three typed
  helpers are public.)
- **`handler.rs::dispatch` (`handler.rs:58-97`)** transmutes an interop pointer to the C-ABI
  wrapper signature and frees the output buffer by hand. Fix:
  `Interop::call::<In: Codec, Out: Codec>(mod_id, symbol, &input) -> Option<Out>` in
  yog-api. That saves ~30 lines of `unsafe` from a mod, and every mod that binds callbacks
  by name needs it.
- **`PipeDef` literals**: each takes ~12 lines, with `model: None, recipe: vec![]` repeated
  (`examples/standard_pipes.rs`, 8×). Add `Default` + builders:
  `PipeDef::new("mymod:pipe_iron", PipeKind::Item).groups(["pipe_item", "inventory"]).prop("speed", 2.0)`.
- **`build_tooltip`** (`pipe.rs:111-152`): 7 `if let` blocks repeat the list of known keys.
  Replace with one `const KNOWN: [(&str, &str /*fmt*/)]` table, saving ~20 lines (local
  refactor).
- **`ModelDef`**: see the dead-code note in §0. Replace it with the content compiler's block
  model type, `yog_api::BlockModel`, coordinated with the Materials/Textures design.

### F12. View-model glue and tests

- `if let Some(h) = handle() { h.update(f) }` appears 3× (`casting_screen.rs:177-200`).
  Generate `pub fn update(f)` next to `handle()` (a no-op before registration).
- Trivial getters exist only to feed bindings: `stack_rows()`, `patterns()`, `has_status()`,
  `status_text()` and `status_color()` (`casting_screen.rs:53-67`). Generated code is
  `include!`d into the view model's module, so `{x:Bind stack}` can bind the private field
  directly. Store `status: Option<Status { text, color }>` and bind
  `{x:Bind status?.text}`; the binding compiler already supports `?`, see `Ex::has_opt`.
  Make sure `ListView items` accepts `&Vec<T>` and document "bind fields, not getters".
  Saves ~15 lines.
- Tests: `TestHost::<View>::new(vm); h.size(427.0, 240.0).open();` appears in both mods.
  427×240 is already the default (`yog-kit/src/testing.rs:28`). Add `TestHost::opened(vm)`
  (or a generated `View::test(vm)`). Pipes' `scratch_dir` belongs in yog-api's
  `testing::TempDir`.
- `GridSurface::shared()` (`hex_grid.rs:104-117`): `OnceLock` + `surface::register`. Add
  `yog_api::ui::surface::shared::<T>(id, init) -> Arc<T>`. Small.

### F13. Declarative recipes (`.yrecipe`) and the registration snapshot

Recipes are the next thing every content mod writes in Rust (Pipes' `recipe.rs`, the copper-pipes example).
Raw `data/` JSON is no replacement: the format differs between MC 1.20 and 1.21 (`result.item` vs `result.id`,
`recipe/` vs `recipes/`).

- **`content/recipes/**/*.yrecipe`**, JSON with comments like `.yblock`. Types: `shaped`, `shapeless`, the
  cooking family (`smelting`, `blasting`, `smoking`, `campfire`), `stonecutting`, `smithing`, plus any
  `"type": "<ns>:<id>"` passed through. Ids always carry a namespace. The recipe id is `<modid>:<file stem>`.
  `"result": "<ns>:<id>"` or `{ "item", "count" }`; `"key"` and ingredients accept items and `#tags`.
  Conditions reuse the content `"mc"` / `"mod"` conditions.
- **Generator, not code.** `yog build` writes the recipe JSON for every targeted MC version into the `.yog`
  (`data/<ns>/recipe(s)/…` in each version's format). No Rust, no `register_recipe_json` call. Ids of items and
  tags are checked against the mod, its dependencies and the vanilla packs. `recipes.g.rs` gives typed handles
  (`recipes::COPPER_PIPE`) for code and book references.
- **Registration snapshot** for what code still registers (generated or truly dynamic recipes, and items, blocks,
  tags registered in code). `yog build` loads the built mod library and calls its `register` against a
  *recording* registry: nothing is registered, every call is recorded, host queries (config, loaded mods, MC
  version) answer with defaults for the targeted version. The result goes to `.yog/registry.json`. It is generic
  loader infrastructure (yog-runtime's registry recorder), with no knowledge of any mod.
  - Users: the book designer's recipe pages (ybook Q7), build-time validation of every id reference (books,
    `.yui`, `.yrecipe`), IDE autocomplete and the content explorer's *Generated* section.
  - A registration that depends on world state can't be captured; such ids are marked "computed in game".
- **Static analyzer as a fallback.** When the mod library doesn't build yet, `yog check` scans `src/` with `syn`
  for registration calls with literal ids (`add_shaped_recipe`, `recipe(ns, name, …)`, item/block defs) and
  fills the same `registry.json` with ids only, marked `static`.
- Deletes from mods: Pipes' consumer recipe code, `registry.add_*_recipe` calls for static recipes.
  `registry.add_recipe(Recipe)` (F11) stays for recipes built at run time (capability rule).

---

## 2. Public API and codegen: summary of the changes

| Crate | Add | Delete (no shims) |
|---|---|---|
| `yog-macros` (rename of `yog-interop`) | `#[derive(Codec)]` (incl. `#[codec(zero_copy)]` → `View<'a>`), `#[derive(Tr)]`, `#[main]`, `#[register]` + `collection!` (build-time collected, D4), `#[yog_export]` on top of `Codec` | rkyv derives (if D1 = b) |
| `yog-api` | `Codec` / `Writer` / `Reader`, `C2S` / `S2C` + `registry.on(..)`, `net::Raw`, `PacketCx` / `PlayerRef`, `ServerState` / `WorldData` / `PlayerData`, `Text`, `DimPos`, `Recipe` (incl. `Recipe::Json`), `Interop::call`, `registry.on::<E>` / `before::<E>`, `testing::TempDir` | `on_packet` / `on_client_packet` / `on_typed_packet`, raw `&[u8]` senders, 25 × `on_xxx`, `add_*_recipe`, `resources!` / `ui_components!` / `content!` (folded into `#[main]`) |
| `yog-network` | — | `packet!`, `PacketField`, `Packet` (replaced by `Codec`) |
| `yog-book` | builders, auto sort order | public struct-literal construction |
| `yog-kit` | generated `update()`, self-registering screens, `menu_entry=`, `TestHost::opened` | explicit `screen::register` calls in mods (`register_with(vm)` and `register_menu_entry` stay for run-time cases) |
| `yog-cli` | content conventions + `_defaults` / `extends`, numbered `.yres` arrays, `[run.matrix]`, an always-present set of generated files | — |
| `yog-abi` / runtime | uuid on packet callbacks, `on_world_save`, `Server::world_dir`, `send_message(Text)`, phase-specific event registration | — |

---

## 3. Internal loader code

Mods never see this code. It is still where the cost of adding a feature lives: every new
event, host call or block property has to cross all of these layers by hand. Counts come
from `grep`/`md5sum` on the current tree.

### I1. Event pipeline: one event is hand-written in ~7 layers (worst)

Here is how `container_open` is wired today:

1. `yog-event/src/events.rs:251`: the owned struct. There are 26.
2. `yog-abi/src/lib.rs`: the `#[repr(C)] YogContainerOpenEvent` (:288), the Fn type (:485) and
   the `YogApi.on_container_open` field (:1166).
3. `yog-runtime/src/lib.rs`, 7 sites:
   - the import (:31);
   - the `RuntimeHandlers` field (:148). There are 33 such `Vec<(LoadedModule, *mut c_void, FnTy)>` fields.
   - its init (:222);
   - the unload `retain` (:353), which is copied 33×;
   - `api_event!` (:2789);
   - the table entry (:3704);
   - two JNI functions `nativeOnContainerOpenPre` / `nativeOnContainerOpen` (:6207-6296).
4. `yog-api/src/registry.rs`:
   - imports;
   - `trampoline_phased!` with a field-by-field closure (22 invocations, 88 `.as_str().to_owned()`);
   - `on_container_open` (37 near-identical `on_*` methods).
5. Java: the `native` declarations in 6 `NativeBridge.java`, 6 `ContainerOpenMixin.java` and 6
   `yog.mixins.json`.

The JNI layer is the bulk. There are 47 `nativeOn*` functions, 1,925 lines. The dispatch loop
`if module.is_retiring() {continue}; let Some(_call) = module.enter() …; f(ud, srv, &ev, phase)`
is copied 50× in `lib.rs` and 8× in `ui_jni.rs`. Each of the 13 Pre/Post pairs is two
near-identical functions.

**Cost of one new event today:** ~24 files and ~30 edit sites (~150-200 Rust lines + 18 Java
files).

**Proposal:** one declarative table, a `macro_rules!` exported from `yog-abi` (or build.rs
codegen):

```rust
yog_events! {
    container_open: phased ContainerOpen { player: Str, player_uuid: Str, container_type: Str, dimension: DimPos },
    server_tick:    plain  ServerTick {},
    …
}
```

It generates:

- the ABI struct, the Fn type and the `YogApi` slot (append-only order);
- the owned struct + `From<&Abi>` (replacing the trampoline closures);
- `impl Event` for the public `registry.on::<E>()` / `before::<E>()` (F7);
- runtime storage `[Vec<Entry>; EVENT_COUNT]` indexed by `EventKind`, so one loop replaces 33
  fields, 33 inits and 33 retains;
- one generic `dispatch(kind, &ev, phase)`;
- `Java_…_nativeOnX` shims via a `FromJni` trait per field type, one native with a `phase`
  argument instead of the Pre/Post pairs;
- a single `YogNatives.java` in `java-common`. The `native` declarations use only `String`
  and primitives, so they are mapping-neutral.

**Saves:** ~2,600 → ~450 Rust lines. One new event becomes one table row + the per-loader mixins.

### I2. `YogServer` host calls: 85 functions × 4 Rust layers × 6 Java copies

The same 85 calls are written out at each layer:

- `yog-abi` (:726-1091): 85 `extern "C" fn` fields.
- `yog-core/src/server.rs`: the `Server` trait (84 fns).
- `yog-api/src/registry.rs:52`: `impl Server for CServer`, 1,022 lines. It has 84× `let s = srv!(self)`.
  The OwnedStr→String→`free_str` sequence is hand-copied 13×.
- `yog-runtime`: 85 `srv_*` functions, 1,671 lines. Each one calls
  `call_static_method("dev/yog/NativeBridge", "worldSetWeather", "(Ljava/lang/String;ZI)Z", …)`.
  The JNI descriptors are hand-typed 80×, so a typo is only caught at runtime. After that,
  `build_server_table()` assigns all 85 again.
- Java: the matching `public static` targets in 6 `NativeBridge.java`.

**Proposal:**

```rust
server_api! { fn set_weather(dim: &str, raining: bool, dur: i32) -> bool => "worldSetWeather"; … }
```

It generates the trait method, the ABI field, the `CServer` impl, the runtime `srv_*` function
and the table init. The JNI descriptor is built from a `JniType { const SIG: &str }` trait.

**Saves:** ~2,900 → ~500 lines. The same macro with a `"dev/yog/NativeDraw"` target covers
`YogGfxApi` (35 functions × 3 layers, ~1,700 → ~600).

### I3. Java hosts: 6 near-copies of every class

`{fabric,forge,neoforge}/platforms/{1.20.1,1.21.1}` holds 166 files, ~3.5k lines each, while
`java-common` has only 977 lines. Duplication:

- forge and neoforge: 115 files, of which only 68 are unique. 83 files sit in byte-identical groups.
- 3,231 lines across platforms are exact copies.
- Mixins: 10 of 10 have only 1-2 distinct variants across 4 copies.
- fabric differs mostly because it uses Yarn mappings.

**Proposal, in increasing cost:**

- (a) A shared `mojmap-common/` source dir for forge+neoforge, with per-MC-version overlays.
  It is pure file moves and saves ~1,700 lines immediately.
- (b) Move the `native` declarations and mapping-free logic (prop parsing, pack generation,
  chunk-gen glue) into `java-common` behind a small per-loader `Platform` interface.
- (c) Optionally switch fabric to `loom.officialMojangMappings()`, so all 6 share one tree.

**Decision (D7):** (c) reverses the deliberate Yarn choice. **Saves:** ~9-12k of ~20.8k
platform lines.

### I4. Block/Item/Inventory defs marshalled 5 times

One block field (`slipperiness`) is written at all of these sites:

- the `yog-registry` struct field + default + builder method (32 identical `mut self … self`
  setters);
- the `YogBlockDef` ABI field;
- Rust→ABI in `registry.rs:2027` (`Option<String>` ↔ `YogStr::EMPTY` sentinels);
- ABI→Rust in the runtime (:2913);
- `key=value` text encoding (:5216, 26 `parts.push`);
- 6 Java `YogHost.java` parsers;
- 3 places in `yog-cli/src/content.rs` (allowed keys, parse, codegen).

**Cost of one new field:** ~10 Rust sites in 5 files + 6 Java files. The string key is retyped
~10× with no compile-time check.

**Proposal:** drop the C mirror structs. Pass a def over the ABI as one serialized `YogStr`;
the runtime already re-serializes defs to text for Java. Generate the builder and the encoder
with `#[derive(YogDef)]` (`#[def(default = 1.5, key = "slipperiness")]`), and parse it with one
generated `BlockProps` record in `java-common`.

**Saves:** ~400 Rust + ~600 Java lines. One new field becomes ~3 sites.

### I5. Small items

- `jstr!` is defined twice (`yog-runtime/src/lib.rs:406`, `ui_jni.rs:10`).
- 66 manual `match env.get_string(..)` blocks sit next to 61 uses of `jstr!`.
- `catch_unwind` is used directly 17× next to the existing `guard()` helper.
- 18 `api_*` functions repeat the `RwLock<RuntimeHandlers>` cast-and-lock. A
  `with_handlers_mut(ctx, |h| …)` helper saves ~100 lines.

**Internal order:**

1. Java forge/neoforge dedup (zero risk, biggest line count).
2. The `yog_events!` table, done together with F7.
3. `server_api!` (+ Gfx).
4. Defs over a serialized string.

All of these bump `ABI_VERSION`, which is acceptable: hot reload already requires an exact ABI
match.

---

## 4. Staged plan

Each stage can ship on its own. The arrows are hard dependencies.

**Stage 0: mod-local, no loader release (can start now).**
- Hex: the `actions!` table macro (F1) with hand-generated `key` / `name`, as the stop-gap
  until `#[yog_api::register]` (D4, Stage 4 with `#[main]`) lands; then each row becomes
  an attributed fn. Delete the dead
  scaffolding (iota registry, `ActionRegistry`, `casting::Mishap`). The gameplay agent owns
  these files, so hand them the design rather than editing.
- Pipes: the `build_tooltip` table, a `PipeDef` builder.

**Stage 1: `Codec` (F2) → typed channels (F3) → `Text` + `#[derive(Tr)]` (F6).**
- yog-macros: the `Codec` derive, with a test corpus taken from Pipes `persist.rs` tests.
- yog-api: `C2S` / `S2C`, `PacketCx` with `PlayerRef` (ABI: uuid on packets), `Text`, `Tr`.
- Migrate Hex `protocol.rs`, `iota`, `wire.rs`, `text.rs`, and Modlist `ModSource`.
- Decide D1 here: whether interop moves to Codec.

**Stage 2: state containers (F4).** Depends on Stage 1 (`Codec`).
- Runtime: `on_world_save`, `Server::world_dir`. yog-api: `ServerState`, `WorldData`,
  `PlayerData` (ctor registrar).
- Migrate Pipes `persist.rs`, `graph.rs`, `handler.rs`, and Hex `vm.rs` stacks.

**Stage 3: event API + `DimPos` (F7) and the internal event table (§3).** Independent of
Stages 1–2. It is best done together with the internal table macro, because the same table
generates both the public `Event` impls and the runtime/ABI glue.

**Stage 4: entry point and screens (F8), `#[register]` + `collection!` (D4), plus F12.** Depends on the `yog-macros` rename from
Stage 1. Touches the yog-cli templates and Yog-IDLE scaffolding (same template source).

**Stage 5: yog-cli data work, independent of everything else.**
- Content conventions and `_defaults` (F5), after agreeing with the Materials/Textures design.
- Numbered `.yres` arrays + book builders (F10). `[run.matrix]` (F9).

**Stage 3a (internal, any time): Java forge/neoforge dedup (I3 a), then `server_api!` + Gfx (I2) and defs as a serialized string (I4).** No mod-visible change apart from the ABI bump.

**Stage 6: Pipes API cleanup (F11).** Depends on Stage 1 (`Recipe: Codec`, `Interop::call`).

Each stage ends with: bump the crates involved (breaking, as policy allows), publish, run
`dev.sh` with the user's permission, and update the three mods in the same change.

---

## 5. Decisions for the user

- **D1 — decided (2026-10-13): yes; revised the same day (capability rule).** Interop
  serialization (`#[yog_export]`) moves onto `Codec`: one derive and one format for interop,
  network and saves, and rkyv is no longer something mods see or depend on. **Zero-copy access
  is kept** as a `Codec` mode, not dropped with rkyv (user: keep every capability, as fast as
  possible):
  - `#[derive(Codec)] #[codec(zero_copy)]` additionally generates `T::View<'a>`: a validated,
    aligned view over `&'a [u8]` with lazy field getters (`view.items().get(i)`, `&str`/`&[u8]`
    borrowed, no allocation). It is accepted wherever `T` is: `#[yog_export]` parameters
    (`fn f(big: View<'_, Chunk>)`), packets (`registry.on(CH, |v: View<Report>, cx| …)`),
    storage reads. Validation runs once per buffer; access after that is pointer arithmetic.
  - Plain `Codec` decoding also avoids copies where it can: borrowed decode
    (`Decode<'a>` for `&'a str`, `&'a [u8]`, `Cow`), length-prefixed bulk copies for
    `Vec<primitive>`, no per-field allocation, and `encode_into(&mut Vec<u8>)` reusing buffers.
  - The interop call path passes the encoded buffer straight across the C ABI (no re-copy
    into an aligned `Vec` as `handler.rs` does today); the callee decodes or views it in place.
  - Whether `zero_copy` is implemented on top of rkyv internally or with an own layout is an
    implementation detail of `yog-macros`; the public surface is `Codec` + `View`.
- **D2 — decided (2026-10-13): generic.** Events are `registry.on::<BlockBreak>()` /
  `before::<…>()`; the named `after_x` / `before_x` forms are not added.
- **D3 — decided (2026-10-13): a `.ybook` file, now.** The book is its own content file
  with a WYSIWYG designer in Yog-IDLE (like the `.yui` screen designer) that shows the
  rendered book while editing; the Rust builders are not the authoring path.
- **D4 — decided (2026-10-13): universal `#[yog_api::register]`.** A generic attribute for
  any registrable item (static / const / fn / type) that adds it to a typed
  `collection!(NAME: T)`, collected at build time by `yog build` into a `&'static [T]`;
  it lives in `yog-macros`, so mods need no proc-macro crate. Nothing Hex-specific in the
  loader: HexMod's `ActionDef` decides what an action is. See F1. Was: a `macro_rules!`
  table inside the mod, or a loader-level `#[collect]` + attribute form? Recommended: the
  table now.
- **D5 — decided (2026-10-13): one source file per id; no `_list`/`variants` files.** Variations
  of one block or item (same id, differing by block state / properties, e.g. a `color` or `facing`
  property) live in that one file, as `states` already do. Entries with different ids always get
  their own file, so each id has one place for its settings, renames and references. An entry
  whose data all comes from `_defaults` / `based_on` may be an empty `{}`. Merging entries
  into lists is done only in the build output (the `.yog`), never in the sources. Was:
  Content conventions: may an item or block file become empty, or disappear in
  favour of `_list` files? Or must every item still have its own file, as now, for the
  IDE's tree view? Recommended: keep one file per entry (an empty `{}` is fine), with
  directory `_defaults`.
- **D6 — decided (2026-10-13): automatic, with an explicit stable key.** Declaring
  `static X: WorldData<T> = WorldData::new("key")` is all a mod does: it registers itself and is
  loaded/saved with the world; there is no `registry.persist`. The key is mandatory (it names
  the save entry, so renaming the Rust item doesn't lose data); `T: Codec + Default`. A
  `X.mark_dirty()`-free model: mutation goes through `X.update(|t| …)`, which marks dirty.
- **D7 — decided (2026-10-13): yes, Fabric moves to official Mojang mappings**
  (`loom.officialMojangMappings()`), so all Java hosts share one source tree. Safe for the
  future: the built jar is still remapped to intermediary, Fabric officially supports Mojmap,
  and Mojang is dropping obfuscation from Java Edition, after which Mojmap names are simply
  the game's real names. The mappings themselves are not redistributed (Mojang's licence).