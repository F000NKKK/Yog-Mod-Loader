//! `yog publish exports` — the `-exports` crate generated from a mod's
//! `#[yog_export]` items.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use yog_codegen::{Header, Regenerate};

/// Mod `mod_id`'s exports crate's name, as it's published to crates.io:
/// `my-mod` → `my_mod_exports`.
pub fn crate_name(mod_id: &str) -> String {
    format!("{}{EXPORTS_SUFFIX}", yog_ids::rust_name(mod_id))
}

/// What an exports crate's name adds to its mod's.
const EXPORTS_SUFFIX: &str = "_exports";

/// What a `#[yog_export]` item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// `pub struct`: shared as a type.
    Struct,
    /// `pub enum`: shared as a type.
    Enum,
    /// `pub fn`: called through the bus.
    Fn,
}

/// Item found by the export scanner.
#[derive(Debug)]
pub struct ExportItem {
    /// What it is.
    pub kind: ExportKind,
    pub(crate) name: String,
    /// Full Rust source of the item (without `#[yog_export]`).
    pub source: String,
}

/// Every `#[yog_export]` item of one source file (inline modules too),
/// read with `syn`: its exact text — comments, layout — without the
/// `#[yog_export]` attribute itself. A file that doesn't parse has none
/// (the build reports it).
pub fn scan(src: &str) -> Vec<ExportItem> {
    let Ok(file) = syn::parse_file(src) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect(&file.items, src, &mut out);
    out
}

fn collect(items: &[syn::Item], src: &str, out: &mut Vec<ExportItem>) {
    use syn::spanned::Spanned;
    for item in items {
        let (attrs, vis, kind, name) = match item {
            syn::Item::Struct(s) => (&s.attrs, &s.vis, ExportKind::Struct, s.ident.to_string()),
            syn::Item::Enum(e) => (&e.attrs, &e.vis, ExportKind::Enum, e.ident.to_string()),
            syn::Item::Fn(f) => (&f.attrs, &f.vis, ExportKind::Fn, f.sig.ident.to_string()),
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect(inner, src, out);
                }
                continue;
            }
            _ => continue,
        };
        let is_export = |a: &syn::Attribute| {
            a.path()
                .segments
                .last()
                .is_some_and(|s| s.ident == "yog_export")
        };
        if !attrs.iter().any(is_export) || !matches!(vis, syn::Visibility::Public(_)) {
            continue;
        }
        let whole = item.span().byte_range();
        // The item's text with each `#[yog_export]` (and the line it leaves empty) cut out.
        let mut text = String::new();
        let mut at = whole.start;
        for a in attrs.iter().filter(|a| is_export(a)) {
            let r = a.span().byte_range();
            text.push_str(&src[at..r.start]);
            at = r.end;
            let rest = &src[at..whole.end];
            let blank = rest.len() - rest.trim_start_matches([' ', '\t']).len();
            if rest[blank..].starts_with('\n') {
                at += blank + 1;
            }
        }
        text.push_str(&src[at..whole.end]);
        // Indentation of nested items stays as written; the text starts at the item.
        let source = format!("{}\n", text.trim_start());
        out.push(ExportItem { kind, name, source });
    }
}

/// Names always in scope — primitives and the std/core/alloc prelude and
/// modules — that never need an import.
const PRIMITIVES: &[&str] = &[
    "bool",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "f32",
    "f64",
    "char",
    "str",
    "String",
    "Vec",
    "Option",
    "Result",
    "Box",
    "Rc",
    "Arc",
    "Cow",
    "PhantomData",
    "Ordering",
    "Self",
    "()",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "LinkedList",
    "VecDeque",
    "BinaryHeap",
    "Path",
    "PathBuf",
    "Duration",
    "Instant",
    "SystemTime",
    "Error",
    "fmt",
    "ops",
    "cmp",
    "iter",
    "slice",
    "cell",
    "sync",
    "collections",
    "convert",
    "marker",
    "mem",
    "ptr",
    "hint",
    "borrow",
    "hash",
    "default",
    "num",
    "array",
    "range",
    "std::",
    "core::",
];

