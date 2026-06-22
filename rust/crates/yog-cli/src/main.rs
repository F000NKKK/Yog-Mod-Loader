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

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("build")          => build(),
        Some("new")            => new_mod(args.get(2).map(String::as_str)),
        Some("setup")          => setup(),
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
         \x20 new <name>   Create a new mod project in ./<name>/\n\
         \x20 build        Compile the current mod and package it as artifacts/<id>.yog\n\
         \x20 setup        Check and install build dependencies (Rust, Zig, targets)\n\
         \x20 help         Show this message\n\n\
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
    /// Optional: path to yog-api for local/monorepo development.
    /// Set via [dev] yog_api_path = "..."  or YOG_API_PATH env var.
    yog_api_path: Option<String>,
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
    ///   3. future: crates.io
    fn api_dep(&self) -> String {
        if let Ok(p) = std::env::var("YOG_API_PATH") {
            // Resolve to absolute so it works from any subdirectory
            let abs = PathBuf::from(&p).canonicalize().unwrap_or_else(|_| PathBuf::from(&p));
            return format!("yog-api = {{ path = {:?} }}", abs.to_string_lossy());
        }
        if let Some(p) = &self.yog_api_path {
            return format!("yog-api = {{ path = {p:?} }}");
        }
        // crates.io — not yet published; this is a forward-looking placeholder.
        "yog-api = \"0.1\"".into()
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
    let mut yog_api_path  = None::<String>;

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
                if line.trim_start().starts_with("authors") {
                    authors = parse_string_array(line);
                }
            }
            "dev" => {
                if let Some(v) = field(line, "yog_api_path") { yog_api_path = Some(v); }
            }
            _ => {}
        }
    }

    let id = id.ok_or("yog.toml: missing [mod] id")?;
    Ok(YogToml {
        name:         name.unwrap_or_else(|| id.clone()),
        version:      version.unwrap_or_else(|| "0.1.0".into()),
        description:  description.unwrap_or_default(),
        authors,
        license:      license.unwrap_or_else(|| "MIT OR Apache-2.0".into()),
        yog_api_path,
        id,
    })
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

fn build() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let yog_toml_path = root.join("yog.toml");
    if !yog_toml_path.exists() {
        return Err("no yog.toml found in the current directory".into());
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
    package(&out, &meta.id, &meta.name, &meta.version, &bundled, &assets)?;

    let tags: Vec<&str> = bundled.iter().map(|(t, _)| t.as_str()).collect();
    eprintln!("==> packaged {} [{}]", out.display(), tags.join(", "));
    Ok(())
}

/// Generate the hidden Cargo.toml from yog.toml metadata.
fn generate_cargo_toml(meta: &YogToml) -> String {
    let authors_toml = if meta.authors.is_empty() {
        String::new()
    } else {
        let list = meta.authors.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>().join(", ");
        format!("authors      = [{list}]\n")
    };

    format!(
        r#"# Generated by yog from yog.toml — do not edit.
[package]
name         = "{id}"
version      = "{version}"
edition      = "2021"
description  = {description:?}
{authors_line}license      = {license:?}

[lib]
crate-type = ["cdylib"]
path       = "../src/lib.rs"

[dependencies]
{api_dep}
"#,
        id           = meta.id,
        version      = meta.version,
        description  = meta.description,
        authors_line = authors_toml,
        license      = meta.license,
        api_dep      = meta.api_dep(),
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
        let status = Command::new("cargo")
            .current_dir(build_dir)
            .env("CARGO_TARGET_DIR", root.join("target"))
            .args([self.subcmd(), "--release", "--target", triple])
            .status();
        match status {
            Ok(s) if s.success() => Ok(()),
            _ => Err(()),
        }
    }
}

fn installed_targets() -> Vec<String> {
    Command::new("rustup").args(["target", "list", "--installed"]).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).lines()
            .map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

fn lib_filename(name: &str, os: &str) -> String {
    let stem = name.replace('-', "_");
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
    out: &Path, id: &str, name: &str, version: &str,
    bundled: &[(String, PathBuf)], assets: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::FileOptions::default()
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
    let manifest = format!(
        "id = {id:?}\nname = {name:?}\nversion = {version:?}\nabi = 2\nplatforms = [{platforms}]\n"
    );
    zip.start_file("yog.toml", opts).map_err(|e| e.to_string())?;
    zip.write_all(manifest.as_bytes()).map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("writing {}: {e}", path.display()))
}
