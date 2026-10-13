//! `yog` — the Yog mod build tool.
//!
//! Mod authors never touch Cargo.toml.  They write `yog.toml`, call `yog build`,
//! and get a cross-platform `.yog` artifact.  The Cargo workspace is generated
//! in `.yog-build/` and is completely hidden from the author.
//!
//! Commands:
//!   yog new <name>   — scaffold a new mod project
//!   yog build        — compile + package the current mod
//!   yog setup        — check/install build dependencies
//!   yog doctor       — report build dependencies (read-only; `--json` for tools)
//!   yog check        — generate `.yog-build/` and `cargo check` the mod
//!   yog ui gen [..]  — generate `.yog-build/generated/**/*.g.rs` from `.yui` screens
//!   yog res check    — check `resources/**/*.yres`, generate their accessors
//!   yog help         — show usage

mod content;
mod yres;
mod yui;
mod yui_vm;

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        // `yog build` — stripped, no debug info (the default, unchanged).
        // `yog build debug` — same build, but with DWARF debug info kept
        // in the packaged native instead of stripped, for `yog-symbols`/
        // `yog-debugger` to load. Still a `--release` cargo build either
        // way — "debug" here names what ships in the artifact, not the
        // cargo profile.
        Some("build") => {
            let debug = args.get(2).map(String::as_str) == Some("debug");
            build(debug).map(|_| ())
        }
        Some("check") => check(),
        Some("designer-lib") => designer_lib(),
        Some("new") => new_mod(args.get(2).map(String::as_str)),
        Some("setup") => setup(),
        Some("doctor") => {
            let report = doctor();
            if args[2..].iter().any(|a| a == "--json") {
                println!("{}", report.to_json());
            } else {
                report.print();
            }
            Ok(())
        }
        Some("-V") | Some("--version") | Some("version") => {
            println!("yog {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("add") => {
            let is_mod = args[2..].iter().any(|a| a == "--mod");
            let name = args[2..].iter().find(|a| a.as_str() != "--mod");
            add_dep(name.map(String::as_str), is_mod)
        }
        Some("remove") => {
            let is_mod = args[2..].iter().any(|a| a == "--mod");
            let name = args[2..].iter().find(|a| a.as_str() != "--mod");
            remove_dep(name.map(String::as_str), is_mod)
        }
        Some("run") => {
            let debug = args[2..].iter().any(|a| a == "--debug");
            match args[2..].iter().find(|a| !a.starts_with("--")) {
                Some(name) => run_config(name, debug),
                None => Err(
                    "usage: yog run <config_name> [--debug]  (see [run.<config_name>] in yog.toml)"
                        .into(),
                ),
            }
        }
        Some("publish") => {
            let dry_run = args.iter().any(|a| a == "--dry-run");
            match args.get(2).map(String::as_str) {
                Some("exports") => publish_exports(dry_run),
                Some(other) => Err(format!("unknown publish subcommand: {other}")),
                None => Err("usage: yog publish exports [--dry-run]".into()),
            }
        }
        Some("res") => match args.get(2).map(String::as_str) {
            Some("check") => res_check(),
            _ => Err("usage: yog res check".into()),
        },
        Some("content") => match args.get(2).map(String::as_str) {
            Some("check") => content_check(),
            _ => Err("usage: yog content check".into()),
        },
        Some("import") => content::import_command(&args[2..]),
        Some("ui") => match args.get(2).map(String::as_str) {
            Some("gen") => ui_gen(&args[3..]),
            _ => Err("usage: yog ui gen [file.yui ...]  (no files: every src/**/*.yui)".into()),
        },
        Some("-h") | Some("--help") | Some("help") | None => {
            print_usage();
            return;
        }
        Some(other) => Err(format!("unknown command: {other}")),
    };
    if let Err(e) = result {
        eprintln!("yog: error: {e}");
        std::process::exit(1);
    }
}

fn print_usage() {
    println!(
        "yog — Yog mod build tool\n\n\
         Usage: yog <command> [args]\n\n\
         Commands:\n\
         \x20 new <name>          Create a new mod project in ./<name>/\n\
         \x20 build               Compile the current mod and package it as artifacts/<id>.yog\n\
         \x20 build debug         Same, but keep debug info in the native (for yog-debugger)\n\
         \x20 check               Generate .yog-build/ and `cargo check` the mod (host only)\n\
         \x20 designer-lib        Build the mod's library for this machine (Yog-IDLE's designer); prints its path\n\
         \x20 setup               Check and install build dependencies (Rust, Zig, targets)\n\
         \x20 doctor [--json]     Report build dependencies without changing anything\n\
         \x20 version             Print the yog CLI version\n\
         \x20 add <crate>         Add a Rust dependency to yog.toml\n\
         \x20 remove <crate>      Remove a dependency from yog.toml\n\
         \x20 run <config>        Build, export, and launch a [run.<config>] dev instance\n\
         \x20 run <config> --debug  Same, built with debug info kept (see `build debug`)\n\
         \x20 publish exports     Publish this mod's exported API to crates.io\n\
         \x20 ui gen [files]      Generate .yog-build/generated/**/*.g.rs from .yui screens (build does this too)\n\
         \x20 res check           Validate resources/**/*.yres, regenerate accessors, list untranslated texts\n\
         \x20 content check       Validate content/**/*.yblock|*.yitem (blocks/items), regenerate yog-content.g.rs\n\
         \x20 import resourcepack <zip|dir> [--model <id>|--all]\n\
         \x20                     Turn a resource pack's block/item models into content/ files\n\
         \x20 import bbmodel <file.bbmodel> [--id <name>] [--item]\n\
         \x20                     Turn a Blockbench model (Java block/item) into a content/ file\n\
         \x20 help                Show this message\n\n\
         Mod projects use yog.toml instead of Cargo.toml.\n\
         Cross-compilation requires cargo-zigbuild + zig (yog setup installs them).\n\
         Set YOG_TARGETS=linux-x86_64 (comma-separated tags/triples) to build only those platforms."
    );
}

// ── yog.toml ─────────────────────────────────────────────────────────────────

/// Parsed content of a `yog.toml` project file.
#[derive(Debug)]
struct YogToml {
    id: String,
    name: String,
    version: String,
    description: String,
    /// Translation keys of `name`/`description` when yog.toml gives them
    /// as `@<Set>.<name>` resources (see `resolve_metadata`).
    name_key: Option<String>,
    description_key: Option<String>,
    /// The mod's icon, an image source (`icon = "@Icons.logo"` resolved).
    icon: Option<String>,
    authors: Vec<String>,
    license: String,
    /// License for the generated `yog publish exports` bindings crate.
    /// Defaults to "MIT OR Apache-2.0" regardless of the mod's own license
    /// (AGPL propagating onto every consumer's dependency tree would kill
    /// adoption of the exports crate) — set `[mod] exports_license = "..."`
    /// to opt into matching the mod's own license instead.
    exports_license: Option<String>,
    edition: Option<String>,
    /// Optional: path to yog-api for local/monorepo development.
    /// Set via [dev] yog_api_path = "..."  or YOG_API_PATH env var.
    yog_api_path: Option<String>,
    /// Optional: pin yog-api version as "yog_api = \"X.Y\"" in [dependencies].
    yog_api_version: Option<String>,
    /// User-declared dependencies from [dependencies] section — plain
    /// crates.io crates, used exactly as written.
    dependencies: Vec<(String, String)>,
    /// Other Yog mods this mod depends on, from `[dependencies.exports]`.
    /// Each entry `name = spec` maps to that mod's generated exports crate:
    /// `{name} = { package = "{name}-exports", ...spec }` — so mod code
    /// imports it under its own name (`use {name}::...`), same as a direct
    /// dependency, and two mod dependencies never collide under a shared
    /// alias (unlike the old name-contains-hyphen heuristic this replaced).
    mod_dependencies: Vec<(String, String)>,
    /// Named dev-instance launch configs from `[run.<name>]` sections.
    run_configs: Vec<RunConfig>,
}

/// A `[run.<name>]` section: where to drop the built artifact and what to
/// launch afterwards, for a full `yog run <name>` dev-instance workflow.
#[derive(Debug, Default, Clone)]
struct RunConfig {
    name: String,
    /// Optional human-readable label (`name = "..."` inside `[run.X]`), for
    /// editors to show/edit instead of the raw dotted section-key id. Falls
    /// back to `name` (the id) when absent.
    display_name: Option<String>,
    /// Directory the built `.yog` gets copied into (e.g. an instance's `yog-mods/`).
    /// Relative paths resolve against the mod project root.
    export_dir: Option<String>,
    /// Executable to launch after export (e.g. `java`, a wrapper script, `./gradlew`).
    command: Option<String>,
    /// Arguments passed to `command`, in order.
    args: Vec<String>,
    /// Working directory for `command`. Relative paths resolve against the project root.
    cwd: Option<String>,
    /// Extra environment variables as `KEY=VALUE` pairs.
    env: Vec<(String, String)>,
}

impl YogToml {
    fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        parse_yog_toml(&text)
    }

    /// Resolve where yog-api lives.  Priority:
    ///   1. YOG_API_PATH env var
    ///   2. [dev] yog_api_path in yog.toml
    ///   3. yog_api_version in [dependencies] (e.g. "0.5")
    ///   4. YOG_API_VERSION env var
    ///   5. latest from crates.io
    fn api_dep(&self) -> String {
        if let Ok(p) = std::env::var("YOG_API_PATH") {
            // Resolve to absolute so it works from any subdirectory
            let abs = PathBuf::from(&p)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&p));
            return format!("yog-api = {{ path = {:?} }}", abs.to_string_lossy());
        }
        if let Some(p) = &self.yog_api_path {
            return format!("yog-api = {{ path = {p:?} }}");
        }
        let version = {
            let env_version = std::env::var("YOG_API_VERSION").ok();
            self.yog_api_version
                .as_deref()
                .or(env_version.as_deref())
                .map(|v| v.to_owned())
                .unwrap_or_else(latest_yog_api_version)
        };
        format!("yog-api = {:?}", version)
    }
}