/// Paths into std/core/alloc — always available, imported as "system" uses.
const SYSTEM_ROOTS: [&str; 3] = ["std::", "core::", "alloc::"];

/// Path roots internal to the mod crate, meaningless in the exports crate.
const LOCAL_ROOTS: [&str; 3] = ["crate", "super", "self"];

fn is_system(path: &str) -> bool {
    SYSTEM_ROOTS.iter().any(|p| path.starts_with(p))
}

/// What a path's segments are joined with.
const PATH_SEP: &str = "::";
/// What a `use` line starts with.
const USE: &str = "use ";
/// What a glob import ends with.
const GLOB: &str = "*";

/// The imports the exports crate needs: std/core/alloc ones, and the rest.
#[derive(Default)]
struct Imports {
    system: HashSet<String>,
    types: HashSet<String>,
}

impl Imports {
    fn add(&mut self, path: String) {
        if is_system(&path) {
            self.system.insert(path);
        } else {
            self.types.insert(path);
        }
    }
}

/// The paths of the mod's own `use` lines (`yog_api::registry::Mod`,
/// `foo::{a, b}`, `foo::*`); a broken one (`a::{` alone) left out.
fn mod_uses(sources: &[String]) -> Vec<String> {
    sources
        .iter()
        .flat_map(|src| src.lines())
        .filter_map(|line| line.trim().strip_prefix(USE))
        .map(|p| p.trim_end_matches(';').trim())
        .filter(|p| !p.is_empty() && !(p.contains('{') && !p.contains('}')))
        .map(str::to_string)
        .collect()
}

/// What a name in an export resolves to, as `rustc` would find it.
struct Resolver<'a> {
    uses: &'a [String],
    /// The exported items' names: defined in the crate itself.
    local: HashSet<&'a str>,
}

impl Resolver<'_> {
    /// The `use` path bringing `name` into scope — the shortest (most
    /// specific) whose last segment is it, or a glob; none for a
    /// primitive, a std name or one the crate defines.
    fn resolve(&self, name: &str) -> Option<String> {
        if PRIMITIVES.contains(&name) || is_system(name) || self.local.contains(name) {
            return None;
        }
        self.uses
            .iter()
            .filter(
                |p| matches!(p.rsplit(PATH_SEP).next(), Some(last) if last == name || last == GLOB),
            )
            .min_by_key(|p| p.len())
            .cloned()
    }
}

/// The path starting at `slice` (`::foo::Bar`, `yog_api::x::Y<T>`): up to
/// what ends it outside angle brackets.
fn path_at(slice: &str) -> &str {
    let mut depth: i32 = 0;
    let mut end = 0;
    for (i, ch) in slice.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ' ' | ',' | ';' | '{' | '}' | '(' | ')' | ':' | '[' | ']' | '.' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
        if depth < 0 {
            end = i;
            break;
        }
    }
    slice[..end].trim().trim_end_matches(PATH_SEP)
}

/// The written-out paths an export uses: std ones, the API's and the mod's own.
fn written_paths(body: &str, imports: &mut Imports) {
    let api = yog_ids::rust_name(yog_ids::rust_crate::API);
    for (start, _) in body.match_indices(PATH_SEP) {
        let path = path_at(&body[start..]);
        if path.is_empty() {
            continue;
        }
        let first = path
            .trim_start_matches(':')
            .split(PATH_SEP)
            .next()
            .unwrap_or("");
        if is_system(path) || first == api || LOCAL_ROOTS.contains(&first) {
            imports.add(path.to_string());
        }
    }
}

