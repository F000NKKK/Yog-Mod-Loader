//! `yog publish exports` — the `-exports` crate generated from a mod's
//! `#[yog_export]` items.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// What a `#[yog_export]` item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Struct,
    Enum,
    Fn,
}

/// Item found by the export scanner.
#[derive(Debug)]
pub struct ExportItem {
    pub kind: ExportKind,
    pub name: String,
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
        let is_export = |a: &syn::Attribute| a.path().segments.last().is_some_and(|s| s.ident == "yog_export");
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

/// Collect the imports a generated exports crate needs.
///
/// This mirrors how `rustc` resolves names:
///   1. Is the name a primitive / in `std` / `core` / `alloc` (always in scope)? → no import.
///   2. Is the name defined in this same crate (an exported struct/enum/fn)? → no import.
///   3. Otherwise, does the mod's own `use` statements bring it into scope?
///      → emit the matching `use` path (preferring the most specific one).
///   4. If it's still unresolved, emit a `use` for the crate the type likely
///      lives in (e.g. `yog_api::…`) so consumers get a best-effort import.
///
/// Returns `(system_uses, type_uses)` where:
/// - `system_uses`: std/core/alloc `use` paths like `std::collections::HashMap`
/// - `type_uses`: external `use` paths like `yog_api::registry::Mod`
pub fn collect_uses(exports: &[ExportItem], sources: &[String]) -> (Vec<String>, Vec<String>) {
    // 1 + 2: names that never need an import.
    let primitives: HashSet<&str> = PRIMITIVES.iter().copied().collect();

    // Names defined locally in the mod (exported items) — no import required.
    let local_names: HashSet<String> = exports.iter().map(|e| e.name.clone()).collect();

    // 3: gather every `use` statement from the mod's source so we can map a
    //    used type back to its import path.
    let mut import_paths: Vec<String> = Vec::new(); // e.g. "yog_api::registry::Mod"
    let mut system_imports: HashSet<String> = HashSet::new();
    let mut type_imports: HashSet<String> = HashSet::new();
    for line in sources.iter().flat_map(|src| src.lines()) {
        let t = line.trim();
        if !t.starts_with("use ") {
            continue;
        }
        let path_part = t["use ".len()..].trim_end_matches(';').trim();
        if path_part.is_empty() {
            continue;
        }
        // Skip malformed imports like `yog_api::{` without closing brace.
        if path_part.contains('{') && !path_part.contains('}') {
            continue;
        }
        // `use foo::{a, b}` / `use foo::*` — keep the prefix as a candidate.
        if is_system(path_part) {
            system_imports.insert(path_part.to_string());
        } else {
            type_imports.insert(path_part.to_string());
        }
        import_paths.push(path_part.to_string());
    }

    /// Resolve a bare type identifier to the most specific `use` path available.
    fn resolve_name(
        name: &str,
        import_paths: &[String],
        local_names: &HashSet<String>,
        primitives: &HashSet<&str>,
    ) -> Option<String> {
        // 1. primitive / prelude
        if primitives.contains(name) {
            return None;
        }
        if is_system(name) {
            return None;
        }
        // 2. defined in this crate
        if local_names.contains(name) {
            return None;
        }
        // 3. find a `use` path whose tail segment matches `name`.
        //    Prefer the shortest matching (most specific) import.
        let mut best: Option<(usize, String)> = None;
        for p in import_paths {
            let last = p.rsplit("::").next().unwrap_or(p);
            if last == name || last == "*" {
                let len = p.len();
                if best.as_ref().map(|(l, _)| len < *l).unwrap_or(true) {
                    best = Some((len, p.clone()));
                }
            }
        }
        best.map(|(_, p)| p)
    }

    // 4: scan exported item sources for type identifiers and resolve each.
    for item in exports {
        // Strip attributes / bodies to a single flat token-ish view, then pull
        // out CamelCase identifiers (types) and `crate::`/`super::`/`self::`
        // / absolute `::` paths.
        let body = &item.source;
        // Absolute paths (`::foo::Bar`, `yog_api::…`) are already fully qualified.
        for m in body.match_indices("::") {
            let start = m.0;
            // Find the path: leading `::` or `ident::`, then segments.
            let slice = &body[start..];
            let mut end = 0;
            let mut depth: i32 = 0;
            for (i, ch) in slice.char_indices() {
                match ch {
                    '<' => depth += 1,
                    '>' => depth -= 1,
                    ' ' | ',' | ';' | '{' | '}' | '(' | ')' | ':' | '[' | ']' | '.'
                        if depth == 0 =>
                    {
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
            let path = slice[..end].trim().trim_end_matches("::");
            if path.is_empty() {
                continue;
            }
            let first = path
                .trim_start_matches(':')
                .split("::")
                .next()
                .unwrap_or("");
            if is_system(path) {
                system_imports.insert(path.to_string());
            } else if first == "yog_api" || LOCAL_ROOTS.contains(&first) {
                type_imports.insert(path.to_string());
            }
        }
        // Bare CamelCase type names (struct/enum args, generic params, fields).
        let re_check: Vec<&str> = body
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| !s.is_empty())
            .collect();
        for tok in re_check {
            if tok
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
                && let Some(p) = resolve_name(tok, &import_paths, &local_names, &primitives)
            {
                if is_system(&p) {
                    system_imports.insert(p);
                } else {
                    type_imports.insert(p);
                }
            }
        }
    }

    // Normalize: merge every `a::b::{X, Y}` / `a::b::X` into one import per
    // module path, so a name pulled in by several files is imported once.
    fn normalize(vec: &mut Vec<String>) {
        let mut groups: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
            Default::default();
        let mut nested: Vec<String> = Vec::new();
        for p in vec.drain(..) {
            let p = p.trim().to_string();
            if let Some((prefix, rest)) = p.split_once("::{") {
                let inside = rest.strip_suffix('}').unwrap_or(rest);
                if inside.contains('{') {
                    nested.push(p);
                    continue;
                }
                let set = groups.entry(prefix.to_string()).or_default();
                set.extend(
                    inside
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                );
            } else if let Some((prefix, item)) = p.rsplit_once("::") {
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
            .map(|(prefix, items)| {
                if items.len() == 1 {
                    format!("{prefix}::{}", items.into_iter().next().unwrap())
                } else {
                    let items: Vec<String> = items.into_iter().collect();
                    format!("{prefix}::{{{}}}", items.join(", "))
                }
            })
            .collect();
        nested.sort();
        nested.dedup();
        keep.extend(nested);
        keep.sort();
        *vec = keep;
    }

    let mut system_vec: Vec<String> = system_imports.into_iter().collect();
    let mut type_vec: Vec<String> = type_imports.into_iter().collect();
    normalize(&mut system_vec);
    normalize(&mut type_vec);

    (system_vec, type_vec)
}

/// Strip `#[derive(...)]` lines from a Rust source item to avoid duplicates
/// when the generator adds its own derives.
pub fn strip_derive_attrs(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim().starts_with("#[derive("))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Generate a full interop wrapper for a `#[yog_export]` function.
///
/// Given the source of a function like:
/// ```text
/// pub fn register_widget(registry: &mut yog_api::Registry, def: WidgetDef) -> Result<(), String> { ... }
/// ```
///
/// Produces (a leading `&mut Registry`-shaped parameter is dropped — the
/// exporting mod reconstructs its own `Registry` from the API pointer it
/// captured at load time, so callers pass only the remaining, natural
/// arguments; multiple remaining arguments are tupled for serialization):
/// ```text
/// #![allow(non_snake_case)]
/// pub fn register_widget(__yog_arg0: WidgetDef) -> Result<(), String> {
///     // rkyv-serialize __yog_arg0, call via C-ABI slot, deserialize result
/// }
/// static __YOG_SLOT_REGISTER_WIDGET: OnceLock<...>;
/// #[unsafe(no_mangle)] pub unsafe extern "C" fn __yog_bind_register_widget(ptr) { ... }
/// ```
pub fn generate_fn_wrapper(mod_id: &str, name: &str, source: &str) -> String {
    // Flatten the whole source into one line before parsing — a signature
    // may wrap across several lines (one param per line is common rustfmt
    // output), and looking at only the first line would silently drop
    // every parameter after the first line break.
    let sig_line = source.lines().map(str::trim).collect::<Vec<_>>().join(" ");
    let sig_line = sig_line.as_str();

    // Parse: "pub fn name(args: ArgType) -> ReturnType"
    // Find the opening paren and its matching close paren
    let paren_start = sig_line.find('(');
    let paren_end = paren_start.and_then(|ps| {
        let after = &sig_line[ps..];
        let mut depth: i32 = 0;
        for (i, ch) in after.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(ps + i);
                    }
                }
                _ => {}
            }
        }
        None
    });

    // Split a parameter list on top-level commas (depth-aware: ignores commas
    // nested inside generics/tuples, e.g. `HashMap<String, f64>`).
    fn split_params(params: &str) -> Vec<&str> {
        let mut result = Vec::new();
        let mut depth: i32 = 0;
        let mut start = 0;
        for (i, ch) in params.char_indices() {
            match ch {
                '<' | '(' | '[' => depth += 1,
                '>' | ')' | ']' => depth -= 1,
                ',' if depth == 0 => {
                    result.push(params[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            }
        }
        let last = params[start..].trim();
        if !last.is_empty() {
            result.push(last);
        }
        result
    }

    // Parse every `name: Type` parameter (not just the first).
    let mut typed_params: Vec<(String, String)> =
        if let (Some(ps), Some(pe)) = (paren_start, paren_end) {
            let params = sig_line[ps + 1..pe].trim();
            split_params(params)
                .into_iter()
                .filter_map(|p| {
                    let parts: Vec<&str> = p.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

    // A leading `&mut Registry`-shaped parameter is never part of the public
    // signature or the serialized payload — the exporting mod reconstructs
    // its own `Registry` from the API pointer it captured at load time (see
    // `yog-interop`'s `#[yog_export]`), so callers never need to supply one.
    if typed_params
        .first()
        .map(|(_, ty)| ty.replace(' ', "").ends_with("Registry"))
        .unwrap_or(false)
    {
        typed_params.remove(0);
    }

    // The export's own parameter names (callers see them); a pattern that
    // isn't a plain name (`(a, b): (u32, u32)`) gets a made-up one.
    let arg_names: Vec<String> = typed_params
        .iter()
        .enumerate()
        .map(|(i, (pat, _))| {
            let name = pat.trim().trim_start_matches("mut ").trim();
            let plain = !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !name.starts_with(|c: char| c.is_ascii_digit());
            if plain { name.to_string() } else { format!("__yog_arg{i}") }
        })
        .collect();
    let data_types: Vec<&str> = typed_params.iter().map(|(_, ty)| ty.as_str()).collect();

    let params_sig = arg_names
        .iter()
        .zip(data_types.iter())
        .map(|(n, t)| format!("{n}: {t}"))
        .collect::<Vec<_>>()
        .join(", ");

    let tuple_expr = match arg_names.len() {
        0 => String::new(),
        1 => arg_names[0].clone(),
        _ => format!("({})", arg_names.join(", ")),
    };

    // Extract return type.
    // Find `->` AFTER the closing paren of parameters (paren_end).
    let output_type = match paren_end {
        Some(pe) => {
            let arrow_rel = if pe < sig_line.len() {
                sig_line[pe..].find("->")
            } else {
                None
            };
            match arrow_rel {
                Some(rel) => {
                    let arrow = pe + rel;
                    let rest: &str = sig_line[arrow + 2..].trim();
                    // Take until `{` or `;` handling angle bracket balance
                    let mut depth: i32 = 0;
                    let mut end = 0;
                    for (i, ch) in rest.char_indices() {
                        match ch {
                            '<' => depth += 1,
                            '>' => depth -= 1,
                            '{' | ';' => break,
                            _ => {}
                        }
                        if depth < 0 {
                            break;
                        }
                        if ch == '=' {
                            break;
                        }
                        end = i + ch.len_utf8();
                    }
                    rest[..end].trim().to_string()
                }
                None => "()".to_string(),
            }
        }
        None => "()".to_string(),
    };

    // Statics use SCREAMING_SNAKE_CASE per Rust convention; the bind fn stays
    // snake_case since it's a function, not a static.
    let slot_name = format!("__YOG_SLOT_{}", name.to_uppercase());
    let bind_name = format!("__yog_bind_{}", name);

    // Escape { and } for use in format! — types like `Result<(), String>` contain
    // curly braces that would be interpreted as format specifiers.
    let output_type_fmt = output_type.replace('{', "{{").replace('}', "}}");
    let params_sig_fmt = params_sig.replace('{', "{{").replace('}', "}}");
    let slot_name_fmt = slot_name.replace('{', "{{").replace('}', "}}");
    let bind_name_fmt = bind_name.replace('{', "{{").replace('}', "}}");

    // Build the C-ABI function pointer type
    let wrapper_ty = "::yog_api::YogExportFn".to_string();

    let serialize_block = if tuple_expr.is_empty() {
        "let input_bytes: Vec<u8> = Vec::new();".to_string()
    } else {
        format!(
            "let aligned = ::yog_api::rkyv::to_bytes::<::yog_api::rkyv::rancor::Error>(\
                &{tuple_expr}\
            ).unwrap_or_default();\n            \
             let input_bytes: Vec<u8> = aligned.to_vec();"
        )
    };

    format!(
        r#"#[allow(non_snake_case)]
pub fn {name}({params_sig_fmt}) -> {output_type_fmt} {{
    {serialize_block}
    let f = {slot_name_fmt}.get()
        .expect(concat!("yog: export '", "{name}", "' not bound"));
    let output = ::yog_api::__interop_call(*f, &input_bytes, "{name}");
    ::yog_api::rkyv::from_bytes::<{output_type_fmt}, ::yog_api::rkyv::rancor::Error>(&output)
        .expect(concat!("yog: the result of export '", "{name}", "' didn't decode"))
}}

static {slot_name_fmt}: ::std::sync::OnceLock<{wrapper_ty}> = ::std::sync::OnceLock::new();

#[doc(hidden)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn {bind_name_fmt}(ptr: *const ::std::os::raw::c_void) {{
    let f: {wrapper_ty} = unsafe {{ ::std::mem::transmute(ptr) }};
    {slot_name_fmt}.set(f).ok();
}}

// Queues the binding when the consuming mod's library loads; `export_mod!`
// binds it once `{mod_id}` has exported `{name}`.
#[::yog_api::ctor::ctor(unsafe, crate_path = ::yog_api::ctor)]
#[doc(hidden)]
fn __yog_import_ctor_{name}() {{
    ::yog_api::__yog_import_registry().lock().unwrap().push(::yog_api::YogImportEntry {{
        mod_id: "{mod_id}",
        symbol: "{name}",
        bind_fn: {bind_name_fmt} as *const () as usize,
    }});
}}
"#
    )
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
    pub name: &'a str,
    pub version: &'a str,
    pub edition: &'a str,
    /// The mod's authors, joined with `", "`.
    pub authors: &'a str,
    pub license: &'a str,
    pub mod_id: &'a str,
    pub yog_api: &'a str,
    pub rkyv: &'a str,
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
rkyv = "{rkyv}"

[lib]
crate-type = ["cdylib", "lib"]
"#,
        name = m.name,
        version = m.version,
        edition = m.edition,
        license = m.license,
        mod_id = m.mod_id,
        yog_api = m.yog_api,
        rkyv = m.rkyv,
    )
}

/// The exports crate's `src/lib.rs`: the imports from [`collect_uses`],
/// structs and enums with rkyv derives so consumers get the serialisable
/// types, and an interop wrapper per fn (the same pattern as `import!`).
pub fn lib_rs(
    mod_id: &str,
    version: &str,
    exports: &[ExportItem],
    system_uses: &[String],
    type_uses: &[String],
) -> String {
    let mut lib_rs = format!(
        "//! Auto-generated exports from `{mod_id}` v{version}.\n\
         //! DO NOT EDIT — generated by `yog publish exports`.\n\n\
         #![allow(unused_imports)]\n\n"
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

    for item in exports {
        match item.kind {
            ExportKind::Struct | ExportKind::Enum => {
                // Strip existing #[derive(...)] to avoid duplicates.
                let _ = writeln!(
                    lib_rs,
                    "#[derive(::yog_api::rkyv::Archive, ::yog_api::rkyv::Serialize, ::yog_api::rkyv::Deserialize, Debug, Clone, PartialEq)]\n{}",
                    strip_derive_attrs(&item.source)
                );
            }
            ExportKind::Fn => lib_rs.push_str(&generate_fn_wrapper(mod_id, &item.name, &item.source)),
        }
    }
    // Types are already in crate root, no extra re-export needed.
    lib_rs
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
use yog_api::prelude::*;

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
        let names: Vec<(&str, ExportKind)> = items.iter().map(|i| (i.name.as_str(), i.kind)).collect();
        assert_eq!(names, [("Pipe", ExportKind::Struct), ("Kind", ExportKind::Enum), ("send", ExportKind::Fn), ("count", ExportKind::Fn)]);
        assert!(!items.iter().any(|i| i.source.contains("#[yog_export]")), "the attribute is stripped");
        assert!(items[2].source.contains("amount: u32"), "a wrapped signature is whole");
    }

    #[test]
    fn wrappers_call_through_one_helper_and_drop_the_registry() {
        let items = scan(SRC);
        let w = generate_fn_wrapper("yog-pipes", "send", &items[2].source);
        assert!(w.contains("pub fn send(pipe: Pipe, amount: u32) -> Option<String>"), "callers see the export's names: {w}");
        assert!(w.contains("::yog_api::__interop_call("), "{w}");
        assert!(w.contains("static __YOG_SLOT_SEND"), "{w}");
        let none = generate_fn_wrapper("yog-pipes", "count", &items[3].source);
        assert!(none.contains("pub fn count() -> u64"), "{none}");
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
        assert!(items[0].source.starts_with("/// Opens"), "{}", items[0].source);
        assert!(items[0].source.trim_end().ends_with('}') && items[0].source.contains("format!(\"{{{s}\")"), "{}", items[0].source);
        assert!(!items[0].source.contains("yog_export"));
    }
}