fn parse_yog_toml(text: &str) -> Result<YogToml, String> {
    let mut section = "";
    let mut id = None::<String>;
    let mut name = None::<String>;
    let mut version = None::<String>;
    let mut description = None::<String>;
    let mut icon = None::<String>;
    let mut authors: Vec<String> = Vec::new();
    let mut license = None::<String>;
    let mut exports_license = None::<String>;
    let mut edition = None::<String>;
    let mut yog_api_path = None::<String>;
    let mut dependencies: Vec<(String, String)> = Vec::new();
    let mut mod_dependencies: Vec<(String, String)> = Vec::new();
    let mut run_configs: HashMap<String, RunConfig> = HashMap::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']');
            continue;
        }
        match section {
            "mod" => {
                if let Some(v) = field(line, "id") {
                    id = Some(v);
                }
                if let Some(v) = field(line, "name") {
                    name = Some(v);
                }
                if let Some(v) = field(line, "version") {
                    version = Some(v);
                }
                if let Some(v) = field(line, "description") {
                    description = Some(v);
                }
                if let Some(v) = field(line, "icon") {
                    icon = Some(v);
                }
                if let Some(v) = field(line, "license") {
                    license = Some(v);
                }
                if let Some(v) = field(line, "exports_license") {
                    exports_license = Some(v);
                }
                if let Some(v) = field(line, "edition") {
                    edition = Some(v);
                }
                if line.trim_start().starts_with("authors") {
                    authors = parse_string_array(line);
                }
            }
            "dev" => {
                if let Some(v) = field(line, "yog_api_path") {
                    yog_api_path = Some(v);
                }
            }
            "dependencies" => {
                if let Some((name, spec)) = parse_dep_line(line) {
                    dependencies.push((name, spec));
                }
            }
            "dependencies.exports" => {
                if let Some((name, spec)) = parse_dep_line(line) {
                    mod_dependencies.push((name, spec));
                }
            }
            _ if section.starts_with("run.") => {
                let run_name = &section["run.".len()..];
                let cfg = run_configs
                    .entry(run_name.to_string())
                    .or_insert_with(|| RunConfig {
                        name: run_name.to_string(),
                        ..Default::default()
                    });
                if let Some(v) = field(line, "name") {
                    cfg.display_name = Some(v);
                }
                if let Some(v) = field(line, "export_dir") {
                    cfg.export_dir = Some(v);
                }
                if let Some(v) = field(line, "command") {
                    cfg.command = Some(v);
                }
                if let Some(v) = field(line, "cwd") {
                    cfg.cwd = Some(v);
                }
                if line.trim_start().starts_with("args") {
                    cfg.args = parse_string_array(line);
                }
                if line.trim_start().starts_with("env") {
                    cfg.env = parse_string_array(line)
                        .into_iter()
                        .filter_map(|kv| {
                            kv.split_once('=')
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                        })
                        .collect();
                }
            }
            _ => {}
        }
    }
    let run_configs: Vec<RunConfig> = run_configs.into_values().collect();

    let id = id.ok_or("yog.toml: missing [mod] id")?;
    // Extract yog_api_version / yog-api from dependencies if there
    let mut yog_api_version = None;
    let mut filtered_deps = Vec::new();
    for (name, spec) in dependencies {
        if matches!(name.as_str(), "yog_api_version" | "yog-api" | "yog_api") {
            yog_api_version = Some(spec.trim_matches('"').to_string());
        } else {
            filtered_deps.push((name, spec));
        }
    }
    Ok(YogToml {
        name: name.unwrap_or_else(|| id.clone()),
        version: version.unwrap_or_else(|| "0.1.0".into()),
        description: description.unwrap_or_default(),
        name_key: None,
        description_key: None,
        icon,
        authors,
        license: license.unwrap_or_else(|| "MIT OR Apache-2.0".into()),
        exports_license,
        edition,
        yog_api_path,
        yog_api_version,
        dependencies: filtered_deps,
        mod_dependencies,
        run_configs,
        id,
    })
}

/// Parse a dependency line like `foo = "1.2"` or `bar = { version = "1", features = ["x"] }`
fn parse_dep_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.starts_with('#') || trimmed.is_empty() || trimmed.starts_with('[') {
        return None;
    }
    let eq_pos = trimmed.find('=')?;
    let name = trimmed[..eq_pos].trim().to_string();
    let spec = trimmed[eq_pos + 1..].trim().to_string();
    Some((name, spec))
}

/// Parse `key = "value"` returning the unquoted value.
fn field(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    Some(rest.trim_matches('"').to_string())
}

/// Parse `key = ["a", "b"]` into a Vec<String>.
fn parse_string_array(line: &str) -> Vec<String> {
    let inner = line
        .find('[')
        .and_then(|s| line.rfind(']').map(|e| &line[s + 1..e]));
    inner
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ── yog new ───────────────────────────────────────────────────────────────────

fn new_mod(name: Option<&str>) -> Result<(), String> {
    let name = name.ok_or("usage: yog new <name>")?;
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return Err(format!("invalid mod name: {name:?}"));
    }
    let root = PathBuf::from(name);
    if root.exists() {
        return Err(format!("directory {name} already exists"));
    }

    std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;

    // yog.toml
    let yog_toml = format!(
        r#"[mod]
id          = "{name}"
name        = "{display}"
version     = "0.1.0"
description = "A Yog mod."
authors     = ["Your Name"]
license     = "MIT OR Apache-2.0"

# Uncomment for local/monorepo development:
# [dev]
# yog_api_path = "../path/to/yog-api"

# Uncomment to enable `yog run <name>` — builds, drops the .yog into
# export_dir, then launches `command` with `args`/`cwd`/`env`. Add as many
# [run.<name>] sections as you like (e.g. one per loader/version).
# [run.dev]
# export_dir = "../my-test-instance/yog-mods"
# command    = "java"
# args       = ["-jar", "server.jar", "--nogui"]
# cwd        = "../my-test-instance"
# env        = ["JAVA_OPTS=-Xmx4G"]
"#,
        name = name,
        display = to_display_name(name),
    );
    write_file(&root.join("yog.toml"), yog_toml.as_bytes())?;

    // src/lib.rs
    let lib_rs = format!(
        r#"use yog_api::{{Mod, Registry}};

pub struct {struct_name};

impl Mod for {struct_name} {{
    fn register(registry: &mut Registry) {{
        registry.on_server_started(|srv| {{
            srv.broadcast("{name} loaded!");
        }});
    }}
}}

yog_api::export_mod!({struct_name});
"#,
        name = name,
        struct_name = to_struct_name(name),
    );
    write_file(&root.join("src/lib.rs"), lib_rs.as_bytes())?;

    // .gitignore
    write_file(
        &root.join(".gitignore"),
        b".yog-build/\ntarget/\nartifacts/\n*.designer.rs\n",
    )?;

    eprintln!("==> created {name}/");
    eprintln!("    yog.toml       ← edit mod metadata here");
    eprintln!("    src/lib.rs     ← write your mod here");
    eprintln!();
    eprintln!("Next: cd {name} && yog build");
    Ok(())
}

fn to_display_name(id: &str) -> String {
    id.replace('-', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().to_string())
                .unwrap_or_default()
                + c.as_str()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn to_struct_name(id: &str) -> String {
    id.split(['-', '_'])
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().to_string())
                .unwrap_or_default()
                + c.as_str()
        })
        .collect()
}