/// The bare type names an export uses (CamelCase), each by its import.
fn bare_types(body: &str, resolver: &Resolver, imports: &mut Imports) {
    let words = body
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty());
    for word in words {
        if word.starts_with(char::is_uppercase)
            && let Some(p) = resolver.resolve(word)
        {
            imports.add(p);
        }
    }
}

/// Collect the imports a generated exports crate needs.
///
/// This mirrors how `rustc` resolves names:
///   1. Is the name a primitive / in `std` / `core` / `alloc` (always in scope)? → no import.
///   2. Is the name defined in this same crate (an exported struct/enum/fn)? → no import.
///   3. Otherwise, does the mod's own `use` statements bring it into scope?
///      → emit the matching `use` path (preferring the most specific one).
///   4. Paths written out in full (`yog_api::…`, `std::…`) come as they are.
///
/// Returns `(system_uses, type_uses)` where:
/// - `system_uses`: std/core/alloc `use` paths like `std::collections::HashMap`
/// - `type_uses`: external `use` paths like `yog_api::registry::Mod`
pub fn collect_uses(exports: &[ExportItem], sources: &[String]) -> (Vec<String>, Vec<String>) {
    let uses = mod_uses(sources);
    let mut imports = Imports::default();
    for u in &uses {
        imports.add(u.clone());
    }
    let resolver = Resolver {
        uses: &uses,
        local: exports.iter().map(|e| e.name.as_str()).collect(),
    };
    for item in exports {
        written_paths(&item.source, &mut imports);
        bare_types(&item.source, &resolver, &mut imports);
    }
    (normalize(imports.system), normalize(imports.types))
}

/// Every `a::b::{X, Y}` / `a::b::X` merged into one import per module
/// path, so a name pulled in by several files is imported once; sorted.
fn normalize(paths: HashSet<String>) -> Vec<String> {
    let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut nested: Vec<String> = Vec::new();
    for p in paths {
        let p = p.trim().to_string();
        if let Some((prefix, rest)) = p.split_once("::{") {
            let inside = rest.strip_suffix('}').unwrap_or(rest);
            if inside.contains('{') {
                nested.push(p);
                continue;
            }
            let names = inside
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            groups.entry(prefix.to_string()).or_default().extend(names);
        } else if let Some((prefix, item)) = p.rsplit_once(PATH_SEP) {
            groups
                .entry(prefix.to_string())
                .or_default()
                .insert(item.to_string());
        } else {
            nested.push(p);
        }
    }
    let mut keep: Vec<String> = groups
        .into_iter()
        .map(|(prefix, names)| grouped(&prefix, names))
        .collect();
    nested.sort();
    nested.dedup();
    keep.extend(nested);
    keep.sort();
    keep
}

/// `prefix::Name`, or `prefix::{A, B}` for several.
fn grouped(prefix: &str, names: BTreeSet<String>) -> String {
    let names: Vec<String> = names.into_iter().collect();
    match names.as_slice() {
        [one] => format!("{prefix}{PATH_SEP}{one}"),
        _ => format!("{prefix}{PATH_SEP}{{{}}}", names.join(", ")),
    }
}

/// Strip `#[derive(...)]` lines from a Rust source item to avoid duplicates
/// when the generator adds its own derives.
pub(crate) fn strip_derive_attrs(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim().starts_with("#[derive("))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A `#[yog_export]` function's signature as an `import!` item: `pub fn
/// name(params) -> Out;` — the parameters as the export names them, the
/// export's own `Registry` left to `import!`. Nothing if it doesn't parse.
pub fn import_signature(source: &str) -> Option<String> {
    use syn::spanned::Spanned;
    let f = syn::parse_str::<syn::ItemFn>(source).ok()?;
    let sig = source.get(f.sig.span().byte_range())?;
    // One line: a wrapped signature's line breaks become spaces.
    Some(format!(
        "pub {};",
        sig.split_whitespace().collect::<Vec<_>>().join(" ")
    ))
}

