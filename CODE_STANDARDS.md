# Yog code standards

The one checklist for every Yog repository (Yog-Mod-Loader, Yog-IDLE, Substrate-Platform and the mods). Every file is reviewed against it, and new code is written to it. It follows Clean Code, the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), and the project's own rules (the "Rules" section). Items marked **[auto]** are checked by `tools/standards_check.py` or by the linters it runs; the rest are checked by review.

## 0. The law of single truth

Every fact has exactly one source; everything else derives from it (composed constants, imports, re-exports, generated TS/Java). A fact about a lower level is defined at that level and flows upward — namespaces, content kinds, schemes and Yog's own ids live in `yog-ids`, the lowest crate. Before defining anything, search every repository for it.

## 1. Must not be in the code

### Meaning carried by strings and numbers
- [ ] **[auto]** **String literals naming a kind, a namespace, a registry, a key prefix, a file extension or a path.** Examples: `"item"`, `"block"`, `"minecraft:"`, `"yog:potion/"`, `"potion.effect."`, `".yitem"`, `"textures/"`. Each such fact has one definition: an enum, a `const`, or a typed id. Every use goes through it.
- [ ] **[auto]** **Magic numbers.** Examples: `0xFF_CCCCAA`, `9.0`, `20`, `16.0`, `3600`. A colour, size, tick count, offset or limit is a named `const` next to its meaning, or data (`.yres`, `.ydata`). Exceptions: `0`, `1`, `-1`, `2` as arithmetic, and test fixtures.
- [ ] **Stringly-typed values.** Examples: ids as bare `String` with prefixes glued by `format!`; `[f32; 4]` as a rectangle; `(f32, f32)` as a size; `u32` as a colour; `&str` as an enum. Use the shared types (`yog_graphics::{Color, Rect, Vec2, …}`), newtypes and enums.
- [ ] **Parsing the same text in several places.** Examples: colours, ids, `ns:path`. One parser, one type.

### Duplication
- [ ] **The same logic in two places.** This covers:
  - crates, editors and the TS/Rust sides;
  - the six Java hosts — fabric/forge/neoforge × 1.20.1/1.21.1. Version-neutral code lives once, in `java-common`, behind small per-platform adapters.
- [ ] **Per-crate copies of shared types.** Examples: `Rect`, `Vec3`, `Color`, `Image`. Use `yog-graphics`.
- [ ] **Per-component copies of a list.** Examples: languages, formats, editors, labels, kinds. One registry, read by everyone ("Single source of truth").
- [ ] **Copied game data** (vanilla JSON, lang keys) where a reference to the game's own would do.

### Size and shape
- [ ] **[auto]** **Files over 800 lines.** Split them by responsibility. Generated files and data tables are exempt.
- [ ] **[auto]** **Functions over 80 lines**, or nested more than 4 levels deep.
- [ ] **[auto]** **More than 5 parameters.** Group them into a struct; `#[allow(clippy::too_many_arguments)]` is not a fix. FFI signatures fixed by the ABI are exempt.
- [ ] **Boolean flag parameters that switch behaviour.** Use two functions or an enum.

### Silencing and failure
- [ ] **[auto]** **`#[allow(dead_code)]`, `#[allow(unused_*)]`, `@ts-ignore`, `eslint-disable`, `@SuppressWarnings`** without a one-line reason. Dead code is deleted, not hidden.
- [ ] **[auto]** **`unwrap()` / `expect()` / `panic!` in non-test code** where the value can be missing. They are allowed only on invariants, with the reason in the `expect` message. No panics across FFI: callbacks are guarded.
- [ ] **Swallowed errors.** Examples: `let _ = fallible()`, `.ok()` dropping the error, `catch {}`, `catch (e) {}`. Either handle the error, propagate it with `?`, or log it with context.
- [ ] **[auto]** **`System.out` / `System.err` / `println!` / `eprintln!` / `console.log` in libraries and hosts.** Use the logger (`yog_logging`, the game's logger, Substrate's output). CLI user output is exempt.

