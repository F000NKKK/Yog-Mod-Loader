# Yog Data: one language for every format — audit and plan

Status: proposal (audit of 2026-10-14). Nothing here is built yet. The project
has no backward compatibility: every format and API below is replaced, not
deprecated.

The goal, in the user's words: analyse every format and make one universal
language, the way Minecraft has NBT and Office has ODF; check and clean the
whole loader, every API, Yog-IDLE and Substrate; cover them with tests and
benchmarks **before** reorganising the APIs further.

---

## 1. What exists today

### 1.1 Formats

| Format | Syntax | Parsed by | Validated by | Crosses the ABI as |
|---|---|---|---|---|
| `yog.toml` | TOML subset | hand-written line parser (yog-cli `manifest.rs`), Yog-IDLE (`toml` crate + its own reader), Substrate (`workflow.toml`, `standards.toml`) | ad hoc, per field | — (build time) |
| `.yres` | JSON, metadata keys | yog-cli `yres.rs`, Yog-IDLE (`book.rs`, `designResources.ts`, `YresEditor`) | yog-cli | lang files (JSON per locale) |
| `.yblock` / `.yitem` | JSON | yog-cli `content.rs` (own `json.rs` with lines), Yog-IDLE `content.rs` + `ContentEditor` | yog-cli | Rust code calling `register_*` with C structs |
| `.yrecipe` | JSON | yog-cli, yog-book (`parse_recipe`), Java `YogDataLayout`, Yog-IDLE `recipeModel.ts` | yog-cli | JSON string (`register_recipe_json`) |
| `.ybook` | JSON | yog-cli, yog-book (serde), Yog-IDLE (`book.rs`, `bookModel.ts`) | yog-cli + `yog_book::refs` | JSON string (`register_book`) |
| `.yui` | XAML-like XML | yog-cli `yui*.rs` (codegen), Yog-IDLE `yui.ts` + `markup.rs` (designer) | yog-cli | Rust code (generated) |
| dimensions | JSON | yog-dimensions (serde), Java host | serde only | JSON string + TSV lines |
| Yog text | `{b|…}` spans | yog-ui `rich.rs`, Java `YogText.java`, Yog-IDLE `font.rs` | — | inside every string |
| References | `@Set.key`, `@item:…`, `@@`, `item:`/`sprite:`/`mod:` image sources, `#tag`, `{x:Bind …}` | yog-cli (3 places), yog-book `refs`, yog-gfx `image.rs`, Yog-IDLE (`designResources.ts`, `book.rs`, `imageSources.ts`) | partly | as plain strings |
| NBT | SNBT strings | the game | — | SNBT strings (8 ABI calls) |
| Runtime ↔ host lists | TSV lines | yog-runtime, Java | — | `\t`/`\n`-joined strings |
| IDE state | JSON (`state.json`, `workspace.json`, publish profiles) | Yog-IDLE | none | — |

**Count:** 4 surface syntaxes (TOML, JSON, XML, ad hoc string grammars), ~6
places that parse references, 3 that parse recipes, 2 that parse Yog text,
and 5 different ways data crosses the ABI (C structs, JSON, TSV, SNBT,
generated code).

### 1.2 Tests and benchmarks (loader crates)

| Crate | Lines | Tests | Benches |
|---|---:|---:|---:|
| yog-cli | 14 742 | 67 | 5 |
| yog-runtime | 6 758 | **4** | 2 |
| yog-ui | 6 525 | 62 | 5 |
| yog-symbols | 3 796 | 16 | 1 |
| yog-api | 3 243 | **3** | 0 |
| yog-book | 2 994 | 14 | 0 |
| yog-kit | 2 231 | 18 | 0 |
| yog-debugger | 1 909 | 9 | 0 |
| yog-abi | 1 457 | **0** | 0 |
| yog-core | 1 318 | 1 | 1 |
| yog-gfx | 1 260 | 4 | 0 |
| yog-registry | 790 | **1** | 0 |
| yog-storage, yog-interop, yog-event, yog-config, yog-entity, yog-player, yog-command, yog-world, yog-logging | 2 090 together | **0** | 0 |
| yog-dimensions, yog-inventory, yog-network, yog-hot-reload | 1 274 together | 9 | 1 |

The runtime — the code every mod goes through — has 4 tests. The ABI, the
registry and nine domain crates have none.

### 1.3 Problems

1. **No shared schema.** What a field means is written down several times:
   the CLI's checks, the serde structs, Yog-IDLE's property tables
   (`PAGE_TYPES`, `CATEGORY_FIELDS`, …), the docs. They drift (the
   recipe editor knows 3 of 9 recipe kinds; docs listed fields the
   renderer never read).
2. **Stringly typed data at the ABI.** Books, recipes and dimensions are
   JSON strings, lists are TSV, NBT is SNBT: parsed again on every side,
   errors found at runtime.
3. **References parsed in many places,** each with its own rules and error
   messages.
4. **Per-format validation code in a 14k-line CLI**, with its own JSON
   parser for line numbers.