/// Recursively collect every `.rs` file under `dir`, so `#[yog_export]` items
/// can live in any module/subfolder — not just flat files directly in `src/`.
pub fn collect_rs_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_rs_files(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(out)
}

/// What the exports crate's `Cargo.toml` says.
pub struct CrateMeta<'a> {
    /// The crate's name (`<mod>-exports`), version and Rust edition.
    pub name: &'a str,
    /// See [`CrateMeta::name`].
    pub version: &'a str,
    /// See [`CrateMeta::name`].
    pub edition: &'a str,
    /// The mod's authors, joined with `", "`.
    pub authors: &'a str,
    /// Its license.
    pub license: &'a str,
    /// The mod it calls into.
    pub mod_id: &'a str,
    /// The `yog-api` version it builds with.
    pub yog_api: &'a str,
}

/// The exports crate's `Cargo.toml`.
pub fn cargo_toml(m: &CrateMeta) -> String {
    let maybe_authors = if m.authors.is_empty() {
        String::new()
    } else {
        format!("authors = [\"{}\"]\n", m.authors.replace(", ", "\", \""))
    };
    format!(
        r#"[package]
name = "{name}"
version = "{version}"
edition = "{edition}"
{maybe_authors}license = "{license}"
description = "Exports from the {mod_id} Yog mod — generated by `yog publish exports`."

[dependencies]
yog-api = "{yog_api}"

[lib]
crate-type = ["cdylib", "lib"]
"#,
        name = m.name,
        version = m.version,
        edition = m.edition,
        license = m.license,
        mod_id = m.mod_id,
        yog_api = m.yog_api,
    )
}

