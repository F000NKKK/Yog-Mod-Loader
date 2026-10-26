# Design: Yog Script bindings to the generated Rust

Status: content ids, `res.g.ys` and the main language implemented
(2026-10-26, not yet built and tested); screens are design only. Scope: `yog-cli` (content pipeline, `.yui`
codegen, `yog.toml`), `yog-codegen-ys`, `yog-script-*`, `yog-kit`,
`java-api`, Yog-IDLE.

Decided by the user (2026-10-26):

- Every mod keeps its **generated Rust**: content (`.yblock`, `.yitem`,
  …) and `.yui` design code. A mod without `src/lib.rs` still has a
  native, whose crate root `yog build` writes (`obj/generated/mod.g.rs`).
- Yog Script gets **its own generators** (`yog-codegen-ys`). They emit YS
  **bindings** that link to that Rust, not a second implementation of it.
- What content *is* is said once, in its file. A `block Lamp { … }` is
  `lamp.yblock`'s **behaviour**; a property written in it is an error
  (YS0037), and so is a declaration without its file (YS0038).
- A mod's **main language** (`yog.toml`) picks the defaults of the tools.
  It never turns a language off.

---

## 1. One project, generated sources in it

A mod's `.ys` files are one project, as a C# project's files: each
reaches the others' types (`Env::others`; in Java, nested in their
files' classes). Generated YS sources are part of the project:

| Generated | From | What the code reaches |
|---|---|---|
| `obj/generated/yog-content.g.ys` ✅ | `content/**` | `Blocks.Lamp`, `Items.Gem`, `Entities`, `Textures`, `Models`, `Anims`, `Advancements`: each id by name, the same sets as the Rust's (`Blocks::LAMP`) |
| `obj/generated/ui/<screen>.g.ys` | `src/**/*.yui` | the screen's base class (§2) |
| `obj/generated/res.g.ys` ✅ | `resources/*.yres` | `Res.<Set>.<Entry>`: texts, media ids, colors, numbers, flags (§4) |

Each is written before the Yog Script is checked (`prepare`), and only
when it changed. A source the build generates is never reported as the
mod's own: its errors are the generator's bugs.

## 2. Screens with a Yog Script code-behind

### 2.1 What the author writes

```ys
// src/charger.yui: <Screen id="charge:charger" x:ViewModel="ChargerScreen"> …
screen ChargerScreen(Charger block) : ChargerUi {
    Energy Energy => block.Charge.Energy;
    f32 Fill => Energy / block.Charge.Capacity;
    public string Filter = "";                 // two-way: <TextBox text="{x:Bind Filter, Mode=TwoWay}"/>
    server void Dump() { block.Charge.Energy = 0; }
}
```

`ChargerUi` is generated from `charger.yui` (`obj/generated/ui/charger.g.ys`):

```ys
// generated
public abstract class ChargerUi : Screen {
    public const string Id = "charge:charger";
    public static class Ids { public const string Fill = "fill"; … }   // named widgets
}
```

`Screen` (Yog) has the lifecycle as events (`Opened`, `Closed`,
`Back`, `Tick`), `Open()`/`Close()`, `Navigate<T>()`, and `Dialog<T>()`,
as yog-kit's `Cx` does for Rust (mvvm-v2 V11, V12).

### 2.2 Where the view model runs

The design code stays Rust (`yog_kit::View`, generated as now). Its
`type Vm` is a generated **mirror**: a Rust struct with one field for
each member the markup binds, of the member's type as Yog Data. The YS
view model lives on the JVM, on the client. The two talk over the bus,
in-process and synchronously, so the screen never lags a frame behind:

| Message | Direction | Payload |
|---|---|---|
| `state` | YS → native | the bound members that changed, by index |
| `command` | native → YS | the command's index and its arguments |
| `set` | native → YS | a two-way binding's member index and its new value |
| `life` | native → YS | `Opened`, `Closed`, `Back`, `Tick` |

The addresses are `<mod>:ui/<screen>/<message>`. The message words live
in `yog-ids` (`ui_channel`), and the Java and TS spellings are generated
from them.

