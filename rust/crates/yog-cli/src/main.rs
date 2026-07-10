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
//!   yog help         — show usage

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("build")          => build().map(|_| ()),
        Some("new")            => new_mod(args.get(2).map(String::as_str)),
        Some("setup")          => setup(),
        Some("add")            => add_dep(args.get(2).map(String::as_str)),
        Some("remove")         => remove_dep(args.get(2).map(String::as_str)),
        Some("run")            => match args.get(2) {
            Some(name) => run_config(name),
            None => Err("usage: yog run <config_name>  (see [run.<config_name>] in yog.toml)".into()),
        },
        Some("publish")         => match args.get(2).map(String::as_str) {
            Some("exports") => publish_exports(),
            Some(other) => Err(format!("unknown publish subcommand: {other}")),
            None => Err("usage: yog publish exports".into()),
        },
        Some("-h") | Some("--help") | Some("help") | None => { print_usage(); return; }
        Some(other)            => Err(format!("unknown command: {other}")),
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
         \x20 new <name>        Create a new mod project in ./<name>/\n\
         \x20 build             Compile the current mod and package it as artifacts/<id>.yog\n\
         \x20 setup             Check and install build dependencies (Rust, Zig, targets)\n\
         \x20 add <crate>       Add a Rust dependency to yog.toml\n\
         \x20 remove <crate>    Remove a dependency from yog.toml\n\
         \x20 run <config>      Build, export, and launch a [run.<config>] dev instance\n\
         \x20 help              Show this message\n\n\
         Mod projects use yog.toml instead of Cargo.toml.\n\
         Cross-compilation requires cargo-zigbuild + zig (yog setup installs them)."
    );
}

// ── yog.toml ─────────────────────────────────────────────────────────────────

/// Parsed content of a `yog.toml` project file.
#[derive(Debug)]
struct YogToml {
    id:          String,
    name:        String,
    version:     String,
    description: String,
    authors:     Vec<String>,
    license:     String,
    edition:     Option<String>,
    /// Optional: path to yog-api for local/monorepo development.
    /// Set via [dev] yog_api_path = "..."  or YOG_API_PATH env var.
    yog_api_path: Option<String>,
    /// Optional: pin yog-api version as "yog_api = \"X.Y\"" in [dependencies].
    yog_api_version: Option<String>,
    /// User-declared dependencies from [dependencies] section.
    dependencies: Vec<(String, String)>,
    /// Named dev-instance launch configs from `[run.<name>]` sections.
    run_configs: Vec<RunConfig>,
}

/// A `[run.<name>]` section: where to drop the built artifact and what to
/// launch afterwards, for a full `yog run <name>` dev-instance workflow.
#[derive(Debug, Default, Clone)]
struct RunConfig {
    name: String,
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
            let abs = PathBuf::from(&p).canonicalize().unwrap_or_else(|_| PathBuf::from(&p));
            return format!("yog-api = {{ path = {:?} }}", abs.to_string_lossy());
        }
        if let Some(p) = &self.yog_api_path {
            return format!("yog-api = {{ path = {p:?} }}");
        }
        let version = {
            let env_version = std::env::var("YOG_API_VERSION").ok();
            self.yog_api_version.as_deref()
                .or(env_version.as_deref())
                .map(|v| v.to_owned())
                .unwrap_or_else(|| latest_yog_api_version())
        };
        format!("yog-api = {:?}", version)
    }
}