/// The exports crate's `src/lib.rs`: the imports from [`collect_uses`],
/// the structs and enums (`#[yog_export]`: their schemas), and the
/// functions as `import!` items — calls of the mod's services.
pub fn lib_rs(
    mod_id: &str,
    version: &str,
    exports: &[ExportItem],
    system_uses: &[String],
    type_uses: &[String],
) -> String {
    let header = Header {
        tool: "`yog publish exports`",
        source: &format!("`{mod_id}` v{version}"),
        regenerate: Regenerate::Command("yog publish exports"),
    };
    let mut lib_rs = format!(
        "//! The exports of `{mod_id}` v{version}.\n{}\n\n#![allow(unused_imports)]\n\n",
        yog_codegen_rust::Rust::inner_doc(&header.text())
    );

    // Add system uses (std, core, alloc)
    if !system_uses.is_empty() {
        lib_rs.push_str("// System dependencies\n");
        for u in system_uses {
            let _ = writeln!(lib_rs, "use {u};");
        }
        lib_rs.push('\n');
    }

    // Add type dependencies (yog_api, rkyv, etc.)
    // Filter out crate-internal imports and duplicates of exported items
    let exported_names: HashSet<&str> = exports.iter().map(|e| e.name.as_str()).collect();
    if !type_uses.is_empty() {
        lib_rs.push_str("// Type dependencies\n");
        for u in type_uses {
            // Skip imports that:
            // 1. Reference crate:: (internal modules not available in exports crate)
            // 2. Are the same as an exported item (would cause duplicate definitions)
            let first_part = u.split("::").next().unwrap_or("");
            if LOCAL_ROOTS.contains(&first_part) {
                continue;
            }
            let last_part = u.rsplit("::").next().unwrap_or(u);
            if exported_names.contains(last_part) {
                continue;
            }
            let _ = writeln!(lib_rs, "use {u};");
        }
        lib_rs.push('\n');
    }

    // Types: with the schema their values cross with (`#[yog_export]`).
    for item in exports.iter().filter(|i| i.kind != ExportKind::Fn) {
        let _ = writeln!(
            lib_rs,
            "#[::yog_api::yog_export]\n#[derive(Debug, Clone, PartialEq)]\n{}\n",
            strip_derive_attrs(&item.source)
        );
    }
    // Functions: calls of `{mod_id}:name` on the bus (`import!`).
    let fns: Vec<String> = exports
        .iter()
        .filter(|i| i.kind == ExportKind::Fn)
        .filter_map(|i| import_signature(&i.source))
        .collect();
    if !fns.is_empty() {
        let module = yog_ids::rust_name(mod_id);
        let _ = writeln!(
            lib_rs,
            "::yog_api::import! {{\n    from {mod_id:?} {{\n        {}\n    }}\n}}\npub use {module}::*;",
            fns.join("\n        ")
        );
    }
    lib_rs
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
use yog_api::prelude::*;
use yog_codegen::{Header, Regenerate};

#[yog_export]
pub struct Pipe { pub id: u32 }

#[yog_export]
pub enum Kind { Item, Fluid }

/// Not exported.
pub fn helper() {}

#[yog_export]
pub fn send(
    registry: &mut Registry,
    pipe: Pipe,
    amount: u32,
) -> Option<String> {
    None
}

#[yog_export]
pub fn count() -> u64 { 0 }
"#;

    #[test]
    fn scans_every_exported_item_and_nothing_else() {
        let items = scan(SRC);
        let names: Vec<(&str, ExportKind)> =
            items.iter().map(|i| (i.name.as_str(), i.kind)).collect();
        assert_eq!(
            names,
            [
                ("Pipe", ExportKind::Struct),
                ("Kind", ExportKind::Enum),
                ("send", ExportKind::Fn),
                ("count", ExportKind::Fn)
            ]
        );
        assert!(
            !items.iter().any(|i| i.source.contains("#[yog_export]")),
            "the attribute is stripped"
        );
        assert!(
            items[2].source.contains("amount: u32"),
            "a wrapped signature is whole"
        );
    }

    #[test]
    fn functions_become_import_items_and_types_exports() {
        let items = scan(SRC);
        let lib = lib_rs("yog-pipes", "1.0.0", &items, &[], &[]);
        assert!(
            lib.contains("::yog_api::import! {\n    from \"yog-pipes\""),
            "{lib}"
        );
        assert!(lib.contains("pub fn send( registry: &mut Registry, pipe: Pipe, amount: u32, ) -> Option<String>;"), "{lib}");
        assert!(lib.contains("pub use yog_pipes::*;"), "{lib}");
        assert!(
            lib.contains(
                "#[::yog_api::yog_export]\n#[derive(Debug, Clone, PartialEq)]\npub struct Pipe"
            ),
            "{lib}"
        );
    }

    #[test]
    fn a_doc_comment_with_parentheses_keeps_the_parameters() {
        let src = "/// Drives `channel` (a pipe or an endpoint); 0 stops.\n#[yog_export]\npub fn emit(\n    capability: String,\n    mut pos: (i32, i32),\n    levels: HashMap<String, u8>,\n) -> Result<(), String> {\n    todo!()\n}\n";
        let item = &scan(src)[0];
        assert!(item.source.starts_with("/// Drives"), "{}", item.source);
        let sig = import_signature(&item.source).expect("parses");
        assert!(
            sig.contains("capability: String, mut pos: (i32, i32), levels: HashMap<String, u8>,"),
            "{sig}"
        );
    }

    #[test]
    fn braces_in_strings_and_comments_dont_end_an_item() {
        let src = r#"
mod inner {
    /// Opens `{` — and a doc comment can say so.
    #[yog_export]
    pub fn open(s: String) -> String {
        // } not the end
        format!("{{{s}")
    }
}

#[yog_export]
pub struct After { pub x: u8 }
"#;
        let items = scan(src);
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["open", "After"]);
        assert!(
            items[0].source.starts_with("/// Opens"),
            "{}",
            items[0].source
        );
        assert!(
            items[0].source.trim_end().ends_with('}')
                && items[0].source.contains("format!(\"{{{s}\")"),
            "{}",
            items[0].source
        );
        assert!(!items[0].source.contains("yog_export"));
    }
}