5. **Duplicated logic across languages** (Yog text in Rust and Java,
   recipe conversion in Rust and Java).

---

## 2. Proposal: Yog Data

One data model, one text syntax, one binary form, one schema system — for
every Yog format, the build, the ABI and the IDE.

### 2.1 The model

A tree of typed values, like NBT:

| Type | Example (text) |
|---|---|
| null, bool, int (i64), float (f64), string | `null`, `true`, `42`, `0.5`, `"Ruby"` |
| list, map | `[1, 2]`, `{ "a": 1 }` |
| **ref** — a typed reference to content | `"@item:minecraft:stick"`, `"@block:minecraft:furnace[facing=north]"` |
| **tag** — a content tag | `"@tag:item:minecraft:planks"` |
| **res** — a mod resource | `"@Strings.title"` |
| **text** — Yog text | `"{b|bold} and {#8d6acc|color}"` |
| **image** — an image source | `"@image:sprite:button"`, `"@image:mymod:gui/logo"` |
| **color** | `"#ff8d6acc"` |

Typed values are written as strings in the text form, so every file stays
valid JSON and every JSON tool still works; the *schema* says which type a
field holds, so `"@item:…"` in a text field is the item's name and in an
item field is the item.

### 2.2 The text syntax

JSON, plus comments, trailing commas and unquoted keys (JSON5's subset that
editors already understand). Source-only metadata (`$…`, `_…`, `comment`)
is allowed everywhere and stripped by the build — as today. `.yui` keeps its
XML shape (it's markup), but every attribute value is a Yog Data value with
the same literal grammar and the same schema system.

### 2.3 Schemas — written once

Each format is a Rust type with `#[derive(Schema)]` (crate `yog-data`).
One schema drives:

- **`yog build` / `yog check`**: validation with file, line and path
  (`entries[2].pages[0].item: no item mymod:nope in this mod`), reference
  checks (own content, vanilla per datapack version, declared `[external]`
  ids), metadata stripping;
- **Yog-IDLE**: property grids, pickers (a `ref<item>` field gets the item
  picker), completion and hover docs — generated from the schema, never
  hand-maintained;
- **docs**: the field tables in `docs/*.md` generated, so they can't drift;
- **migrations**: a `format` version per file, upgraded by the build.

### 2.4 The binary form

The build compiles every content file to Yog Data binary — tagged values,
varints, a string table, refs as (kind, id) pairs — like NBT. The runtime
and the Java hosts read it directly:

- one ABI call per kind of content, taking a binary blob, replaces JSON
  strings, TSV lines and the per-kind C structs (`register_item`'s
  `YogItemDef` etc.);
- smaller `.yog` files, no parsing JSON at game start;
- SNBT stays only where the game itself speaks it (entity/block NBT).

This is ABI 2.0: every mod is rebuilt (they are anyway).

### 2.5 Versions: datapacks

A mod names vanilla content by the ids of the vanilla data version it's
written against (its *datapack*: 1.20.1, 1.21.1, …). For each version the
loader supports, a reference maps to that version's equivalent (`grass` ↔
`short_grass`); content missing there is embedded into the mod for that
version. (To settle: embedding Mojang's assets runs into the EULA, and
content with behaviour can't simply be embedded.)

---

## 3. Plan

Each step ends green: tests pass, benches recorded, everything pushed.

**P0 — safety net (before any API change).**
Tests for the runtime (dispatch, registration, books/recipes/dimension
paths, control socket), yog-abi (layout and version checks), yog-registry,
yog-api's registry wrappers, the domain crates; golden tests for every
content format in yog-cli (input file → errors / generated code); Yog-IDLE
backend and model tests; Substrate. Benches: content compile per format,
runtime event dispatch, UI layout and draw, book render, JSON vs binary
parse (baseline for P1).

**P1 — `yog-data`.** Model, text parser with positions, binary codec,
`#[derive(Schema)]`, references and tags; Java reader. Benches against
serde_json.

**P2 — formats onto Yog Data,** one at a time with its golden tests:
`.yres`, `.ybook`, `.yrecipe` (tags, all kinds, custom kinds, layouts),
`.yblock`/`.yitem`, `.yui` attributes, dimensions; `yog.toml` (see the
decisions). Yog-IDLE's editors switch to schema-driven property grids.
Each format's old parser is deleted.

**P3 — ABI 2.0:** binary blobs for content; TSV and JSON strings removed.

**P4 — audit and cleanup** of every crate, Yog-IDLE and Substrate: dead
code, duplication (Yog text in Java, recipe conversion), the generic
book page drawn by mod code (replacing the Hex-specific pattern page),
performance where the benches point.

## 4. Decisions (2026-10-14)

1. Typed values are JSON strings (`"@item:minecraft:stick"`): every file stays JSON.
2. `yog.toml` becomes a Yog Data file too — one parser, one schema system.
3. ABI 2.0 ships once, at the end of P3; inside, content kinds move over one at a time.
