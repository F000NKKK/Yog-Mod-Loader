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

use yog_cli::cargo_toml::generate_cargo_toml;
use yog_cli::exports::{self, ExportItem, collect_rs_files};
use yog_cli::manifest::{
    BUILD_DIR, DEFAULT_EDITION, DEFAULT_LICENSE, DEPS_SECTION, EXPORT_DEPS_SECTION, MANIFEST,
    YogToml, latest_yog_api_version, project_root, to_snake_name, workspace_rkyv_version,
};
use yog_cli::{bench, content, yres, yui, yui_component};

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
        Some("bench") => run_benches(&args[2..]),
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
         \x20 bench [filter]      Run the mod's benches/*.rs #[yog_api::bench] functions (host only)\n\
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
    write_file(&root.join(MANIFEST), yog_toml.as_bytes())?;

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
        struct_name = yui::pascal(name),
    );
    write_file(&root.join("src/lib.rs"), lib_rs.as_bytes())?;

    // .gitignore
    write_file(
        &root.join(".gitignore"),
        b".yog-build/\ntarget/\nartifacts/\n",
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
    let root = project_root()?;
    let yog_toml_path = root.join(MANIFEST);

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
    let build_dir = root.join(BUILD_DIR);
    std::fs::create_dir_all(&build_dir).map_err(|e| e.to_string())?;
    let benches = bench::discover(&root)?;
    bench::write_sources(&root, &benches)?;
    let cargo_toml = generate_cargo_toml(&meta, &benches);
    write_file(&build_dir.join("Cargo.toml"), cargo_toml.as_bytes())?;

    let screens = yui::generate_all(&root)?;
    if screens > 0 {
        eprintln!(
            "    generated code for {screens} .yui screen(s) into {}/",
            yui::GENERATED_DIR
        );
    }

    configure_editor(&root).map_err(|e| format!("configuring .vscode/settings.json: {e}"))?;

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

/// `yog bench [args]` — the mod's `benches/*.rs` under criterion, built
/// for the host after the same generation `yog build` does (see `bench`).
/// `args` go to the benchmarks (`yog bench parse` runs those matching
/// "parse").
fn run_benches(args: &[String]) -> Result<(), String> {
    let Prepared {
        root, build_dir, ..
    } = prepare()?;
    if bench::discover(&root)?.is_empty() {
        return Err(format!(
            "no benchmarks: add {}/<name>.rs with #[yog_api::bench] functions",
            bench::BENCH_DIR
        ));
    }
    let status = Command::new("cargo")
        .current_dir(&build_dir)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .args(["bench", "--benches", "--"])
        .args(args)
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo bench failed ({status})"));
    }
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
    let mut failed = 0;

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
            Err(_) => {
                failed += 1;
                eprintln!("    skip {} (build failed)", t.tag);
            }
        }
    }

    if bundled.is_empty() {
        return Err(if failed > 0 {
            "no platform built — the build failed (errors above)".into()
        } else {
            "no platform built — run `yog setup` to install cross-compilation tools".into()
        });
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
        let root = project_root()?;
        load_resource_refs(&root);
        for file in yui::find_yui_files(&root.join("src")) {
            let out = yui::generate_file(&root, &file)?;
            eprintln!("==> {} -> {}", file.display(), out.display());
        }
        yui_component::write_index(&root)?;
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
        yui_component::write_index(&root)?;
    }
    Ok(())
}