fn parse_yog_toml(text: &str) -> Result<YogToml, String> {
    let mut section       = "";
    let mut id            = None::<String>;
    let mut name          = None::<String>;
    let mut version       = None::<String>;
    let mut description   = None::<String>;
    let mut authors: Vec<String> = Vec::new();
    let mut license       = None::<String>;
    let mut edition       = None::<String>;
    let mut yog_api_path  = None::<String>;
    let mut dependencies: Vec<(String, String)> = Vec::new();
    let mut run_configs: HashMap<String, RunConfig> = HashMap::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']');
            continue;
        }
        match section {
            "mod" | "package" => {
                if let Some(v) = field(line, "id")          { id          = Some(v); }
                if let Some(v) = field(line, "name")        { name        = Some(v); }
                if let Some(v) = field(line, "version")     { version     = Some(v); }
                if let Some(v) = field(line, "description") { description = Some(v); }
                if let Some(v) = field(line, "license")     { license     = Some(v); }
                if let Some(v) = field(line, "edition")     { edition     = Some(v); }
                if line.trim_start().starts_with("authors") {
                    authors = parse_string_array(line);
                }
            }
            "dev" => {
                if let Some(v) = field(line, "yog_api_path") { yog_api_path = Some(v); }
            }
            "dependencies" => {
                if let Some((name, spec)) = parse_dep_line(line) {
                    dependencies.push((name, spec));
                }
            }
            _ if section.starts_with("run.") => {
                let run_name = &section["run.".len()..];
                let cfg = run_configs.entry(run_name.to_string())
                    .or_insert_with(|| RunConfig { name: run_name.to_string(), ..Default::default() });
                if let Some(v) = field(line, "export_dir") { cfg.export_dir = Some(v); }
                if let Some(v) = field(line, "command")    { cfg.command    = Some(v); }
                if let Some(v) = field(line, "cwd")        { cfg.cwd        = Some(v); }
                if line.trim_start().starts_with("args") {
                    cfg.args = parse_string_array(line);
                }
                if line.trim_start().starts_with("env") {
                    cfg.env = parse_string_array(line).into_iter()
                        .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
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
        if name == "yog_api_version" {
            yog_api_version = Some(spec.trim_matches('"').to_string());
        } else if name == "yog-api" {
            yog_api_version = Some(spec.trim_matches('"').to_string());
        } else if name == "yog_api" {
            yog_api_version = Some(spec.trim_matches('"').to_string());
        } else {
            filtered_deps.push((name, spec));
        }
    }
    Ok(YogToml {
        name:         name.unwrap_or_else(|| id.clone()),
        version:      version.unwrap_or_else(|| "0.1.0".into()),
        description:  description.unwrap_or_default(),
        authors,
        license:      license.unwrap_or_else(|| "MIT OR Apache-2.0".into()),
        edition,
        yog_api_path,
        yog_api_version,
        dependencies: filtered_deps,
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
    let spec = trimmed[eq_pos+1..].trim().to_string();
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
    let inner = line.find('[').and_then(|s| line.rfind(']').map(|e| &line[s+1..e]));
    inner.unwrap_or("").split(',')
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
        name    = name,
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
        name        = name,
        struct_name = to_struct_name(name),
    );
    write_file(&root.join("src/lib.rs"), lib_rs.as_bytes())?;

    // .gitignore
    write_file(&root.join(".gitignore"), b".yog-build/\ntarget/\nartifacts/\n")?;

    eprintln!("==> created {name}/");
    eprintln!("    yog.toml       ← edit mod metadata here");
    eprintln!("    src/lib.rs     ← write your mod here");
    eprintln!("");
    eprintln!("Next: cd {name} && yog build");
    Ok(())
}

fn to_display_name(id: &str) -> String {
    id.replace('-', " ").split_whitespace()
        .map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().to_string()).unwrap_or_default() + c.as_str() })
        .collect::<Vec<_>>().join(" ")
}

