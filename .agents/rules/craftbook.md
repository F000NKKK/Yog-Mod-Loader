# Engineering standard: Craftbook

All Rust code in this repository follows **Craftbook**:
https://github.com/F000NKKK/craftbook

Before writing or reviewing Rust code, read its `README.md` (limits table) and
the chapter for what you are touching in `rust/`:
files and modules (01), functions (02), naming (03), types and errors (04),
docs (05), tooling (06), DDD architecture (07), testing and coverage (08),
review checklist (09). Where this repository's own docs disagree with
Craftbook, Craftbook wins and the local doc is fixed in the same change.

The rules most often broken, in short:

- A file has at most 400 production lines (target 250) and one primary item,
  named by the file; `lib.rs`/`mod.rs` of a split module is wiring only
  (`mod`, `pub use`), no logic. Modules are directories with `mod.rs`; no
  `utils`/`helpers`/`common`/`types` modules.
- A function has at most 50 lines (target 25), 5 parameters with `self`, no
  `bool` flag parameters. Method if it touches a type's state or invariant;
  free function for symmetric operations and for I/O modules with no state;
  trait only for two or more implementations. No field-less structs used as
  namespaces, no managers, no global mutable state.
- Names come from the glossary: full words, acronyms as words (`GpuDevice`),
  no `Manager`/`Helper`/`Data`/`Info`, no `get_` getters, no stutter
  (`audio::streaming`, not `audio::audio_streaming`).
- Strings are data, never structure: ids, keys, paths, commands, events and
  errors are typed; text is parsed once on the way in and rendered once on
  the way out.
- Libraries return typed error enums (`thiserror`), never `anyhow`; no
  `unwrap` outside tests; `#[expect(lint, reason = "…")]`, never `#[allow]`.
- DDD layers: interface → app → domain ← infra, everyone may use the shared
  kernel. Domain has no I/O and no `async`; I/O ports are traits in the app
  crate, implemented by adapter crates.
- Coverage: shared kernel 95 %, domain 90 %, app 85 %, adapters 70 % of lines;
  changed lines in a PR 90 %. Every bug fix comes with a regression test.

Check before pushing: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`python3 <craftbook>/tools/craftcheck check .` and `… deps .`, tests.

Java and TypeScript code, and the Yog-specific rules (single source of truth
in `yog-ids`, `yog-sync`, the Java hosts), keep following
[`CODE_STANDARDS.md`](../../CODE_STANDARDS.md); for Rust, where the two
differ, Craftbook's stricter rule applies.
