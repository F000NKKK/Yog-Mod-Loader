# Design: declared state — `saved`, `synced`, storage scopes

Status: design only. Builds on `yog-storage` (scoped `.ydata` stores,
implemented), F3 mirrors (`yog-mirror`, implemented), and ys.md §4.2
(the language side). Scope: `yog-event` (a storage domain),
`yog-runtime`, `java-api`, `yog-script-*`, the hosts (item stack data).

## 0. Decisions

| # | Decision | Why |
|---|---|---|
| S1 | **Scopes as web storage** (user, 2026-10-21): `session` (while the game runs), `game` (the install), `world`, and the world's parts: `dimension`, `player`, `entity`, `chunk`. A block's state is its chunk's. An item's state is its stack's. | One mental model, and one per place state really lives. |
| S2 | **State is declared** (ys.md principle 4): `saved` writes it to its owner's scope, `synced` mirrors it to clients that see the owner. Codecs, keys, packets and mirrors are generated. A mod never names a file, a key or a packet. | The language's reason to exist. |
| S3 | **Yog Data only**: a saved or synced member's type must be Data-able: numbers, `bool`, `string`, `Text`, quantities, enums, records, lists, dictionaries of those, ids, positions, `Player?` (as its uuid). Anything else is a build error, YS0040. | One format everywhere (D1); binary, versioned, small. |
| S4 | **One store per mod and scope instance**, which yog-storage already has. In it, an owner's state is one record **per saved member**, keyed `Owner.Member` (`Player.Visits`, a component's `Charge.Energy`) under the owner. | An `extend`'s unnamed component has no name that survives edits (its place in its file moves); a member's does, and adding or removing one never rewrites another's data. A removed member's record is dropped on load, with a warning. |
| S5 | **Migrations in the declaration**: `saved Energy Energy = 0 was Power: i32;` (ys.md §4.2). The generated reader tries the field, then each `was`, then the default. | Old saves load. No migration scripts. |
| S6 | **The JVM keeps live state; the native keeps storage.** YS objects hold their fields on the JVM. They are read in when their owner loads and written out when the world saves, over a storage domain on the bus. | Reads and writes are plain field accesses: nothing crosses per access. |
| S7 | **`synced` is a mirror** (F3). Each type with synced members gets a mirror spec, generated from its declaration. The server provides it, and a client's reads of the member read the mirror's column. | Clients never send a message per read; the rate and the scope are the mirror's. |
| S8 | **Hot reload keeps state**: before the old classes go, live objects write their state to the `session` scope, and the new ones read it back. | ys.md principle 6. |

## 1. Where each owner's state lives

| Owner (YS) | Scope | Store (`yog-storage`) | Key in it | Loaded / written |
|---|---|---|---|---|
| `block X` object, `extend Block` | chunk | `chunk/<dim>_<cx>_<cz>` | the position in the chunk, then the type | with the chunk / as the world saves; removed with the block |
| `entity X`, `extend Entity` | entity | `entity/<uuid>` | the type | as the entity loads / saves; removed when it dies for good |
| `extend Player` | player | `player/<uuid>` | the type | at join / at save and leave |
| `extend World` | world | `world` | the type | at world load / save |
| `extend Dimension` | dimension | `dimension/<dim>` | the type | as the dimension loads / saves |
| `item X` object, `extend ItemStack` | the stack | the stack's own data (host: 1.20.1 NBT `yog`, 1.21+ `minecraft:custom_data`) | `<mod>:<type>` | as the stack is read; written as it changes |
| `mod X` members marked `saved` | game | `game` | the type | at mod load / game exit |
| any `session` member | session | memory (`Scope::Session`) | the type | never written |

The words come from `yog_ids::storage` (`DIR`, `GAME`, `WORLD`, …), as
yog-storage already uses them. A type's key is its full YS name
(`charge.Charger`). The build reports two types whose names collide.

## 2. The storage domain on the bus

New services in `yog-event` (`planes/system/storage.rs`), **keyed by the
mod**. Each mod's own native answers for itself: every mod has one (the
generated crate root of ys-bindings), and yog-api already keeps the mod's
stores, knows the world's folder and saves with it
(`yog_api::storage::__provide_records`). A record is kept as its bytes
(`Value::Bytes` in the store), neither decoded when it's written nor
encoded when it's read. Its owner's codec, on the JVM, is the only one
that reads it. A rewrite reuses the record's buffer, and an owner's
records go in one pass (`Store::remove_under`). Bench:
`yog-storage/benches/records.rs`.