/// Cargo derives a lib crate name from the package `name` by turning `-` into
/// `_`, but keeps case — so a mod `id` like "HexMod-Yog" becomes the invalid
/// `HexMod_Yog` and rustc warns on every build. `id` is a free-form project
/// identifier (not required to be a valid Rust ident), so the generated
/// wrapper pins `[lib] name` to a proper snake_case form instead.
fn to_snake_name(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for c in id.chars() {
        if c == '-' || c == '_' {
            if !out.ends_with('_') {
                out.push('_');
            }
        } else if c.is_uppercase() {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// ── yog build ─────────────────────────────────────────────────────────────────

/// A platform Yog can target.
struct Target {
    triple: &'static str,
    tag: &'static str,
    os: &'static str,
}

const TARGETS: &[Target] = &[
    Target {
        triple: "x86_64-unknown-linux-gnu",
        tag: "linux-x86_64",
        os: "linux",
    },
    Target {
        triple: "aarch64-unknown-linux-gnu",
        tag: "linux-aarch64",
        os: "linux",
    },
    Target {
        triple: "x86_64-pc-windows-gnu",
        tag: "windows-x86_64",
        os: "windows",
    },
    Target {
        triple: "x86_64-apple-darwin",
        tag: "macos-x86_64",
        os: "macos",
    },
    Target {
        triple: "aarch64-apple-darwin",
        tag: "macos-aarch64",
        os: "macos",
    },
];

/// Builds and packages the mod in the current directory. Returns the parsed
/// `yog.toml` metadata and the path to the produced `.yog` artifact, so
/// callers like `run_config` can reuse them without re-parsing/rebuilding.
///
/// `debugging_symbols` (`yog build debug` / `yog run <name> --debug` at the
/// CLI surface): still a `--release` build (same output layout `package()`
/// expects), but unoptimized and with debug info retained instead of
/// stripped — for `yog-symbols`/`yog-debugger` to have something to load
/// and step through line by line. No separate artifact layout: the native
/// this produces is simply larger, slower and symbol-ful, packaged the same
/// way.
/// A mod project made ready for cargo: `.yog-build/` (Cargo.toml, generated
/// `.yui` screen code, toolchain, lock) written, translations compiled.
struct Prepared {
    root: PathBuf,
    meta: YogToml,
    build_dir: PathBuf,
    resources: yres::Compiled,
    /// `content/**/*.yblock|*.yitem` — blocks and items.
    content: content::Compiled,
}

/// What every command that compiles does first (`yog build`, `yog check`):
/// regenerates `.yog-build/` from `yog.toml` and `src/**/*.yui`, and checks
/// the translations.
fn prepare() -> Result<Prepared, String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let yog_toml_path = root.join("yog.toml");
    if !yog_toml_path.exists() {
        return Err("no yog.toml found in the current directory".into());
    }

    // Enforce: never create Cargo.toml in the mod root
    let root_cargo = root.join("Cargo.toml");
    if root_cargo.exists() {
        return Err("Cargo.toml found in mod root — this is forbidden. Use yog.toml only. Remove Cargo.toml and retry.".into());
    }

    let mut meta = YogToml::read(&yog_toml_path)?;

    // Resolve yog_api_path relative to project root (Cargo.toml lives one level deeper)
    if let Some(rel) = &meta.yog_api_path {
        let abs = root
            .join(rel)
            .canonicalize()
            .unwrap_or_else(|_| root.join(rel));
        meta.yog_api_path = Some(abs.to_string_lossy().into_owned());
    }

    // Resources first: `.yui` screens and yog.toml (name, description,
    // icon) refer to them.
    let resources = yres::compile(&root, &meta.id)?;
    for w in &resources.warnings {
        eprintln!("    warning: {w}");
    }
    yres::write_sources(&root, &resources)?;
    resolve_metadata(&mut meta, &resources.refs)?;
    yui::set_resource_refs(&resources.refs);

    // Blocks and items: models, states, loot, lang and their Rust glue.
    let content = content::compile(&root, &meta.id, &resources.refs)?;
    for w in &content.warnings {
        eprintln!("    warning: {w}");
    }
    content::write_source(&root, &content)?;

    // Generate .yog-build/Cargo.toml from yog.toml
    let build_dir = root.join(".yog-build");
    std::fs::create_dir_all(&build_dir).map_err(|e| e.to_string())?;
    let cargo_toml = generate_cargo_toml(&meta);
    write_file(&build_dir.join("Cargo.toml"), cargo_toml.as_bytes())?;

    let designer_path = root.join("src").join("lib.designer.rs");
    match generate_lib_facade(&meta, &root)? {
        Some(facade) => {
            // Written as a sibling of the real `src/lib.rs` (not under
            // `.yog-build/`) so its `mod foo;` declarations still resolve.
            write_file(&designer_path, facade.as_bytes())?;
        }
        None => {
            // No `[dependencies.exports]` anymore — a `lib.designer.rs` left
            // over from a previous build is dead weight (unreferenced by
            // `[lib] path`, which now points straight at `src/lib.rs`) and,
            // worse, stale: it won't track further edits to `src/lib.rs` at
            // all. `yog build` owns this file end to end, so it cleans up
            // after itself rather than leaving it to rot.
            if designer_path.exists() {
                std::fs::remove_file(&designer_path).map_err(|e| e.to_string())?;
            }
        }
    }

    let screens = yui::generate_all(&root)?;
    if screens > 0 {
        eprintln!(
            "    generated code for {screens} .yui screen(s) into {}/",
            yui::GENERATED_DIR
        );
    }

    configure_editor(&root).map_err(|e| format!("configuring .vscode/settings.json: {e}"))?;
    ensure_gitignore_entry(&root, "*.designer.rs")
        .map_err(|e| format!("updating .gitignore: {e}"))?;

    write_file(
        &build_dir.join("rust-toolchain.toml"),
        generate_rust_toolchain().as_bytes(),
    )?;

    // Restore yog.lock → .yog-build/Cargo.lock so cargo respects pinned versions
    let yog_lock = root.join("yog.lock");
    let cargo_lock = build_dir.join("Cargo.lock");
    if yog_lock.exists() && !cargo_lock.exists() {
        std::fs::copy(&yog_lock, &cargo_lock).map_err(|e| e.to_string())?;
    }
    Ok(Prepared {
        root,
        meta,
        build_dir,
        resources,
        content,
    })
}

/// `yog check` — `cargo check` of the mod for the host platform, after the
/// same generation `yog build` does; nothing is packaged. Quick feedback
/// (and fresh `.yog-build/generated/` for rust-analyzer) without a build.
fn check() -> Result<(), String> {
    let Prepared {
        root, build_dir, ..
    } = prepare()?;
    let status = Command::new("cargo")
        .current_dir(&build_dir)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .arg("check")
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo check failed ({status})"));
    }
    eprintln!("==> check OK");
    Ok(())
}

/// `yog designer-lib` — the mod's library built for this machine (debug
/// profile, `target/designer/`), for Yog-IDLE's screen designer host to
/// load and draw the mod's custom `.yui` components with. Cargo's output
/// goes to stderr; the library's path is the one line on stdout.
fn designer_lib() -> Result<(), String> {
    let Prepared {
        root,
        meta,
        build_dir,
        ..
    } = prepare()?;
    let target_dir = root.join("target").join("designer");
    let status = Command::new("cargo")
        .current_dir(&build_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args(["build", "--lib", "--color", "never"])
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo build failed ({status})"));
    }
    let lib = target_dir
        .join("debug")
        .join(lib_filename(&meta.id, std::env::consts::OS));
    if !lib.is_file() {
        return Err(format!("built, but {} is missing", lib.display()));
    }
    println!("{}", lib.display());
    Ok(())
}

fn build(debugging_symbols: bool) -> Result<(YogToml, PathBuf), String> {
    let Prepared {
        root,
        meta,
        build_dir,
        resources,
        content,
    } = prepare()?;
    let yog_lock = root.join("yog.lock");

    let builder = Builder::detect();
    eprintln!(
        "==> building {} {} with `cargo {}`",
        meta.id,
        meta.version,
        builder.subcmd()
    );

    let selected = selected_targets();
    let wanted: Vec<&Target> = TARGETS
        .iter()
        .filter(|t| match &selected {
            Some(sel) => sel.iter().any(|s| s == t.tag || s == t.triple),
            None => true,
        })
        .collect();
    install_targets(&wanted);
    let installed = installed_targets();
    let mut bundled: Vec<(String, PathBuf)> = Vec::new();

    for t in wanted {
        if !installed.iter().any(|s| s == t.triple) {
            eprintln!(
                "    skip {} (rustup target not installed; run: yog setup)",
                t.tag
            );
            continue;
        }
        match builder.build(&build_dir, t.triple, &root, debugging_symbols) {
            Ok(()) => {
                let lib = lib_filename(&meta.id, t.os);
                // Cargo puts output under project-root/target/<triple>/release/ (we set CARGO_TARGET_DIR)
                let path = root
                    .join("target")
                    .join(t.triple)
                    .join("release")
                    .join(&lib);
                if path.exists() {
                    eprintln!("    built {}", t.tag);
                    bundled.push((t.tag.to_string(), path));
                } else {
                    eprintln!("    skip {} (built but output not found: {})", t.tag, lib);
                }
            }
            Err(_) => eprintln!("    skip {} (build failed)", t.tag),
        }
    }

    if bundled.is_empty() {
        return Err(
            "no platform built — run `yog setup` to install cross-compilation tools".into(),
        );
    }

    // Save Cargo.lock as yog.lock
    let new_lock = build_dir.join("Cargo.lock");
    if new_lock.exists() {
        std::fs::copy(&new_lock, &yog_lock).map_err(|e| e.to_string())?;
    }

    if resources.files > 0 {
        eprintln!(
            "    compiled {} .yres resource file(s) into {} asset file(s)",
            resources.files,
            resources.outputs.len()
        );
    }
    if content.blocks + content.items > 0 {
        eprintln!(
            "    compiled {} block(s) and {} item(s) from {}/",
            content.blocks,
            content.items,
            content::CONTENT_DIR
        );
    }
    let mut compiled_assets = resources.outputs;
    content::merge_outputs(&root, &mut compiled_assets, content)?;
    let assets = gather_assets(&root, compiled_assets);
    if !assets.is_empty() {
        eprintln!("    bundled {} asset file(s)", assets.len());
    }

    let artifacts = root.join("artifacts");
    std::fs::create_dir_all(&artifacts).map_err(|e| e.to_string())?;
    let out = artifacts.join(format!("{}.yog", meta.id));
    package(&out, &meta, &bundled, &assets)?;

    let tags: Vec<&str> = bundled.iter().map(|(t, _)| t.as_str()).collect();
    eprintln!("==> packaged {} [{}]", out.display(), tags.join(", "));
    Ok((meta, out))
}

// ── yog ui gen ────────────────────────────────────────────────────────────────

/// `yog ui gen [files...]` — regenerates `.yog-build/generated/**/*.g.rs`
/// for the given `.yui` screen files (each in the project whose `yog.toml`
/// is above it), or for every `src/**/*.yui` of the current project.
/// `yog build`/`yog check` do the same before compiling; this is for editors
/// (Yog-IDLE runs it on save) and for checking a screen without a build.
fn ui_gen(files: &[String]) -> Result<(), String> {
    if files.is_empty() {
        let root = std::env::current_dir().map_err(|e| e.to_string())?;
        if !root.join("yog.toml").is_file() {
            return Err("no yog.toml found in the current directory".into());
        }
        load_resource_refs(&root);
        for file in yui::find_yui_files(&root.join("src")) {
            let out = yui::generate_file(&root, &file)?;
            eprintln!("==> {} -> {}", file.display(), out.display());
        }
        return Ok(());
    }
    for file in files.iter().map(PathBuf::from) {
        let root = yui::project_root(&file).ok_or_else(|| {
            format!(
                "{}: not inside a yog project (no yog.toml above it)",
                file.display()
            )
        })?;
        load_resource_refs(&root);
        let out = yui::generate_file(&root, &file)?;
        eprintln!("==> {} -> {}", file.display(), out.display());
    }
    Ok(())
}

/// Makes the project's `.yres` resources available to `.yui` generation
/// (`text="@Strings.title"`). Broken resources are reported but don't stop
/// the screen from generating — `yog build` fails on them anyway.
fn load_resource_refs(root: &Path) {
    let refs = YogToml::read(&root.join("yog.toml"))
        .and_then(|meta| yres::compile(root, &meta.id))
        .map(|c| c.refs)
        .unwrap_or_else(|e| {
            eprintln!("warning: {e}");
            yres::Refs::default()
        });
    yui::set_resource_refs(&refs);
}

// ── yog res check ─────────────────────────────────────────────────────────────

/// `yog res check` — what `yog build` does with `resources/**/*.yres`,
/// without building: errors with file:line, untranslated texts per locale,
/// and fresh accessors in `.yog-build/generated/resources/`.
fn res_check() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut meta = YogToml::read(&root.join("yog.toml"))?;
    let compiled = yres::compile(&root, &meta.id)?;
    yres::write_sources(&root, &compiled)?;
    resolve_metadata(&mut meta, &compiled.refs)?;
    for w in &compiled.warnings {
        eprintln!("warning: {w}");
    }
    eprintln!(
        "==> {} resource file(s) OK{}",
        compiled.files,
        if compiled.warnings.is_empty() {
            String::new()
        } else {
            format!(", {} warning(s)", compiled.warnings.len())
        }
    );
    Ok(())
}

// ── yog content check ────────────────────────────────────────────────────────

/// `yog content check` — what `yog build` does with `content/**/*.yblock`
/// and `*.yitem`, without building: errors with file:line, a fresh
/// `.yog-build/generated/yog-content.g.rs`.
fn content_check() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let meta = YogToml::read(&root.join("yog.toml"))?;
    let refs = yres::compile(&root, &meta.id)
        .map(|c| c.refs)
        .unwrap_or_else(|e| {
            eprintln!("warning: {e}");
            yres::Refs::default()
        });
    let compiled = content::compile(&root, &meta.id, &refs)?;
    content::write_source(&root, &compiled)?;
    for w in &compiled.warnings {
        eprintln!("warning: {w}");
    }
    eprintln!(
        "==> {} block(s), {} item(s) OK{}",
        compiled.blocks,
        compiled.items,
        if compiled.warnings.is_empty() {
            String::new()
        } else {
            format!(", {} warning(s)", compiled.warnings.len())
        }
    );
    Ok(())
}