/// Makes the project's `.yres` resources available to `.yui` generation
/// (`text="@Strings.title"`). Broken resources are reported but don't stop
/// the screen from generating — `yog build` fails on them anyway.
fn load_resource_refs(root: &Path) {
    let refs = YogToml::read(&root.join(MANIFEST))
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
    let root = project_root()?;
    let mut meta = YogToml::read(&root.join(MANIFEST))?;
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
    let root = project_root()?;
    let meta = YogToml::read(&root.join(MANIFEST))?;
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
    let root = project_root()?;
    let yog_toml_path = root.join(MANIFEST);
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
        write_file(&source_marker(&dir, &meta.id), root.display().to_string().as_bytes())?;
        eprintln!("==> exported {} -> {}", meta.id, dest.display());
        refresh_exported(&dir, &meta.id, debugging_symbols);
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

/// `<id>.source` beside an exported `<id>.yog`: the project it was built
/// from, so the next `yog run` into that folder can rebuild it.
fn source_marker(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.source"))
}

/// Rebuilds every other mod `yog run` exported into `dir` (the ones with a
/// `<id>.source`) and re-exports it — mods from an older build would carry
/// an older ABI, and the runtime refuses those. A mod that fails to build
/// keeps its old artifact (with a warning); the run goes on.
fn refresh_exported(dir: &Path, current: &str, debugging_symbols: bool) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut others: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let id = path.file_name()?.to_str()?.strip_suffix(".source")?.to_string();
            if id == current || !dir.join(format!("{id}.yog")).is_file() {
                return None;
            }
            let source = std::fs::read_to_string(&path).ok()?;
            Some((id, PathBuf::from(source.trim())))
        })
        .collect();
    others.sort();
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    for (id, source) in others {
        if !source.join(MANIFEST).is_file() {
            eprintln!(
                "warning: {id}: {} is no longer a mod project — keeping the exported {id}.yog",
                source.display()
            );
            continue;
        }
        eprintln!("==> refreshing {id} ({})", source.display());
        let mut cmd = Command::new(&exe);
        cmd.arg("build").current_dir(&source);
        if debugging_symbols {
            cmd.arg("debug");
        }
        let built = cmd.status().is_ok_and(|s| s.success());
        let artifact = source.join("artifacts").join(format!("{id}.yog"));
        if !built || !artifact.is_file() {
            eprintln!("warning: {id} didn't build — keeping its previous {id}.yog");
            continue;
        }
        if let Err(e) = std::fs::copy(&artifact, dir.join(format!("{id}.yog"))) {
            eprintln!("warning: {id}: copying {}: {e}", artifact.display());
        }
    }
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
    if path == "~"
        && let Ok(home) = std::env::var("HOME")
    {
        return home;
    }
    if path.starts_with("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return home + &path[1..];
    }
    path.to_string()
}