fn to_struct_name(id: &str) -> String {
    id.split(|c: char| c == '-' || c == '_')
        .map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().to_string()).unwrap_or_default() + c.as_str() })
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
            if out.chars().last() != Some('_') { out.push('_'); }
        } else if c.is_uppercase() {
            if !out.is_empty() && out.chars().last() != Some('_') { out.push('_'); }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// ── yog build ─────────────────────────────────────────────────────────────────

/// A platform Yog can target.
struct Target { triple: &'static str, tag: &'static str, os: &'static str }

const TARGETS: &[Target] = &[
    Target { triple: "x86_64-unknown-linux-gnu",  tag: "linux-x86_64",    os: "linux"   },
    Target { triple: "aarch64-unknown-linux-gnu", tag: "linux-aarch64",   os: "linux"   },
    Target { triple: "x86_64-pc-windows-gnu",     tag: "windows-x86_64",  os: "windows" },
    Target { triple: "x86_64-apple-darwin",       tag: "macos-x86_64",    os: "macos"   },
    Target { triple: "aarch64-apple-darwin",      tag: "macos-aarch64",   os: "macos"   },
];

/// Builds and packages the mod in the current directory. Returns the parsed
/// `yog.toml` metadata and the path to the produced `.yog` artifact, so
/// callers like `run_config` can reuse them without re-parsing/rebuilding.
fn build() -> Result<(YogToml, PathBuf), String> {
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
        let abs = root.join(rel).canonicalize()
            .unwrap_or_else(|_| root.join(rel));
        meta.yog_api_path = Some(abs.to_string_lossy().into_owned());
    }

    // Generate .yog-build/Cargo.toml from yog.toml
    let build_dir = root.join(".yog-build");
    std::fs::create_dir_all(&build_dir).map_err(|e| e.to_string())?;
    let cargo_toml = generate_cargo_toml(&meta);
    write_file(&build_dir.join("Cargo.toml"), cargo_toml.as_bytes())?;

    // Restore yog.lock → .yog-build/Cargo.lock so cargo respects pinned versions
    let yog_lock = root.join("yog.lock");
    let cargo_lock = build_dir.join("Cargo.lock");
    if yog_lock.exists() && !cargo_lock.exists() {
        std::fs::copy(&yog_lock, &cargo_lock).map_err(|e| e.to_string())?;
    }

    let builder = Builder::detect();
    eprintln!("==> building {} {} with `cargo {}`",
        meta.id, meta.version, builder.subcmd());

    let installed = installed_targets();
    let mut bundled: Vec<(String, PathBuf)> = Vec::new();

    for t in TARGETS {
        if !installed.iter().any(|s| s == t.triple) {
            eprintln!("    skip {} (rustup target not installed; run: yog setup)", t.tag);
            continue;
        }
        match builder.build(&build_dir, t.triple, &root) {
            Ok(()) => {
                let lib = lib_filename(&meta.id, t.os);
                // Cargo puts output under project-root/target/<triple>/release/ (we set CARGO_TARGET_DIR)
                let path = root.join("target").join(t.triple).join("release").join(&lib);
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
        return Err("no platform built — run `yog setup` to install cross-compilation tools".into());
    }

    // Save Cargo.lock as yog.lock
    let new_lock = build_dir.join("Cargo.lock");
    if new_lock.exists() {
        std::fs::copy(&new_lock, &yog_lock).map_err(|e| e.to_string())?;
    }

    let assets = gather_assets(&root);
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

// ── yog run ───────────────────────────────────────────────────────────────────

/// Build, optionally export the artifact into a dev instance's mod folder,
/// then optionally launch a configured command (e.g. the instance's server
/// or client launcher) — driven by a `[run.<config_name>]` section.
fn run_config(config_name: &str) -> Result<(), String> {
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

    let (meta, artifact) = build()?;

    if let Some(export_dir) = &cfg.export_dir {
        let dir = resolve(&root, export_dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        let dest = dir.join(format!("{}.yog", meta.id));
        std::fs::copy(&artifact, &dest).map_err(|e| format!("copying to {}: {e}", dest.display()))?;
        eprintln!("==> exported {} -> {}", meta.id, dest.display());
    }

    let Some(command) = &cfg.command else {
        eprintln!("==> [run.{config_name}] has no `command` set — export-only, nothing to launch.");
        return Ok(());
    };

    eprintln!("==> launching [run.{config_name}]: {command} {}", cfg.args.join(" "));
    let mut proc = Command::new(command);
    proc.args(&cfg.args);
    if let Some(cwd) = &cfg.cwd {
        proc.current_dir(resolve(&root, cwd));
    }
    for (k, v) in &cfg.env {
        proc.env(k, v);
    }

    let status = proc.status().map_err(|e| format!("failed to launch `{command}`: {e}"))?;
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
        let list = meta.authors.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>().join(", ");
        format!("authors      = [{list}]\n")
    };

    let mut deps_lines: Vec<String> = Vec::new();
    for (name, spec) in &meta.dependencies {
        // Heuristic: names with hyphens are Yog mods → add their exports crate instead
        // (Runtime reads [dependencies] from embedded yog.toml for load ordering.)
        if name.contains('-') {
            let exports_name = format!("{}-exports", name);
            deps_lines.push(format!("{} = {}", exports_name, spec));
        } else {
            deps_lines.push(format!("{} = {}", name, spec));
        }
    }

    format!(
        r#"# Generated by yog from yog.toml — do not edit.
[package]
name         = "{id}"
version      = "{version}"
edition      = "2021"
description  = {description:?}
{authors_line}license      = {license:?}

[lib]
name       = "{lib_name}"
crate-type = ["cdylib"]
path       = "../src/lib.rs"

[dependencies]
{api_dep}
{deps}
"#,
        id           = meta.id,
        lib_name     = to_snake_name(&meta.id),
        version      = meta.version,
        description  = meta.description,
        authors_line = authors_toml,
        license      = meta.license,
        api_dep      = meta.api_dep(),
        deps         = deps_lines.join("\n"),
    )
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

    eprintln!("");
    if rust_ok && zig_build_ok {
        eprintln!("==> all good — `yog build` should produce all 5 platforms.");
    } else if rust_ok {
        eprintln!("==> Rust OK but cross-compilation incomplete. Fix the above, then re-run `yog setup`.");
    } else {
        eprintln!("==> Install Rust first, then re-run `yog setup`.");
    }
    Ok(())
}

fn check_rust() -> bool {
    eprint!("  [?] Rust / cargo ... ");
    let ok = Command::new("cargo").arg("--version").output()
        .map(|o| o.status.success()).unwrap_or(false);
    if ok {
        let ver = Command::new("cargo").arg("--version").output()
            .ok().and_then(|o| String::from_utf8(o.stdout).ok())
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
    let ok = Command::new("cargo").args(["zigbuild", "--help"])
        .output().map(|o| o.status.success()).unwrap_or(false);
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
            let status = Command::new("cargo").args(["install", "cargo-zigbuild"]).status();
            match status {
                Ok(s) if s.success() => { eprintln!("       cargo-zigbuild installed."); return true; }
                _ => eprintln!("       Installation failed. Install manually: cargo install cargo-zigbuild"),
            }
        } else {
            eprintln!("       Skipped. Cross-compilation will only work for the host platform.");
        }
        false
    }
}

fn check_zig() -> bool {
    eprint!("  [?] zig ... ");
    let ok = Command::new("zig").arg("version").output()
        .map(|o| o.status.success()).unwrap_or(false);
    if ok {
        let ver = Command::new("zig").arg("version").output()
            .ok().and_then(|o| String::from_utf8(o.stdout).ok())
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
        if input.is_empty() || input == "y" {
            if try_install_zig() {
                return true;
            }
        }
        eprintln!("       Download zig from: https://ziglang.org/download/");
        eprintln!("       Extract, add to PATH, then re-run `yog setup`.");
        false
    }
}

fn try_install_zig() -> bool {
    // snap
    if Command::new("snap").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("       Running: snap install zig --classic --beta");
        if Command::new("snap").args(["install", "zig", "--classic", "--beta"])
            .status().map(|s| s.success()).unwrap_or(false) {
            eprintln!("       zig installed via snap.");
            return true;
        }
    }
    // brew
    if Command::new("brew").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("       Running: brew install zig");
        if Command::new("brew").args(["install", "zig"])
            .status().map(|s| s.success()).unwrap_or(false) {
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
    let needed: Vec<&str> = TARGETS.iter()
        .map(|t| t.triple)
        .filter(|triple| !installed.iter().any(|s| s == triple))
        .collect();
    if needed.is_empty() {
        eprintln!("       all 5 targets installed.");
        return;
    }
    eprintln!("       missing: {}", needed.join(", "));
    eprint!("       Install missing targets? [Y/n]: ");
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let input = input.trim().to_lowercase();
    if input.is_empty() || input == "y" {
        for triple in needed {
            eprint!("       rustup target add {triple} ... ");
            let ok = Command::new("rustup").args(["target", "add", triple])
                .status().map(|s| s.success()).unwrap_or(false);
            eprintln!("{}", if ok { "done" } else { "FAILED" });
        }
    } else {
        eprintln!("       Skipped. Only host-platform builds will work.");
    }
}

// ── Build internals ───────────────────────────────────────────────────────────

enum Builder { Zig, Cargo }

impl Builder {
    fn detect() -> Self {
        let ok = Command::new("cargo").args(["zigbuild", "--help"])
            .output().map(|o| o.status.success()).unwrap_or(false);
        if ok { Builder::Zig } else { Builder::Cargo }
    }
    fn subcmd(&self) -> &'static str { match self { Builder::Zig => "zigbuild", Builder::Cargo => "build" } }

    fn build(&self, build_dir: &Path, triple: &str, root: &Path) -> Result<(), ()> {
        let output = Command::new("cargo")
            .current_dir(build_dir)
            .env("CARGO_TARGET_DIR", root.join("target"))
            .args([self.subcmd(), "--release", "--target", triple])
            .output();
        match output {
            Ok(o) => {
                eprint!("{}", filter_benign_warnings(&String::from_utf8_lossy(&o.stderr)));
                if o.status.success() { Ok(()) } else { Err(()) }
            }
            _ => Err(()),
        }
    }
}

/// Drop the benign "xcrun … MacOSX.sdk failed" warning block that rustc emits
/// when cross-linking Apple targets without a macOS SDK (zig handles the
/// actual linking), along with the matching cargo "generated 1 warning" line.
fn filter_benign_warnings(stderr: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    let mut suppressed = 0usize;
    for line in stderr.lines() {
        let t = line.trim_start();
        if t.starts_with("warning:") && t.contains("xcrun") && t.contains("failed") {
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

fn installed_targets() -> Vec<String> {
    Command::new("rustup").args(["target", "list", "--installed"]).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).lines()
            .map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

fn lib_filename(name: &str, os: &str) -> String {
    // Must match the `[lib] name` pinned in the generated wrapper Cargo.toml
    // (see to_snake_name) — that's what cargo actually names the artifact.
    let stem = to_snake_name(name);
    match os {
        "windows" => format!("{stem}.dll"),
        "macos"   => format!("lib{stem}.dylib"),
        _         => format!("lib{stem}.so"),
    }
}

// ── Assets ───────────────────────────────────────────────────────────────────

fn gather_assets(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut present: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();

    let mut stack: Vec<PathBuf> = ["assets", "data"].iter()
        .map(|d| root.join(d)).filter(|p| p.is_dir()).collect();
    if stack.is_empty() { return out; }

    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
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

    // Collect auto-generated model/blockstate JSONs without borrow conflict
    let keys: Vec<String> = present.iter().cloned().collect();
    let mut generated: Vec<(String, Vec<u8>)> = Vec::new();
    for key in &keys {
        if let Some((ns, name)) = parse_texture(key, "item") {
            let path = format!("assets/{ns}/models/item/{name}.json");
            if present.insert(path.clone()) {
                generated.push((path, format!(r#"{{"parent":"item/generated","textures":{{"layer0":"{ns}:item/{name}"}}}}"#).into_bytes()));
            }
        } else if let Some((ns, name)) = parse_texture(key, "block") {
            for (path, json) in [
                (format!("assets/{ns}/blockstates/{name}.json"),
                 format!(r#"{{"variants":{{"":{{"model":"{ns}:block/{name}"}}}}}}"#)),
                (format!("assets/{ns}/models/block/{name}.json"),
                 format!(r#"{{"parent":"block/cube_all","textures":{{"all":"{ns}:block/{name}"}}}}"#)),
                (format!("assets/{ns}/models/item/{name}.json"),
                 format!(r#"{{"parent":"{ns}:block/{name}"}}"#)),
            ] {
                if present.insert(path.clone()) {
                    generated.push((path, json.into_bytes()));
                }
            }
        }
    }
    out.extend(generated);
    out
}

fn parse_texture(entry: &str, kind: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = entry.split('/').collect();
    if parts.len() == 5 && parts[0] == "assets" && parts[2] == "textures"
        && parts[3] == kind && parts[4].ends_with(".png")
    {
        Some((parts[1].to_string(), parts[4].strip_suffix(".png")?.to_string()))
    } else { None }
}

// ── Packaging ─────────────────────────────────────────────────────────────────

fn package(
    out: &Path, meta: &YogToml,
    bundled: &[(String, PathBuf)], assets: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (tag, native) in bundled {
        let lib_name = native.file_name().and_then(|n| n.to_str()).ok_or("bad native filename")?;
        let entry = format!("natives/{tag}/{lib_name}");
        let bytes = std::fs::read(native).map_err(|e| e.to_string())?;
        zip.start_file(&entry, opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    for (entry, bytes) in assets {
        zip.start_file(entry, opts).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }

    let platforms = bundled.iter().map(|(t, _)| format!("{t:?}")).collect::<Vec<_>>().join(", ");
    let authors = meta.authors.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>().join(", ");
    // The full metadata travels inside the archive so the loader can show it
    // (mod list UI etc.) without needing the source project.
    let manifest = format!(
        "id = {:?}\nname = {:?}\nversion = {:?}\ndescription = {:?}\nauthors = [{}]\nlicense = {:?}\nabi = 2\nplatforms = [{platforms}]\n",
        meta.id, meta.name, meta.version, meta.description, authors, meta.license,
    );
    zip.start_file("yog.toml", opts).map_err(|e| e.to_string())?;
    zip.write_all(manifest.as_bytes()).map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

// ── yog add / remove ──────────────────────────────────────────────────────────

fn add_dep(crate_name: Option<&str>) -> Result<(), String> {
    let name = crate_name.ok_or("usage: yog add <crate>")?;
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let yog_toml_path = root.join("yog.toml");
    if !yog_toml_path.exists() {
        return Err("no yog.toml found in the current directory".into());
    }

    let text = std::fs::read_to_string(&yog_toml_path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    
    // Check if [dependencies] section exists
    let mut has_deps = false;
    for line in &lines {
        if line.trim().starts_with('[') && line.trim().contains("dependencies") {
            has_deps = true;
            break;
        }
    }

    if !has_deps {
        // Add [dependencies] section before any [dev] section or at the end
        let insert_idx = lines.iter().position(|l| l.trim().starts_with('[') && l.trim() != "[mod]" && l.trim() != "[package]")
            .unwrap_or(lines.len());
        lines.insert(insert_idx, "[dependencies]".to_string());
        lines.insert(insert_idx + 1, format!("{} = \"*\"", name));
    } else {
        // Find [dependencies] and add the crate
        let mut in_deps = false;
        for i in 0..lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with('[') && !trimmed.contains("dependencies") {
                if in_deps {
                    // We passed the end of [dependencies]
                    lines.insert(i, format!("{} = \"*\"", name));
                    break;
                }
            } else if trimmed == "[dependencies]" {
                in_deps = true;
            }
        }
        if !in_deps {
            // [dependencies] was at the end
            lines.push(format!("{} = \"*\"", name));
        }
    }

    let new_text = lines.join("\n") + "\n";
    std::fs::write(&yog_toml_path, new_text).map_err(|e| e.to_string())?;
    eprintln!("==> added {} to yog.toml", name);
    Ok(())
}

fn remove_dep(crate_name: Option<&str>) -> Result<(), String> {
    let name = crate_name.ok_or("usage: yog remove <crate>")?;
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let yog_toml_path = root.join("yog.toml");
    if !yog_toml_path.exists() {
        return Err("no yog.toml found in the current directory".into());
    }

    let text = std::fs::read_to_string(&yog_toml_path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = Vec::new();
    let mut removed = false;

    for line in text.lines() {
        let trimmed = line.trim();
        // Remove lines like: crate-name = "..." or crate-name = { ... }
        if trimmed.starts_with(name) && trimmed.contains('=') && !trimmed.starts_with('[') {
            removed = true;
            continue;
        }
        lines.push(line.to_string());
    }

    if !removed {
        return Err(format!("dependency {} not found in yog.toml", name));
    }

    let new_text = lines.join("\n") + "\n";
    std::fs::write(&yog_toml_path, new_text).map_err(|e| e.to_string())?;
    eprintln!("==> removed {} from yog.toml", name);
    Ok(())
}

// ── yog publish exports ────────────────────────────────────────────────────────

/// Scans the mod for `#[yog_export]` items, generates an `-exports` crate,
/// and publishes it to crates.io.
fn publish_exports() -> Result<(), String> {
    let proj = std::env::current_dir().map_err(|e| e.to_string())?;
    let manifest = YogToml::read(&proj.join("yog.toml"))?;

    let mod_id = manifest.id;
    let version = manifest.version;
    let edition = manifest.edition.as_deref().unwrap_or("2021");
    let license = &manifest.license;
    let authors = manifest.authors.join(", ");
    let exports_crate_name = format!("{}-exports", mod_id.replace('_', "-"));

    let src_dir = proj.join("src");
    if !src_dir.exists() {
        return Err("no src/ directory — nothing to export".into());
    }

    // Scan .rs files for #[yog_export] items
    let mut exports = Vec::new(); // (path, item_type, name, source_lines)
    for entry in std::fs::read_dir(&src_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") { continue; }
        let src = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        for (line_no, line) in src.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[yog_export") {
                // Extract the item type and name from the following line
                let next_line = src.lines().nth(line_no + 1).unwrap_or("");
                if next_line.trim().starts_with("pub fn ") {
                    let name = extract_fn_name(next_line);
                    exports.push((path.clone(), "fn", name, next_line.to_string()));
                } else if next_line.trim().starts_with("pub struct ") {
                    let name = extract_struct_name(next_line);
                    exports.push((path.clone(), "struct", name, next_line.to_string()));
                }
            }
        }
    }

    if exports.is_empty() {
        eprintln!("==> no #[yog_export] items found — nothing to publish");
        return Ok(());
    }

    eprintln!("==> generating {exports_crate_name} ({items} export(s))",
        items = exports.len());

    // Generate the exports crate in .yog-build/exports/
    let build_dir = proj.join(".yog-build").join("exports").join(&exports_crate_name);
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

    // lib.rs — generate wrapper module
    let mod_ident = mod_id.replace('-', "_");
    let mut lib_rs = format!(
        "//! Auto-generated exports from `{mod_id}` v{version}.\n\
         //! DO NOT EDIT — generated by `yog publish exports`.\n\n\
         pub mod {mod_ident} {{\n"
    );

    for (_path, item_type, name, _source_line) in &exports {
        match item_type.as_ref() {
            "struct" => {
                // Struct: re-export with rkyv derives
                // We can't include the original struct body here (it's in the other crate).
                // Instead, we generate a transparent newtype wrapper?
                // Better: just generate the rkyv-compatible type definition.
                // For now: the user gets struct definitions auto-pasted as is.
                // In the real implementation, we'd parse the struct and regenerate it.
                lib_rs.push_str(&format!(
                    "    // struct {name} — re-exported from {mod_id}\n\
                     "    // (struct definitions need to be duplicated in the exports crate)\n"
                ));
            }
            "fn" => {
                lib_rs.push_str(&format!(
                    r#"    pub fn {name}(/* args serialized via rkyv */) {{
        // Auto-generated wrapper — calls through interop to {mod_id}
        static SLOT: std::sync::OnceLock<unsafe extern "C" fn(
            *const u8, u32, *mut *mut u8, *mut u32, *mut u32,
        )> = std::sync::OnceLock::new();

        // For now: stub. Real implementation in Phase 2.
        todo!("yog publish exports: function wrappers coming soon");
    }}

    #[doc(hidden)]
    #[no_mangle]
    pub unsafe extern "C" fn __yog_bind_{name}(ptr: *const std::os::raw::c_void) {{
        SLOT.set(std::mem::transmute(ptr)).ok();
    }}
"#,
                ));
            }
            _ => {}
        }
    }

    lib_rs.push_str("}\n");
    write_file(&build_dir.join("src").join("lib.rs"), lib_rs.as_bytes())?;

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

fn extract_fn_name(line: &str) -> String {
    // "pub fn register_pipe(...)" → "register_pipe"
    line.trim()
        .trim_start_matches("pub ")
        .trim_start_matches("fn ")
        .split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn extract_struct_name(line: &str) -> String {
    // "pub struct PipeDef {" → "PipeDef"
    line.trim()
        .trim_start_matches("pub ")
        .trim_start_matches("struct ")
        .split('{')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("writing {}: {e}", path.display()))
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
                    parts.next();                              // skip crate name (e.g. "yog-api")
                    if parts.next() == Some("=") {            // skip the "=" separator
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