/// `yog.toml`'s `name`/`description = "@<Set>.<name>"` → the neutral text
/// (the key goes into the packaged manifest), `icon = "@<Set>.<name>"` →
/// the image source.
fn resolve_metadata(meta: &mut YogToml, refs: &yres::Refs) -> Result<(), String> {
    let at = |e: String| format!("yog.toml: {e}");
    (meta.name, meta.name_key) = yres::resolve_text(refs, &meta.name).map_err(at)?;
    (meta.description, meta.description_key) =
        yres::resolve_text(refs, &meta.description).map_err(at)?;
    if let Some(icon) = &meta.icon {
        meta.icon = Some(yres::resolve_image(refs, icon).map_err(at)?);
    }
    Ok(())
}

// ── yog run ───────────────────────────────────────────────────────────────────

/// Build, optionally export the artifact into a dev instance's mod folder,
/// then optionally launch a configured command (e.g. the instance's server
/// or client launcher) — driven by a `[run.<config_name>]` section.
fn run_config(config_name: &str, debugging_symbols: bool) -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let yog_toml_path = root.join("yog.toml");
    if !yog_toml_path.exists() {
        return Err("no yog.toml found in the current directory".into());
    }
    let meta = YogToml::read(&yog_toml_path)?;

    let cfg = meta.run_configs.iter().find(|c| c.name == config_name)
        .ok_or_else(|| {
            if meta.run_configs.is_empty() {
                format!("no [run.{config_name}] section in yog.toml, and no [run.*] sections are defined at all.\n\
                    Add one, e.g.:\n\n[run.{config_name}]\nexport_dir = \"../my-instance/yog-mods\"\ncommand    = \"java\"\nargs       = [\"-jar\", \"server.jar\", \"--nogui\"]\ncwd        = \"../my-instance\"")
            } else {
                let available: Vec<&str> = meta.run_configs.iter().map(|c| c.name.as_str()).collect();
                format!("no [run.{config_name}] section in yog.toml. Available: {}", available.join(", "))
            }
        })?
        .clone();

    let (meta, artifact) = build(debugging_symbols)?;

    if let Some(export_dir) = &cfg.export_dir {
        let dir = resolve(&root, export_dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        let dest = dir.join(format!("{}.yog", meta.id));
        std::fs::copy(&artifact, &dest)
            .map_err(|e| format!("copying to {}: {e}", dest.display()))?;
        eprintln!("==> exported {} -> {}", meta.id, dest.display());
    }

    let Some(command) = &cfg.command else {
        eprintln!("==> [run.{config_name}] has no `command` set — export-only, nothing to launch.");
        return Ok(());
    };

    eprintln!(
        "==> launching [run.{config_name}]: {command} {}",
        cfg.args.join(" ")
    );
    let mut proc = Command::new(command);
    proc.args(&cfg.args);
    if let Some(cwd) = &cfg.cwd {
        proc.current_dir(resolve(&root, cwd));
    }
    for (k, v) in &cfg.env {
        proc.env(k, v);
    }

    // `spawn` (not `status`) so the PID is available before the process
    // exits — a stable, greppable line an external debugger (`yog-debugger`,
    // Yog-IDLE) can read to know what to attach to.
    let mut child = proc
        .spawn()
        .map_err(|e| format!("failed to launch `{command}`: {e}"))?;
    eprintln!("==> launched pid {}", child.id());

    let status = child
        .wait()
        .map_err(|e| format!("failed waiting on `{command}`: {e}"))?;
    if !status.success() {
        return Err(format!("`{command}` exited with {status}"));
    }
    Ok(())
}

/// Resolve a possibly-relative path against the project root.
/// Expands a leading `~` or `~/` to the user's home directory on Unix,
/// then canonicalises the result (resolving `..` and symlinks) when the
/// path already exists.
fn resolve(root: &Path, path: &str) -> PathBuf {
    let expanded = expand_tilde(path);
    let p = PathBuf::from(&expanded);
    let joined = if p.is_absolute() { p } else { root.join(p) };
    // If the path already exists on disk, canonicalise to remove `..` etc.
    joined.canonicalize().unwrap_or(joined)
}

/// Replace a leading `~` with the value of `$HOME`.
fn expand_tilde(path: &str) -> String {
    if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return home;
        }
    }
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return home + &path[1..];
        }
    }
    path.to_string()
}

/// Generate the hidden Cargo.toml from yog.toml metadata.
fn generate_cargo_toml(meta: &YogToml) -> String {
    let authors_toml = if meta.authors.is_empty() {
        String::new()
    } else {
        let list = meta
            .authors
            .iter()
            .map(|a| format!("{a:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("authors      = [{list}]\n")
    };
    let edition = meta.edition.as_deref().unwrap_or("2021");

    // With a local yog-api, its sibling yog-* crates (yog-ui, yog-gfx,
    // yog-interop, ...) come from the same checkout too — the local API may
    // need versions of them that aren't on crates.io yet.
    let patch = if meta.yog_api_path.is_some() || std::env::var("YOG_API_PATH").is_ok() {
        let env_path = std::env::var("YOG_API_PATH").ok();
        let api_path = meta
            .yog_api_path
            .as_deref()
            .or(env_path.as_deref())
            .unwrap_or("");
        // The mod's own yog-* dependencies (yog-kit, ...) too: they're
        // built against the local yog-api.
        let extra: Vec<&str> = meta
            .dependencies
            .iter()
            .map(|(name, _)| name.as_str())
            .filter(|n| n.starts_with("yog-"))
            .collect();
        local_api_patch(Path::new(api_path), &extra)
    } else {
        String::new()
    };

    // Plain crates.io dependencies from [dependencies] — used exactly as
    // written, never guessed at.
    let mut deps_lines: Vec<String> = Vec::new();
    for (name, spec) in &meta.dependencies {
        deps_lines.push(format!("{} = {}", name, spec));
    }

    // Other Yog mods this mod depends on, from [dependencies.exports].
    // Each maps to that mod's generated exports crate — package name must
    // match `publish_exports`'s own `{sanitized_id}_exports` naming exactly
    // (underscores, not hyphens: that's the literal name published to
    // crates.io). Each dependency keeps its own name as the local alias
    // (`{name} = { package = ... }`), so mod code imports it the same way
    // as a direct dependency (`use {name}::...`) and two mod dependencies
    // never collide under one shared alias.
    for (name, spec) in &meta.mod_dependencies {
        let exports_name = format!("{}_exports", name.replace('-', "_"));
        let dep = if spec.starts_with('{') {
            let inner = spec.trim_start_matches('{').trim_end_matches('}').trim();
            if inner.is_empty() {
                format!("{name} = {{ package = \"{exports_name}\" }}")
            } else {
                format!("{name} = {{ package = \"{exports_name}\", {inner} }}")
            }
        } else {
            format!("{name} = {{ package = \"{exports_name}\", version = {spec} }}")
        };
        deps_lines.push(dep);
    }

    // With no `[dependencies.exports]` entries there's nothing to facade —
    // point straight at the mod's own `src/lib.rs`. Otherwise `[lib] path`
    // points at a generated sibling file (see `generate_lib_facade`, written
    // by `build()` next to the real `src/lib.rs` so `mod foo;` declarations
    // in the copied content still resolve) that carries the real
    // `src/lib.rs` content plus a `yog_exports` module nesting each mod
    // dependency under its own name.
    let lib_path = if meta.mod_dependencies.is_empty() {
        "../src/lib.rs"
    } else {
        "../src/lib.designer.rs"
    };

    format!(
        r#"# Generated by yog from yog.toml — do not edit.
[package]
name         = "{id}"
version      = "{version}"
edition      = "{edition}"
description  = {description:?}
{authors_line}license      = {license:?}

[lib]
name       = "{lib_name}"
crate-type = ["cdylib"]
path       = "{lib_path}"

[dependencies]
{api_dep}
rkyv = "{rkyv_ver}"
{deps}
{patch}"#,
        id = meta.id,
        rkyv_ver = workspace_rkyv_version(),
        lib_name = to_snake_name(&meta.id),
        version = meta.version,
        edition = edition,
        description = meta.description,
        authors_line = authors_toml,
        license = meta.license,
        api_dep = meta.api_dep(),
        deps = deps_lines.join("\n"),
        patch = patch,
        lib_path = lib_path,
    )
}

/// The `yog-*` crates a yog-api `Cargo.toml` depends on, in order.
fn yog_dependencies(cargo_toml: &str) -> Vec<String> {
    let mut in_deps = false;
    let mut out = Vec::new();
    for line in cargo_toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]";
            continue;
        }
        if !in_deps {
            continue;
        }
        let Some((name, _)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.starts_with("yog-") && !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// `[patch.crates-io]` pointing every yog-* dependency of the local yog-api
/// at `api_path`'s sibling crate directory (always including yog-interop,
/// whose proc-macros generated code must match).
/// `extra` adds the mod's own yog-* dependencies.
fn local_api_patch(api_path: &Path, extra: &[&str]) -> String {
    let crates_dir = api_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("crates"));
    let mut names = std::fs::read_to_string(api_path.join("Cargo.toml"))
        .map(|t| yog_dependencies(&t))
        .unwrap_or_default();
    if !names.iter().any(|n| n == "yog-interop") {
        names.push("yog-interop".into());
    }
    for name in extra {
        if !names.iter().any(|n| n == name) {
            names.push((*name).to_string());
        }
    }
    let mut out = String::from("\n[patch.crates-io]\n");
    for name in names {
        let dir = crates_dir.join(&name);
        if !dir.join("Cargo.toml").is_file() && name != "yog-interop" {
            continue;
        }
        let abs = dir.canonicalize().unwrap_or(dir);
        let _ = writeln!(out, "{name} = {{ path = {:?} }}", abs.to_string_lossy());
    }
    out
}

/// Generate the `yog_exports` facade module: the mod's real `src/lib.rs`
/// content, followed by `pub mod yog_exports { pub mod <name> { pub use
/// ::<name>::*; } ... }` — one submodule per `[dependencies.exports]`
/// entry, so mod code writes `yog_exports::yog_pipes::register_pipe(...)`
/// regardless of how many other Yog mods it depends on (each already has
/// its own unique Cargo alias, so nothing here can collide).
///
/// This inlines the source rather than `include!`ing it: rustc rejects
/// inner (`//!`) doc comments pulled in through `include!` even when the
/// `include!` call is the very first and only thing in the file (a known
/// rustc limitation — expanded tokens are never treated as "start of
/// file"), so any leading `//!` lines are dropped here instead (this is a
/// generated build artifact, not the author's real `src/lib.rs`).
///
/// The result is written as a **sibling** of the real `src/lib.rs` (see
/// `build()`), never under `.yog-build/` — `mod foo;` declarations inside
/// the copied content resolve relative to the file's own directory, so the
/// generated file must live next to `alu.rs`/`chip.rs`/etc. for those to
/// still be found; the module's own directory is the only stable answer to
/// "where do my submodules live" that survives this rewrite.
///
/// Returns `None` when there are no mod dependencies — `[lib] path` then
/// points straight at `src/lib.rs`, and no generated file exists at all.
fn generate_lib_facade(meta: &YogToml, root: &Path) -> Result<Option<String>, String> {
    if meta.mod_dependencies.is_empty() {
        return Ok(None);
    }
    let lib_rs = std::fs::read_to_string(root.join("src").join("lib.rs"))
        .map_err(|e| format!("reading src/lib.rs: {e}"))?;
    let body: String = lib_rs
        .lines()
        .skip_while(|l| l.trim_start().starts_with("//!") || l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let mut modules = String::new();
    for (name, _) in &meta.mod_dependencies {
        let ident = name.replace('-', "_");
        modules.push_str(&format!(
            "    pub mod {ident} {{ pub use ::{ident}::*; }}\n"
        ));
    }
    Ok(Some(format!(
        "// Generated by yog from yog.toml — do not edit.\n{body}\n\npub mod yog_exports {{\n{modules}}}\n"
    )))
}

/// Make the editor "just work" for a `yog.toml`-only project: rust-analyzer
/// has nothing to find at the project root (a `Cargo.toml` there is
/// forbidden — `.yog-build/Cargo.toml` is the real one), and `lib.designer.rs`
/// is easy to mistake for a second, unrelated source file. Both are fixed by
/// merging two settings into `.vscode/settings.json`:
/// - `rust-analyzer.linkedProjects` — points it at the generated Cargo.toml.
/// - `explorer.fileNesting.patterns` — `*.rs -> ${capture}.designer.rs`
///   nests any `x.designer.rs` under its `x.rs` in the file tree, the same
///   relationship VS's WinForms designer has with its code-behind file.
///
/// Merges into whatever the author already has — an existing `.vscode/
/// settings.json` (their own editor prefs) is read and only these two keys
/// are touched; a existing `linkedProjects`/`fileNesting.patterns` entry
/// list keeps every entry the author already had and just adds ours if
/// missing, rather than overwriting the whole file.
fn configure_editor(root: &Path) -> Result<(), String> {
    let vscode_dir = root.join(".vscode");
    std::fs::create_dir_all(&vscode_dir).map_err(|e| e.to_string())?;
    let settings_path = vscode_dir.join("settings.json");

    let mut settings: serde_json::Value = if settings_path.exists() {
        let text = std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| format!("parsing existing settings.json: {e}"))?
    } else {
        serde_json::json!({})
    };
    let obj = settings
        .as_object_mut()
        .ok_or("settings.json root is not a JSON object")?;

    let linked = obj
        .entry("rust-analyzer.linkedProjects")
        .or_insert_with(|| serde_json::json!([]));
    if let Some(arr) = linked.as_array_mut() {
        let wanted = serde_json::json!(".yog-build/Cargo.toml");
        if !arr.contains(&wanted) {
            arr.push(wanted);
        }
    }

    obj.entry("explorer.fileNesting.enabled")
        .or_insert(serde_json::json!(true));

    let patterns = obj
        .entry("explorer.fileNesting.patterns")
        .or_insert_with(|| serde_json::json!({}));
    if let Some(map) = patterns.as_object_mut() {
        map.entry("*.rs".to_string())
            .or_insert(serde_json::json!("${capture}.designer.rs"));
    }

    let pretty = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    write_file(&settings_path, (pretty + "\n").as_bytes())
}

/// Make sure `.gitignore` at the project root has `pattern` as its own
/// line — appends it (creating the file if missing) without touching
/// anything already there. Used to keep generated build artifacts
/// (`lib.designer.rs`) out of git even for projects that existed before
/// `yog build` started generating them.
fn ensure_gitignore_entry(root: &Path, pattern: &str) -> Result<(), String> {
    let path = root.join(".gitignore");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    if text.lines().any(|l| l.trim() == pattern) {
        return Ok(());
    }
    let mut updated = text;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(pattern);
    updated.push('\n');
    write_file(&path, updated.as_bytes())
}

// ── yog setup ────────────────────────────────────────────────────────────────

fn setup() -> Result<(), String> {
    eprintln!("==> yog setup — checking build dependencies\n");

    let rust_ok = check_rust();
    let zig_build_ok = if rust_ok { check_zigbuild() } else { false };
    let _zig_ok = check_zig();
    if rust_ok && zig_build_ok {
        check_targets();
    }

    eprintln!();
    if rust_ok && zig_build_ok {
        eprintln!("==> all good — `yog build` should produce all 5 platforms.");
    } else if rust_ok {
        eprintln!(
            "==> Rust OK but cross-compilation incomplete. Fix the above, then re-run `yog setup`."
        );
    } else {
        eprintln!("==> Install Rust first, then re-run `yog setup`.");
    }
    Ok(())
}

fn check_rust() -> bool {
    eprint!("  [?] Rust / cargo ... ");
    let ok = Command::new("cargo")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        let ver = Command::new("cargo")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();
        eprintln!("OK  ({})", ver.trim());
        true
    } else {
        eprintln!("NOT FOUND");
        eprintln!("       Install Rust via rustup:");
        eprintln!("         curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh");
        false
    }
}