/// Make the editor "just work" for a `yog.toml`-only project: rust-analyzer
/// has nothing to find at the project root (a `Cargo.toml` there is
/// forbidden — `.yog-build/Cargo.toml` is the real one), so
/// `rust-analyzer.linkedProjects` in `.vscode/settings.json` points it at
/// the generated one.
///
/// Merges into whatever the author already has — an existing `.vscode/
/// settings.json` (their own editor prefs) is read and only that key is
/// touched, keeping every entry the author already listed.
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

    let pretty = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    write_file(&settings_path, (pretty + "\n").as_bytes())
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
    if let Some(ver) = first_line_of("cargo", &["--version"]) {
        eprintln!("OK  ({ver})");
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
    if has_zigbuild() {
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
    if let Some(ver) = first_line_of("zig", &["version"]) {
        eprintln!("OK  ({ver})");
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

/// Whether `cargo zigbuild` is installed.
fn has_zigbuild() -> bool {
    Command::new("cargo")
        .args(["zigbuild", "--help"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The first line `program args` prints, if it runs and succeeds.
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
        cargo_zigbuild: has_zigbuild(),
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
        if has_zigbuild() {
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
                if o.status.success() { Ok(()) } else { Err(()) }
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
    let mut out = yog_cli::content::OutputSet::default();

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
                    out.put(key, bytes);
                }
            }
        }
    }

    for (key, bytes) in compiled {
        out.put(key, bytes);
    }
    out.into_vec()
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
    zip.start_file(MANIFEST, opts).map_err(|e| e.to_string())?;
    zip.write_all(manifest.as_bytes())
        .map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

// ── yog add / remove ──────────────────────────────────────────────────────────

/// `yog add`/`yog remove`'s operand, the `yog.toml` it edits and the
/// section: `[dependencies]` (plain crates.io crates) or
/// `[dependencies.exports]` (other Yog mods, mapped to their generated
/// exports crate) — matched by exact header, never as a substring.
fn dep_edit<'a>(
    verb: &str,
    crate_name: Option<&'a str>,
    is_mod: bool,
) -> Result<(&'a str, PathBuf, &'static str), String> {
    let name = crate_name.ok_or_else(|| {
        if is_mod {
            format!("usage: yog {verb} --mod <yog-mod-id>")
        } else {
            format!("usage: yog {verb} <crate>")
        }
    })?;
    let section = if is_mod {
        EXPORT_DEPS_SECTION
    } else {
        DEPS_SECTION
    };
    Ok((name, project_root()?.join(MANIFEST), section))
}

fn add_dep(crate_name: Option<&str>, is_mod: bool) -> Result<(), String> {
    let (name, yog_toml_path, target_section) = dep_edit("add", crate_name, is_mod)?;

    let text = std::fs::read_to_string(&yog_toml_path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

    let has_section = lines.iter().any(|l| l.trim() == target_section);

    if !has_section {
        // Add the section before any [dev] section or at the end
        let insert_idx = lines
            .iter()
            .position(|l| l.trim().starts_with('[') && l.trim() != "[mod]")
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
    let (name, yog_toml_path, target_section) = dep_edit("remove", crate_name, is_mod)?;

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
        // only within the targeted section and only `name` itself (not
        // every crate whose name it prefixes).
        if in_section
            && trimmed
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == name)
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

/// Scans the mod for `#[yog_export]` items, generates an `-exports` crate,
/// and publishes it to crates.io.
fn publish_exports(dry_run: bool) -> Result<(), String> {
    let proj = project_root()?;
    let manifest = YogToml::read(&proj.join(MANIFEST))?;

    let mod_id = manifest.id;
    let version = manifest.version;
    let edition = manifest.edition.as_deref().unwrap_or(DEFAULT_EDITION);
    // Exports crates default to MIT OR Apache-2.0 regardless of the mod's own
    // license — an AGPL mod would otherwise
    // force every consumer's dependency tree under AGPL too, which kills
    // adoption. `[mod] exports_license = "..."` opts into a different license.
    let license = manifest
        .exports_license
        .clone()
        .unwrap_or_else(|| DEFAULT_LICENSE.to_string());
    let authors = manifest.authors.join(", ");
    let exports_crate_name = format!("{}_exports", mod_id.replace('-', "_"));

    let src_dir = proj.join("src");
    if !src_dir.exists() {
        return Err("no src/ directory — nothing to export".into());
    }

    let mut sources = Vec::new();
    for path in collect_rs_files(&src_dir)? {
        sources.push(std::fs::read_to_string(&path).map_err(|e| e.to_string())?);
    }
    let items: Vec<ExportItem> = sources.iter().flat_map(|src| exports::scan(src)).collect();

    if items.is_empty() {
        eprintln!("==> no #[yog_export] items found — nothing to publish");
        return Ok(());
    }

    // Collect used namespaces from exported items
    let (system_uses, type_uses) = exports::collect_uses(&items, &sources);

    eprintln!(
        "==> generating {exports_crate_name} ({items} export(s))",
        items = items.len()
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

    let cargo_toml = exports::cargo_toml(&exports::CrateMeta {
        name: &exports_crate_name,
        version: &version,
        edition,
        authors: &authors,
        license: &license,
        mod_id: &mod_id,
        yog_api: yog_api_override,
        rkyv: &rkyv_ver,
    });
    write_file(&build_dir.join("Cargo.toml"), cargo_toml.as_bytes())?;

    let lib_rs = exports::lib_rs(&mod_id, &version, &items, &system_uses, &type_uses);
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

fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("writing {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{DoctorReport, filter_benign_warnings};

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

warning: `example` (lib) generated 1 warning
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