| Service | Request | Answer |
|---|---|---|
| `yog:storage/read` | scope, owner key, type | the record's bytes, if any |
| `yog:storage/write` | scope, owner key, type, bytes | — |
| `yog:storage/remove` | scope, owner key (all types, or one) | — |
| `yog:storage/keys` | scope | the owners with records (a chunk's positions) |

`flush_all` stays where it is, as the world saves. The hosts send the
existing world save event, and the YS runtime writes every dirty live
object **before** the runtime flushes. Item stacks don't use the domain:
their data is the host's `yog:item/data` get/set services, because it
travels with the stack.

## 3. What the compiler generates

For each type with `saved` members, the generated Java has:

- a `$State` record of the saved members, with its Yog Data codec
  (`YogBus.bytes`/`fromBytes`, already used by messages), and a reader
  that applies `was`;
- `$load(bytes)` and `$save()`, called by the runtime;
- a dirty bit, set by every write to a saved member. Writes are known
  statically, the same write-set analysis as screens use
  (ys-bindings §2.2). An object that wasn't written isn't saved again.
  **Deferred**: a change inside a member's value (`Spots.Add(…)`) isn't a
  write the analysis sees, so until values track their own changes every
  live object is written as the world saves (correct, not minimal).

The runtime (`dev.yog.script.yog.State`) keeps each scope's live objects
by owner key. On an owner's load event it reads the records and makes the
objects. On save it writes the dirty ones. On unload it saves the dirty
ones and drops them all.

A saved `async` method's task is saved with its object: its state
machine's fields join the record, and a delay in progress restarts with
the time left. This is S9; it waits for the rest of this design.

## 4. `synced`

For each type with `synced` members, the build generates a mirror spec
(`MirrorSpec`): one column per member, and the owner's key space
(`yog:keys/block/<dim>` for blocks, entity ids for entities, players for
`extend Player`). The rules:

- **Server.** A write to a synced member writes its column. The host's
  mirror sync sends changed rows to the clients that track the owner.
- **Client.** Reading the member reads the column. A client never writes
  it: the analyzer reports such a write as WRONG_SIDE.
- **Screens.** A screen's binding to a synced member goes through the
  same column. Container screens need no `Synced<T>` of their own (mvvm-v2
  V13).

## 5. Checks

- **YS0040**: a saved or synced member whose type isn't Data-able (S3).
- **YS0041**: `was` naming a type the old value can't be read as.
- A `saved` member of a type with no owner scope (a plain class) is an
  error: state needs an owner.
- `synced` on client-only code is an error: the server owns state.

## 6. Steps

1. ✅ (not yet built) The storage domain in yog-event (`Where`, `Read`,
   `Write`, `Remove`, `Owners`). Each mod's native answers it over
   yog-storage records (`record`, `set_record`, `remove_under`), with
   tests and a bench.
2. `saved` on `extend Player`/`World` and on `mod` members. The analyzer
   checks S3 (YS0048) and the owner (YS0049). The Java side generates a
   record per member, `$load`/`$save`/`$forget`, and `State.java` reads
   an owner's records as its object is made, writes them as the world
   saves, and drops a player's as they leave. ✅ `extend` (compiled to
   Java), `extend Player`/`World` with members the bus's codec keeps as
   they are (numbers, `bool`, `string`, quantities, ids, a player), `T?`
   (as an `Optional`), `List<T>` of those, enums without data (by their
   number), records of what's kept as it is, `Dictionary<K, V>` of those
   (a list of its entries), enums with data (their case by name, its data
   a record), `Text` as its own Yog Data (its key, plainness and
   arguments: numbers, `bool`s, strings and texts as they are, anything
   else as its words). ✅ `mod` members: the mod's class reads them from the
   game's store at the end of its constructors.
3. Blocks (the chunk scope, removal with the block) and entities.
   ✅ Blocks: a block's object reads its `saved` members from its chunk's
   store (`<dim>_<cx>_<cz>`, under its place there, `<x>_<y>_<z>`) as
   BlockObject makes it; its records go when the block is broken or
   another is placed there, its object made or not. Entities wait for
   entity objects (content of that kind has none yet).
4. Item stacks: the host's `yog:item/data` on each version.
5. `synced` on mirrors.
6. `was` migrations, hot reload through `session` (S8), saved tasks (S9).
7. The Rust API's parity: `#[saved]` and `#[synced]` on a component
   struct, made from the same model.