After a command, a handler or a `set`, the YS side sends the members whose
values changed. Which members a method can write is known statically: the
analyzer computes each method's write set, as mvvm-v2's V2 does for Rust.
A snapshot comparison of the bound members is the fallback, and decides
the final value: tracking only skips work. The native side applies the
diff to the mirror and runs `View::update`, whose `Slot` diff patches only
the widgets that changed.

### 2.3 Checks at `yog build`

- Every `{x:Bind}` path names a member of the YS view model, with a
  `.yui:line:col` position and a "did you mean" hint (mvvm-v2 V8). The
  schema comes from the analyzer's types of the `screen` class, not from
  rustc.
- A bound member's type must cross as Yog Data: numbers, `bool`,
  `string`, `Text`, quantities, enums, records and lists of those. Any
  other type is an error with a new code.
- A `command=` must name a method of the view model. Its arguments are
  checked against the method's parameters.
- `x:ViewModel` resolves to the project's YS types first, then to Rust.
  A name that is both is an error.

### 2.4 Lists and templates

A list's items are records: each item is mirrored as its record, and
templates bind to its fields. Rows are reconciled by key, as v1 does
(`ItemsHost`). `ObsVec<T>`'s change log (mvvm-v2 V4) crosses as
insert/remove/move messages once YS has its own `ObsList<T>`.

### 2.5 Later: no mirror

Rust lowering (ys.md §9.6) can move a view model into the native. Its
optimizer decides this per screen (ys-mixed-lowering). When it does, the
mirror and the messages go away, and `type Vm` is the lowered struct.
The markup and the `.ys` don't change.

## 3. The mod's main language

```toml
[mod]
language = "ys"      # or "rust"; "ys" when absent
```

The setting is a **preference** for the tools. Both languages always
build, and a mod may mix them. Every place below picks the main language
by default, and can be switched per file:

| Where | What follows the main language |
|---|---|
| `yog new` | the starting sources: `src/main.ys` with a `mod`, or `src/lib.rs` |
| A new `.yui` (IDLE, `yog new screen`) | its code-behind file: a `screen` in `.ys`, or a `#[view_model]` struct in `.rs` |
| IDLE "Go to code-behind" / "Create handler" | the file it opens or creates, and the method stub it writes |
| A new block or item (IDLE content editor) | the behaviour stub it offers to add: `block X { }` or a Rust `impl` |
| Designers that bind to code (screen, book, particle triggers) | the language of the handlers they generate |
| Snippets, completion and templates | the language offered first |
| `yog check` hints | "add a `.ys` handler" vs "add a Rust fn" |

The generated code doesn't change: design code stays Rust, and bindings
are generated for both languages whenever their sources exist. IDLE reads
the setting through the CLI (`formats through the CLI`), never by parsing
`yog.toml` itself.

## 4. Resources (`.yres`)

`Res.Strings.Title`, `Res.Icons.Max`: the `.yres` compiler emits
`obj/generated/res.g.ys` beside the Rust `res` module, entry by entry. A set
becomes a class nested in `Res`, and each entry becomes a member named by
its accessor in PascalCase (`screen.title` gives `ScreenTitle`). The kinds
map as follows:

- A text is `public static string Title => t"m.title";`. The source
  can't yet write the type `Text`, and a text's parameters are formatted
  where it's shown.
- An image, sound, file or video is its id, as a `const string`.
- A color is a `const u32`, a number a `const i64` or `const f64` (as
  the Rust's: `accessors::number`), and a flag a `const bool`.

## 5. Steps

1. ✅ `yog-codegen-ys`; content ids (`yog-content.g.ys`); a YS-only mod's
   native; the project check (`others`); YS0037/YS0038.
2. ✅ `res.g.ys` from `.yres` (§4).
3. ✅ `yog.toml` `language`, and `yog new` starts a mod in it (`--rust`).
   Next: IDLE's defaults follow it (§3).
4. Screens (§2): `Screen` in Yog; the generated `<screen>.g.ys`;
   the mirror and the `ui_channel` messages; binding checks against the
   YS schema; write sets.
5. Lists, dialogs and navigation for YS screens.