### Rules of the project
- [ ] **Mod-specific logic in the loader** ("Loader generic only"), and **Yog-specific logic in Substrate-Platform** ("Substrate stays generic").
- [ ] **Backward-compat shims, deprecated aliases, fallback defaults on traits** ("No backward compat").
- [ ] **Test or bench code in production builds** ("Test code out of prod"). Use `cfg(test)` or features.
- [ ] **Closed lists where mods should extend.** Examples: tiers, sounds, recipe kinds, page types ("Open vocabularies"). Built-ins are data.
- [ ] **Game-version or loader specifics leaking into mod-facing APIs or formats** ("Yog-own resource formats").

### UI text
- [ ] **[auto]** **User-visible text not passed through `t()`, or a key missing from the catalogs.** `Yog-IDLE/src/locales/catalog.test.ts` enforces this.
- [ ] **Two spellings or two names for one thing** (colour/color, "Designer"/"Editor" for the same view).

### Comments and leftovers
- [ ] **Commented-out code.**
- [ ] **Comments that narrate history** ("changed X", "was Y", "new:") or restate the code.
- [ ] **`TODO` / `FIXME` without what and why.**
- [ ] **Debug leftovers.** Examples: temporary logs, `dbg!`, harness files, scratch scripts, generated files committed by accident.

## 2. Must be in the code

### Types and data
- [ ] **Types that carry meaning.**
  - Enums for closed sets inside the code; newtypes for ids.
  - `yog_graphics` types for geometry and colour.
  - `From`/`TryFrom` for conversions; `as_`/`to_`/`into_` naming (C-CONV).
- [ ] **One registry per fact.** Kinds, namespaces, formats, editors and languages are each defined once. Generated code or data feeds the other language (Rust → TS/Java) instead of a hand copy.
- [ ] **Data over code.** Vocabularies, layouts and presets live in data files the code reads.

### Structure
- [ ] **Small modules with one responsibility.** The module doc says what the module is for and why.
- [ ] **Functions that do one thing**, with names that say what. No `get_` prefix on getters (C-GETTER). Verbs for actions.
- [ ] **Narrow visibility.** `pub(crate)` by default; `pub` only for the API.
- [ ] **No glob imports** outside preludes and tests.
- [ ] **Platform/version differences isolated in adapters**, not spread as `if version …` branches.

### Errors and logging
- [ ] **Fallible functions return `Result`** with an error that says what failed and with what input. `?` propagates it.
- [ ] **Errors are logged once, at the boundary**, with context.
- [ ] **FFI and callbacks catch panics.**

### Rust conventions ([API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html))
- [ ] **Naming:** `snake_case` / `CamelCase` / `SCREAMING_SNAKE` (C-CASE).
- [ ] **Common traits derived:** `Debug`, `Clone`, `Default`, `PartialEq`, `Eq`, `Hash` where they make sense (C-COMMON-TRAITS).
- [ ] **`#[must_use]`** on pure functions and builders whose result matters.
- [ ] **Doc comments on every public item**; examples where usage isn't obvious.
- [ ] **[auto]** **`cargo fmt` clean.**
- [ ] **[auto]** **`cargo clippy --all-targets -- -D warnings` clean**, plus the workspace lint set.
- [ ] **Tests next to the code they test.** Golden tests for generators; round-trip tests for formats.

### TypeScript
- [ ] **`strict` mode, no `any`.** No non-null `!` without a reason.
- [ ] **Constants and enums in one module per concept.**
- [ ] **Components small;** state in hooks/stores, not in giant components.
- [ ] **Shared UI from Substrate** (buttons, menus, inspectors, dialogs), not raw HTML controls.

### Java hosts
- [ ] **Version-neutral logic in `java-common`;** per-platform classes only adapt the game's API.
- [ ] **The game's logger, not `System.err`.**
- [ ] **Ids and keys from one constants class**, not literals.

## 3. Review process
1. `tools/standards_check.py` lists every source file of every repository and the automatic findings per file. Its report is `docs/standards-review.tsv`: path, findings, review status.
2. Each file is then read against section 1 and 2 by hand and fixed; its status becomes `ok` with the commit.
3. A file isn't `ok` while it has an automatic finding, unless the finding is a listed exemption.
4. New and changed files must pass the check before they're committed.