fn check_zigbuild() -> bool {
    eprint!("  [?] cargo-zigbuild ... ");
    let ok = Command::new("cargo")
        .args(["zigbuild", "--help"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        eprintln!("OK");
        true
    } else {
        eprintln!("NOT FOUND");
        eprint!("       Install cargo-zigbuild? [Y/n]: ");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let input = input.trim().to_lowercase();
        if input.is_empty() || input == "y" {
            eprintln!("       Running: cargo install cargo-zigbuild");
            let status = Command::new("cargo")
                .args(["install", "cargo-zigbuild"])
                .status();
            match status {
                Ok(s) if s.success() => {
                    eprintln!("       cargo-zigbuild installed.");
                    return true;
                }
                _ => eprintln!(
                    "       Installation failed. Install manually: cargo install cargo-zigbuild"
                ),
            }
        } else {
            eprintln!("       Skipped. Cross-compilation will only work for the host platform.");
        }
        false
    }
}

fn check_zig() -> bool {
    eprint!("  [?] zig ... ");
    let ok = Command::new("zig")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        let ver = Command::new("zig")
            .arg("version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();
        eprintln!("OK  ({})", ver.trim());
        true
    } else {
        eprintln!("NOT FOUND");
        // Try to install via package manager
        eprint!("       Install zig? [Y/n]: ");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let input = input.trim().to_lowercase();
        if (input.is_empty() || input == "y") && try_install_zig() {
            return true;
        }
        eprintln!("       Download zig from: https://ziglang.org/download/");
        eprintln!("       Extract, add to PATH, then re-run `yog setup`.");
        false
    }
}

fn try_install_zig() -> bool {
    // snap
    if Command::new("snap")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        eprintln!("       Running: snap install zig --classic --beta");
        if Command::new("snap")
            .args(["install", "zig", "--classic", "--beta"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            eprintln!("       zig installed via snap.");
            return true;
        }
    }
    // brew
    if Command::new("brew")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        eprintln!("       Running: brew install zig");
        if Command::new("brew")
            .args(["install", "zig"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            eprintln!("       zig installed via brew.");
            return true;
        }
    }
    eprintln!("       Could not auto-install zig (no snap or brew found).");
    false
}

fn check_targets() {
    eprintln!("  [?] rustup cross-compile targets ...");
    let installed = installed_targets();
    let needed: Vec<&str> = TARGETS
        .iter()
        .map(|t| t.triple)
        .filter(|triple| !installed.iter().any(|s| s == triple))
        .collect();
    if needed.is_empty() {
        eprintln!("       all 5 targets installed.");
        return;
    }
    eprintln!(
        "       installing Rust {RUST_TOOLCHAIN}: {} ...",
        needed.join(", ")
    );
    let all: Vec<&Target> = TARGETS.iter().collect();
    install_targets(&all);
    let installed = installed_targets();
    let missing = needed
        .iter()
        .filter(|triple| !installed.iter().any(|s| s == *triple))
        .count();
    eprintln!("       {}", if missing == 0 { "done" } else { "FAILED" });
}

// ── yog doctor ───────────────────────────────────────────────────────────────

/// What `yog doctor` found — the same checks as `yog setup`, but read-only
/// and non-interactive, so IDEs (Yog-IDLE) can show it and install what's
/// missing themselves. `--json` is the machine-readable form; its keys are
/// a stable contract.
struct DoctorReport {
    cli_version: &'static str,
    rust_toolchain: &'static str,
    cargo: Option<String>,
    rustup: Option<String>,
    cargo_zigbuild: bool,
    zig: Option<String>,
    /// `(tag, triple, installed)` for every platform a `.yog` bundles.
    targets: Vec<(&'static str, &'static str, bool)>,
}

fn first_line_of(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

fn doctor() -> DoctorReport {
    let installed = installed_targets();
    DoctorReport {
        cli_version: env!("CARGO_PKG_VERSION"),
        rust_toolchain: RUST_TOOLCHAIN,
        cargo: first_line_of("cargo", &["--version"]),
        rustup: first_line_of("rustup", &["--version"]),
        cargo_zigbuild: Command::new("cargo")
            .args(["zigbuild", "--help"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false),
        zig: first_line_of("zig", &["version"]),
        targets: TARGETS
            .iter()
            .map(|t| (t.tag, t.triple, installed.iter().any(|s| s == t.triple)))
            .collect(),
    }
}

/// A JSON string literal (the report only holds tool version lines).
fn json_str(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn json_opt(s: &Option<String>) -> String {
    s.as_deref().map_or_else(|| "null".to_string(), json_str)
}

impl DoctorReport {
    fn to_json(&self) -> String {
        let targets: Vec<String> = self
            .targets
            .iter()
            .map(|(tag, triple, installed)| {
                format!(
                    "{{\"tag\":{},\"triple\":{},\"installed\":{installed}}}",
                    json_str(tag),
                    json_str(triple)
                )
            })
            .collect();
        format!(
            "{{\"cli_version\":{},\"rust_toolchain\":{},\"cargo\":{},\"rustup\":{},\"cargo_zigbuild\":{},\"zig\":{},\"targets\":[{}]}}",
            json_str(self.cli_version),
            json_str(self.rust_toolchain),
            json_opt(&self.cargo),
            json_opt(&self.rustup),
            self.cargo_zigbuild,
            json_opt(&self.zig),
            targets.join(",")
        )
    }

    fn print(&self) {
        let show = |name: &str, v: &Option<String>| match v {
            Some(v) => println!("  [ok] {name:<16} {v}"),
            None => println!("  [--] {name:<16} not found"),
        };
        println!(
            "yog {} (mods build with Rust {})",
            self.cli_version, self.rust_toolchain
        );
        show("cargo", &self.cargo);
        show("rustup", &self.rustup);
        println!(
            "  [{}] {:<16} {}",
            if self.cargo_zigbuild { "ok" } else { "--" },
            "cargo-zigbuild",
            if self.cargo_zigbuild {
                "installed"
            } else {
                "not found"
            }
        );
        show("zig", &self.zig);
        for (tag, triple, installed) in &self.targets {
            println!(
                "  [{}] {tag:<16} {triple}",
                if *installed { "ok" } else { "--" }
            );
        }
        println!("Run `yog setup` to install what's missing.");
    }
}

// ── Build internals ───────────────────────────────────────────────────────────

enum Builder {
    Zig,
    Cargo,
}

impl Builder {
    fn detect() -> Self {
        let ok = Command::new("cargo")
            .args(["zigbuild", "--help"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            Builder::Zig
        } else {
            Builder::Cargo
        }
    }
    fn subcmd(&self) -> &'static str {
        match self {
            Builder::Zig => "zigbuild",
            Builder::Cargo => "build",
        }
    }

    fn build(
        &self,
        build_dir: &Path,
        triple: &str,
        root: &Path,
        debugging_symbols: bool,
    ) -> Result<(), ()> {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(build_dir)
            .env("CARGO_TARGET_DIR", root.join("target"))
            .args([self.subcmd(), "--release", "--target", triple]);
        if debugging_symbols {
            // Still `--release` (the output layout `package()` expects), but
            // a debug build in all but name: debug info kept, and nothing
            // optimized — inlining and instruction scheduling make a
            // debugger's line steps hop between files and back.
            cmd.env("CARGO_PROFILE_RELEASE_DEBUG", "true");
            cmd.env("CARGO_PROFILE_RELEASE_STRIP", "none");
            cmd.env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "0");
            cmd.env("CARGO_PROFILE_RELEASE_LTO", "false");
            cmd.env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "256");
            cmd.env("CARGO_PROFILE_RELEASE_INCREMENTAL", "true");
        }
        let output = cmd.output();
        match output {
            Ok(o) => {
                eprint!(
                    "{}",
                    filter_benign_warnings(&String::from_utf8_lossy(&o.stderr))
                );
                if o.status.success() {
                    Ok(())
                } else {
                    Err(())
                }
            }
            _ => Err(()),
        }
    }
}

/// Warnings that come from the zig cross-linking toolchain itself, not from
/// the mod's code, and that the mod author can't act on:
/// - "xcrun … MacOSX.sdk failed" — rustc probing for a macOS SDK when
///   cross-linking Apple targets (zig handles the actual linking).
/// - "linker stderr: ignoring deprecated linker optimization setting" —
///   rustc always passes `-Wl,-O1` to gnu-flavored linkers; zig's ld (0.16+)
///   ignores it and says so on every link.
fn is_benign_warning(t: &str) -> bool {
    t.starts_with("warning:")
        && ((t.contains("xcrun") && t.contains("failed"))
            || (t.contains("linker stderr")
                && t.contains("deprecated linker optimization setting")))
}

/// Drop each [`is_benign_warning`] block, along with the matching cargo
/// "generated 1 warning" line.
fn filter_benign_warnings(stderr: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    let mut suppressed = 0usize;
    for line in stderr.lines() {
        let t = line.trim_start();
        if is_benign_warning(t) {
            in_block = true;
            suppressed += 1;
            continue;
        }
        if in_block {
            if t == "|" || t.starts_with("= note:") || t.starts_with("= help:") || t.is_empty() {
                continue;
            }
            in_block = false;
        }
        // Cargo's per-crate summary counting only the suppressed warning.
        if suppressed > 0 && t.starts_with("warning:") && t.ends_with("generated 1 warning") {
            suppressed -= 1;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `YOG_TARGETS` — comma-separated platform tags or triples
/// (e.g. `linux-x86_64`) to restrict `yog build` to. Unset or empty means
/// every installed target. A dev loop only needs the host platform; release
/// builds should leave it unset so the `.yog` stays multi-platform.
fn selected_targets() -> Option<Vec<String>> {
    let raw = std::env::var("YOG_TARGETS").ok()?;
    let list: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    (!list.is_empty()).then_some(list)
}

/// The Rust toolchain mods are built with: the `rust-version` the yog-*
/// crates themselves declare, so a mod always compiles with the same
/// compiler as the API it links against.
const RUST_TOOLCHAIN: &str = env!("CARGO_PKG_RUST_VERSION");

/// `.yog-build/rust-toolchain.toml` — pins the toolchain and lists every
/// target so rustup fetches them on its own. Generated, never hand-written.
fn generate_rust_toolchain() -> String {
    let targets: Vec<String> = TARGETS
        .iter()
        .map(|t| format!("    \"{}\",\n", t.triple))
        .collect();
    format!(
        "# Generated by `yog build` — do not edit.\n\
         [toolchain]\n\
         channel = \"{RUST_TOOLCHAIN}\"\n\
         profile = \"minimal\"\n\
         targets = [\n{}]\n",
        targets.concat()
    )
}

/// Installs the mod toolchain plus `targets` (a no-op when they're already
/// there), so a build never silently drops a platform for lack of setup.
fn install_targets(targets: &[&Target]) {
    if targets.is_empty() {
        return;
    }
    let list: Vec<&str> = targets.iter().map(|t| t.triple).collect();
    let ok = Command::new("rustup")
        .args([
            "toolchain",
            "install",
            RUST_TOOLCHAIN,
            "--profile",
            "minimal",
        ])
        .args(["--target", &list.join(",")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("    warning: could not install Rust {RUST_TOOLCHAIN} targets via rustup");
    }
}

fn installed_targets() -> Vec<String> {
    Command::new("rustup")
        .args([
            "target",
            "list",
            "--installed",
            "--toolchain",
            RUST_TOOLCHAIN,
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn lib_filename(name: &str, os: &str) -> String {
    // Must match the `[lib] name` pinned in the generated wrapper Cargo.toml
    // (see to_snake_name) — that's what cargo actually names the artifact.
    let stem = to_snake_name(name);
    match os {
        "windows" => format!("{stem}.dll"),
        "macos" => format!("lib{stem}.dylib"),
        _ => format!("lib{stem}.so"),
    }
}

// ── Assets ───────────────────────────────────────────────────────────────────

/// The mod's `assets/` and `data/` files plus `compiled` ones (`.yres`
/// resources, `content/` blocks and items — replacing a file at the same
/// path).
fn gather_assets(root: &Path, compiled: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
    let mut present: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();

    let mut stack: Vec<PathBuf> = ["assets", "data"]
        .iter()
        .map(|d| root.join(d))
        .filter(|p| p.is_dir())
        .collect();

    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(root) {
                let key = rel.to_string_lossy().replace('\\', "/");
                if let Ok(bytes) = std::fs::read(&path) {
                    present.insert(key.clone());
                    out.push((key, bytes));
                }
            }
        }
    }

    for (key, bytes) in compiled {
        if !present.insert(key.clone()) {
            out.retain(|(k, _)| *k != key);
        }
        out.push((key, bytes));
    }

    out
}

// ── Packaging ─────────────────────────────────────────────────────────────────

fn package(
    out: &Path,
    meta: &YogToml,
    bundled: &[(String, PathBuf)],
    assets: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (tag, native) in bundled {
        let lib_name = native
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("bad native filename")?;
        let entry = format!("natives/{tag}/{lib_name}");
        let bytes = std::fs::read(native).map_err(|e| e.to_string())?;
        zip.start_file(&entry, opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    for (entry, bytes) in assets {
        zip.start_file(entry, opts).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }

    let platforms = bundled
        .iter()
        .map(|(t, _)| format!("{t:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let authors = meta
        .authors
        .iter()
        .map(|a| format!("{a:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    // The full metadata travels inside the archive so the loader can show it
    // (mod list UI etc.) without needing the source project.
    let mut manifest = format!(
        "id = {:?}\nname = {:?}\nversion = {:?}\ndescription = {:?}\nauthors = [{}]\nlicense = {:?}\nabi = 2\nplatforms = [{platforms}]\n",
        meta.id, meta.name, meta.version, meta.description, authors, meta.license,
    );
    // Localized metadata: the loader shows the translation when it has one.
    for (field, value) in [
        ("name_key", &meta.name_key),
        ("description_key", &meta.description_key),
        ("icon", &meta.icon),
    ] {
        if let Some(v) = value {
            let _ = writeln!(manifest, "{field} = {v:?}");
        }
    }
    zip.start_file("yog.toml", opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(manifest.as_bytes())
        .map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

// ── yog add / remove ──────────────────────────────────────────────────────────

fn add_dep(crate_name: Option<&str>, is_mod: bool) -> Result<(), String> {
    let usage = if is_mod {
        "usage: yog add --mod <yog-mod-id>"
    } else {
        "usage: yog add <crate>"
    };
    let name = crate_name.ok_or(usage)?;
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let yog_toml_path = root.join("yog.toml");
    if !yog_toml_path.exists() {
        return Err("no yog.toml found in the current directory".into());
    }

    // `[dependencies]` (plain crates.io crates) and `[dependencies.exports]`
    // (other Yog mods, mapped to their generated exports crate) are
    // distinct sections — match the exact header, not a substring, so
    // "dependencies" never also matches "dependencies.exports" or vice versa.
    let target_section = if is_mod {
        "[dependencies.exports]"
    } else {
        "[dependencies]"
    };

    let text = std::fs::read_to_string(&yog_toml_path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

    let has_section = lines.iter().any(|l| l.trim() == target_section);

    if !has_section {
        // Add the section before any [dev] section or at the end
        let insert_idx = lines
            .iter()
            .position(|l| {
                l.trim().starts_with('[') && l.trim() != "[mod]"
            })
            .unwrap_or(lines.len());
        lines.insert(insert_idx, target_section.to_string());
        lines.insert(insert_idx + 1, format!("{} = \"*\"", name));
    } else {
        // Find the section and add the crate at its end
        let mut in_section = false;
        let mut inserted = false;
        for i in 0..lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with('[') {
                if in_section && trimmed != target_section {
                    lines.insert(i, format!("{} = \"*\"", name));
                    inserted = true;
                    break;
                }
                in_section = trimmed == target_section;
            }
        }
        if !inserted {
            // The section was at the end of the file
            lines.push(format!("{} = \"*\"", name));
        }
    }

    let new_text = lines.join("\n") + "\n";
    std::fs::write(&yog_toml_path, new_text).map_err(|e| e.to_string())?;
    eprintln!(
        "==> added {} to yog.toml [{}]",
        name,
        &target_section[1..target_section.len() - 1]
    );
    Ok(())
}

fn remove_dep(crate_name: Option<&str>, is_mod: bool) -> Result<(), String> {
    let usage = if is_mod {
        "usage: yog remove --mod <yog-mod-id>"
    } else {
        "usage: yog remove <crate>"
    };
    let name = crate_name.ok_or(usage)?;
    let target_section = if is_mod {
        "[dependencies.exports]"
    } else {
        "[dependencies]"
    };
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let yog_toml_path = root.join("yog.toml");
    if !yog_toml_path.exists() {
        return Err("no yog.toml found in the current directory".into());
    }

    let text = std::fs::read_to_string(&yog_toml_path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = Vec::new();
    let mut removed = false;
    let mut in_section = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == target_section;
        }
        // Remove lines like: crate-name = "..." or crate-name = { ... },
        // only within the targeted section.
        if in_section
            && trimmed.starts_with(name)
            && trimmed.contains('=')
            && !trimmed.starts_with('[')
        {
            removed = true;
            continue;
        }
        lines.push(line.to_string());
    }

    if !removed {
        return Err(format!(
            "dependency {} not found in yog.toml {}",
            name, target_section
        ));
    }

    let new_text = lines.join("\n") + "\n";
    std::fs::write(&yog_toml_path, new_text).map_err(|e| e.to_string())?;
    eprintln!("==> removed {} from yog.toml {}", name, target_section);
    Ok(())
}

// ── yog publish exports ────────────────────────────────────────────────────────

/// Item found by the export scanner.
#[derive(Debug)]
struct ExportItem {
    kind: String, // "struct", "enum", "fn"
    name: String,
    source: String, // full Rust source of the item (without #[yog_export])
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
fn collect_uses(exports: &[ExportItem], src_dir: &Path) -> (Vec<String>, Vec<String>) {
    use std::collections::HashSet;

    // 1 + 2: names that never need an import.
    // Primitives and std/core/alloc items are always in scope.
    let mut primitives: HashSet<&str> = HashSet::new();
    for p in [
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
    ] {
        primitives.insert(p);
    }
    // std/core/alloc prelude modules — anything starting with these is in scope.
    let prelude_prefixes: &[&str] = &["std::", "core::", "alloc::"];

    // Names defined locally in the mod (exported items) — no import required.
    let local_names: HashSet<String> = exports.iter().map(|e| e.name.clone()).collect();

    // 3: gather every `use` statement from the mod's source so we can map a
    //    used type back to its import path.
    let mut import_paths: Vec<String> = Vec::new(); // e.g. "yog_api::registry::Mod"
    let mut system_imports: HashSet<String> = HashSet::new();
    let mut type_imports: HashSet<String> = HashSet::new();
    if let Ok(files) = collect_rs_files(src_dir) {
        for path in files {
            let Ok(src) = std::fs::read_to_string(&path) else {
                continue;
            };
            for line in src.lines() {
                let t = line.trim();
                if !t.starts_with("use ") {
                    continue;
                }
                let path_part = t[4..].trim_end_matches(';').trim();
                if path_part.is_empty() {
                    continue;
                }
                // Skip malformed imports like `yog_api::{` without closing brace.
                if path_part.contains('{') && !path_part.contains('}') {
                    continue;
                }
                // `use foo::{a, b}` / `use foo::*` — keep the prefix as a candidate.
                if path_part.starts_with("std::")
                    || path_part.starts_with("core::")
                    || path_part.starts_with("alloc::")
                {
                    system_imports.insert(path_part.to_string());
                } else {
                    type_imports.insert(path_part.to_string());
                }
                import_paths.push(path_part.to_string());
            }
        }
    }

    /// Resolve a bare type identifier to the most specific `use` path available.
    fn resolve_name(
        name: &str,
        import_paths: &[String],
        local_names: &HashSet<String>,
        primitives: &HashSet<&str>,
        prelude_prefixes: &[&str],
    ) -> Option<String> {
        // 1. primitive / prelude
        if primitives.contains(name) {
            return None;
        }
        if prelude_prefixes.iter().any(|p| name.starts_with(p)) {
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
            if prelude_prefixes.iter().any(|p| path.starts_with(p)) {
                system_imports.insert(path.to_string());
            } else if first == "yog_api" || first == "crate" || first == "super" || first == "self"
            {
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
            {
                if let Some(p) = resolve_name(
                    tok,
                    &import_paths,
                    &local_names,
                    &primitives,
                    prelude_prefixes,
                ) {
                    if p.starts_with("std::") || p.starts_with("core::") || p.starts_with("alloc::")
                    {
                        system_imports.insert(p);
                    } else {
                        type_imports.insert(p);
                    }
                }
            }
        }
    }

    // Normalize: drop a simple import if a brace form already covers it.
    fn normalize(vec: &mut Vec<String>) {
        vec.sort();
        let mut braces: Vec<String> = Vec::new();
        let mut simples: Vec<String> = Vec::new();
        for p in vec.drain(..) {
            if p.contains('{') {
                braces.push(p);
            } else {
                simples.push(p);
            }
        }
        let mut keep: Vec<String> = Vec::new();
        for brace in braces {
            let prefix = brace.split('{').next().unwrap_or("");
            let inside = brace
                .split('{')
                .nth(1)
                .and_then(|s| s.split('}').next())
                .unwrap_or("");
            let items: HashSet<&str> = inside.split(',').map(|s| s.trim()).collect();
            simples.retain(|s| {
                if s.starts_with(prefix) {
                    !items.contains(&s.strip_prefix(prefix).unwrap_or(s).trim())
                } else {
                    true
                }
            });
            keep.push(brace);
        }
        keep.extend(simples);
        keep.sort();
        *vec = keep;
    }

    let mut system_vec: Vec<String> = system_imports.into_iter().collect();
    let mut type_vec: Vec<String> = type_imports.into_iter().collect();
    normalize(&mut system_vec);
    normalize(&mut type_vec);

    (system_vec, type_vec)
}

/// Scans the mod for `#[yog_export]` items, generates an `-exports` crate,
/// and publishes it to crates.io.
fn publish_exports(dry_run: bool) -> Result<(), String> {
    let proj = std::env::current_dir().map_err(|e| e.to_string())?;
    let manifest = YogToml::read(&proj.join("yog.toml"))?;

    let mod_id = manifest.id;
    let version = manifest.version;
    let edition = manifest.edition.as_deref().unwrap_or("2021");
    // Exports crates default to MIT OR Apache-2.0 regardless of the mod's own
    // license — an AGPL mod (like this CLI's own Yog-Pipes) would otherwise
    // force every consumer's dependency tree under AGPL too, which kills
    // adoption. `[mod] exports_license = "..."` opts into a different license.
    let license = manifest
        .exports_license
        .clone()
        .unwrap_or_else(|| "MIT OR Apache-2.0".to_string());
    let authors = manifest.authors.join(", ");
    let exports_crate_name = format!("{}_exports", mod_id.replace('-', "_"));

    let src_dir = proj.join("src");
    if !src_dir.exists() {
        return Err("no src/ directory — nothing to export".into());
    }

    let mut exports: Vec<ExportItem> = Vec::new();

    for path in collect_rs_files(&src_dir)? {
        let src = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let lines: Vec<&str> = src.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed == "#[yog_export]" || trimmed.starts_with("#[yog_export(") {
                i += 1;
                // Skip outer attributes that may appear between #[yog_export] and the item
                while i < lines.len() && lines[i].trim() == "#[yog_export]" {
                    i += 1;
                }
                // Collect the item source: pub struct / pub enum / pub fn
                if i >= lines.len() {
                    break;
                }
                let first = lines[i].trim();
                let kind = if first.starts_with("pub struct ") {
                    "struct"
                } else if first.starts_with("pub enum ") {
                    "enum"
                } else if first.starts_with("pub fn ") {
                    "fn"
                } else {
                    i += 1;
                    continue;
                };

                // Extract item name from the first line
                let name = if kind == "fn" {
                    first
                        .trim_start_matches("pub fn ")
                        .split('(')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string()
                } else if kind == "struct" {
                    first
                        .trim_start_matches("pub struct ")
                        .split(&['{', '(', ' ', '<', ';', '\t'][..])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string()
                } else {
                    // enum
                    first
                        .trim_start_matches("pub enum ")
                        .split(&['{', ' ', '<', ';', '\t'][..])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string()
                };

                if name.is_empty() {
                    i += 1;
                    continue;
                }

                // Collect lines until the top-level brace block closes
                let mut depth: i32 = 0;
                let mut found_open = false;
                let mut item_lines: Vec<String> = Vec::new();
                // Start from the first line of the item (current i)
                let _start_i = i;
                while i < lines.len() {
                    let line = lines[i];
                    item_lines.push(line.to_string());
                    for ch in line.chars() {
                        if ch == '{' {
                            depth += 1;
                            found_open = true;
                        } else if ch == '}' {
                            depth -= 1;
                        }
                    }
                    if found_open && depth <= 0 {
                        item_lines.push(String::new());
                        i += 1;
                        break;
                    }
                    // Also handle items with no body (empty struct/enum with semicolon)
                    if !found_open && line.trim().ends_with(';') {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let source = item_lines.join("\n");
                exports.push(ExportItem {
                    kind: kind.to_string(),
                    name,
                    source,
                });
            }
            i += 1;
        }
    }

    if exports.is_empty() {
        eprintln!("==> no #[yog_export] items found — nothing to publish");
        return Ok(());
    }

    // Collect used namespaces from exported items
    let (system_uses, type_uses) = collect_uses(&exports, &src_dir);

    eprintln!(
        "==> generating {exports_crate_name} ({items} export(s))",
        items = exports.len()
    );
    eprintln!("    system_uses: {:?}", system_uses);
    eprintln!("    type_uses: {:?}", type_uses);

    // Generate the exports crate in .yog-build/exports/
    let build_dir = proj
        .join(".yog-build")
        .join("exports")
        .join(&exports_crate_name);
    let _ = std::fs::remove_dir_all(&build_dir);
    std::fs::create_dir_all(build_dir.join("src")).map_err(|e| e.to_string())?;

    // Resolve versions dynamically (same logic as `yog build`)
    let yog_api_ver = latest_yog_api_version();
    let rkyv_ver = workspace_rkyv_version();
    let yog_api_override = manifest.yog_api_version.as_deref().unwrap_or(&yog_api_ver);

    let maybe_authors = if authors.is_empty() {
        String::new()
    } else {
        format!("authors = [\"{}\"]\n", authors.replace(", ", "\", \""))
    };

    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "{version}"
edition = "{edition}"
{maybe_authors}license = "{license}"
description = "Exports from the {mod_id} Yog mod — generated by `yog publish exports`."

[dependencies]
yog-api = "{yog_api_override}"
rkyv = "{rkyv_ver}"

[lib]
crate-type = ["cdylib", "lib"]
"#,
        name = exports_crate_name,
        version = version,
        edition = edition,
        maybe_authors = maybe_authors,
        license = license,
        mod_id = mod_id,
        yog_api_override = yog_api_override,
        rkyv_ver = rkyv_ver,
    );
    write_file(&build_dir.join("Cargo.toml"), cargo_toml.as_bytes())?;

    // lib.rs — generate wrapper module.
    // For struct/enum: emit with rkyv derives so consumers get the serialisable types.
    // For fn: emit a full interop wrapper using the same pattern as `import!`.
    let mut lib_rs = format!(
        "//! Auto-generated exports from `{mod_id}` v{version}.\n\
         //! DO NOT EDIT — generated by `yog publish exports`.\n\n\
         #![allow(unused_imports)]\n\n"
    );

    // Add system uses (std, core, alloc)
    if !system_uses.is_empty() {
        lib_rs.push_str("// System dependencies\n");
        for u in &system_uses {
            lib_rs.push_str(&format!("use {};\n", u));
        }
        lib_rs.push('\n');
    }

    // Add type dependencies (yog_api, rkyv, etc.)
    // Filter out crate-internal imports and duplicates of exported items
    let exported_names: HashSet<String> = exports.iter().map(|e| e.name.clone()).collect();
    if !type_uses.is_empty() {
        lib_rs.push_str("// Type dependencies\n");
        for u in &type_uses {
            // Skip imports that:
            // 1. Reference crate:: (internal modules not available in exports crate)
            // 2. Are the same as an exported item (would cause duplicate definitions)
            let first_part = u.split("::").next().unwrap_or("");
            if first_part == "crate" || first_part == "super" || first_part == "self" {
                continue;
            }
            let last_part = u.rsplit("::").next().unwrap_or(u);
            if exported_names.contains(last_part) {
                continue;
            }
            lib_rs.push_str(&format!("use {};\n", u));
        }
        lib_rs.push('\n');
    }

    for item in &exports {
        match item.kind.as_str() {
            "struct" | "enum" => {
                // Reproduce the item with #[derive] for rkyv + serde.
                // Strip existing #[derive(...)] to avoid duplicates.
                let cleaned = strip_derive_attrs(&item.source);
                lib_rs.push_str(&format!(
                     "#[derive(::yog_api::rkyv::Archive, ::yog_api::rkyv::Serialize, ::yog_api::rkyv::Deserialize, Debug, Clone, PartialEq)]\n\
                      {}\n",
                     cleaned,
                 ));
            }
            "fn" => {
                // Parse the function signature to build a proper interop wrapper.
                // We have the full source of the fn; extract sig and produce:
                //   pub fn {name}(input_type) -> output_type { rkyv call }
                //   static __YOG_SLOT_{NAME}: OnceLock<...>
                //   #[no_mangle] pub unsafe extern "C" fn __yog_bind_{name}(ptr)
                let wrapper = generate_fn_wrapper(&item.name, &item.source);
                lib_rs.push_str(&wrapper);
            }
            _ => {}
        }
    }
    // Types are already in crate root, no extra re-export needed.
    write_file(&build_dir.join("src").join("lib.rs"), lib_rs.as_bytes())?;

    if dry_run {
        eprintln!("==> dry-run: skipping cargo publish");
        eprintln!("    generated crate is at: {}", build_dir.display());
        eprintln!("    Run `cargo publish` manually from that directory, or");
        eprintln!("    re-run without --dry-run to publish automatically.");
        return Ok(());
    }

    // Publish
    eprintln!("==> cargo publish (--allow-dirty)");
    let status = std::process::Command::new("cargo")
        .args(["publish", "--allow-dirty"])
        .current_dir(&build_dir)
        .status()
        .map_err(|e| format!("cargo publish failed: {e}"))?;

    if !status.success() {
        return Err(format!("cargo publish exited with {}", status));
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&build_dir);
    eprintln!("==> published {exports_crate_name} v{version}");
    Ok(())
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/// Strip `#[derive(...)]` lines from a Rust source item to avoid duplicates
/// when the generator adds its own derives.
fn strip_derive_attrs(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim().starts_with("#[derive("))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Generate a full interop wrapper for a `#[yog_export]` function.
///
/// Given the source of a function like:
/// ```rust
/// pub fn register_pipe(registry: &mut yog_api::Registry, def: PipeDef) -> Result<(), String> { ... }
/// ```
///
/// Produces (a leading `&mut Registry`-shaped parameter is dropped — the
/// exporting mod reconstructs its own `Registry` from the API pointer it
/// captured at load time, so callers pass only the remaining, natural
/// arguments; multiple remaining arguments are tupled for serialization):
/// ```rust
/// #![allow(non_snake_case)]
/// pub fn register_pipe(__yog_arg0: PipeDef) -> Result<(), String> {
///     // rkyv-serialize __yog_arg0, call via C-ABI slot, deserialize result
/// }
/// static __YOG_SLOT_REGISTER_PIPE: OnceLock<...>;
/// #[no_mangle] pub unsafe extern "C" fn __yog_bind_register_pipe(ptr) { ... }
/// ```
fn generate_fn_wrapper(name: &str, source: &str) -> String {
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

    let arg_names: Vec<String> = (0..typed_params.len())
        .map(|i| format!("__yog_arg{i}"))
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
    let wrapper_ty = "unsafe extern \"C\" fn(input_ptr: *const u8, input_len: u32, \
         out_data: *mut *mut u8, out_len: *mut u32, out_cap: *mut u32)"
        .to_string();

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
    let mut out_data: *mut u8 = std::ptr::null_mut();
    let mut out_len: u32 = 0;
    let mut out_cap: u32 = 0;
    unsafe {{
        f(input_bytes.as_ptr(), input_bytes.len() as u32,
          &mut out_data, &mut out_len, &mut out_cap);
        let output_slice = std::slice::from_raw_parts(out_data, out_len as usize);
        let result: {output_type_fmt} = ::yog_api::rkyv::from_bytes::<_, ::yog_api::rkyv::rancor::Error>(output_slice)
            .expect(concat!("yog: deser failed in export '", "{name}", "'"));
        let _ = Vec::from_raw_parts(out_data, out_len as usize, out_cap as usize);
        result
    }}
}}

static {slot_name_fmt}: ::std::sync::OnceLock<{wrapper_ty}> = ::std::sync::OnceLock::new();

#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn {bind_name_fmt}(ptr: *const ::std::os::raw::c_void) {{
    let f: {wrapper_ty} = ::std::mem::transmute(ptr);
    {slot_name_fmt}.set(f).ok();
}}
"#,
        name = name,
        params_sig_fmt = params_sig_fmt,
        output_type_fmt = output_type_fmt,
        slot_name_fmt = slot_name_fmt,
        wrapper_ty = wrapper_ty,
        bind_name_fmt = bind_name_fmt,
        serialize_block = serialize_block,
    )
}

fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// Recursively collect every `.rs` file under `dir`, so `#[yog_export]` items
/// can live in any module/subfolder — not just flat files directly in `src/`.
fn collect_rs_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_rs_files(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            // Skip the `yog_exports` facade `build()` generates next to
            // `lib.rs` — it's a verbatim copy of the real source, and
            // scanning it too would double every `#[yog_export]` item.
            && path.file_name().and_then(|n| n.to_str()) != Some("lib.designer.rs")
        {
            out.push(path);
        }
    }
    Ok(out)
}

/// rkyv version embedded at compile time via `build.rs`.
fn workspace_rkyv_version() -> String {
    env!("RKYV_VERSION").to_string()
}

/// Fetch the latest non-yanked version of `yog-api` from crates.io.
fn latest_yog_api_version() -> String {
    match std::process::Command::new("cargo")
        .args(["search", "yog-api", "--limit", "1"])
        .output()
    {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .and_then(|l| {
                    let mut parts = l.split_whitespace();
                    parts.next(); // skip crate name (e.g. "yog-api")
                    if parts.next() == Some("=") {
                        // skip the "=" separator
                        parts.next().map(|v| v.trim_matches('"').to_owned())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "0.5".into())
        }
        _ => "0.5".into(), // fallback when cargo search fails (offline, etc.)
    }
}

#[cfg(test)]
mod tests {
    use super::{filter_benign_warnings, DoctorReport};

    #[test]
    fn yog_dependencies_reads_the_dependencies_table() {
        let toml = "[package]\nname = \"yog-api\"\n\n[dependencies]\nyog-abi  = \"0.25\"\nyog-ui = { version = \"0.35\" }\nserde = \"1\"\n\n[dev-dependencies]\nyog-test = \"1\"\n";
        assert_eq!(super::yog_dependencies(toml), ["yog-abi", "yog-ui"]);
    }

    #[test]
    fn doctor_json_is_valid() {
        let report = DoctorReport {
            cli_version: "1.2.3",
            rust_toolchain: "1.99",
            cargo: Some("cargo 1.99.0 (abc \"q\")".into()),
            rustup: None,
            cargo_zigbuild: true,
            zig: None,
            targets: vec![("linux-x86_64", "x86_64-unknown-linux-gnu", true)],
        };
        let v: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(v["cli_version"], "1.2.3");
        assert_eq!(v["cargo"], "cargo 1.99.0 (abc \"q\")");
        assert!(v["rustup"].is_null());
        assert_eq!(v["targets"][0]["installed"], true);
    }

    #[test]
    fn drops_zig_linker_optimization_warning() {
        let stderr = "\
warning: linker stderr: ignoring deprecated linker optimization setting '1'
  |
  = note: `#[warn(linker_messages)]` on by default

warning: `yog-pipes` (lib) generated 1 warning
    Finished `release` profile [optimized] target(s) in 0.03s
";
        assert_eq!(
            filter_benign_warnings(stderr),
            "    Finished `release` profile [optimized] target(s) in 0.03s\n"
        );
    }

    #[test]
    fn keeps_real_warnings() {
        let stderr = "\
warning: unused variable: `x`
  --> src/lib.rs:1:5
warning: `my-mod` (lib) generated 1 warning
";
        assert_eq!(filter_benign_warnings(stderr), stderr);
    }
}
