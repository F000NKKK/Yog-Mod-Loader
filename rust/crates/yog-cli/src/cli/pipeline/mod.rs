//! `yog build`, `yog check`, `yog bench`, `yog designer-lib`: the mod
//! compiled, its assets gathered, the `.yog` packaged.

pub(crate) mod cargo_out;
mod pictures;
pub(crate) mod prepare;
mod prune;
pub(crate) mod steps;
pub(crate) mod symbols;
pub(crate) mod targets;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use yog_cli::project::bench;
use yog_cli::project::manifest::YogToml;
use yog_cli::{content, diagnostics};
use yog_ids::build_problem;

use prepare::{Prepared, prepare};
use yog_ids::platform::{self, Platform};

use symbols::Symbols;
use targets::{Builder, install_targets, installed_targets, lib_filename, selected_targets};

/// `yog check` — `cargo check` of the mod for the host platform, after the
/// same generation `yog build` does; nothing is packaged. Quick feedback
/// (and fresh `obj/generated/` for rust-analyzer) without a build.
pub(crate) fn check() -> Result<(), String> {
    let Prepared {
        root, build_dir, ..
    } = prepare()?;
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&build_dir)
        .env("CARGO_TARGET_DIR", root.join(yog_ids::project::TARGET))
        .arg("check");
    let ok = cargo_out::run(&mut cmd, &build_dir, &root)
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !ok {
        return Err("cargo check failed".to_owned());
    }
    eprintln!("==> check OK");
    Ok(())
}

/// `yog bench [args]` — the mod's `benches/*.rs` under criterion, built
/// for the host after the same generation `yog build` does (see `bench`).
/// `args` go to the benchmarks (`yog bench parse` runs those matching
/// "parse").
pub(crate) fn run_benches(args: &[String]) -> Result<(), String> {
    let Prepared {
        root, build_dir, ..
    } = prepare()?;
    if bench::discover(&root)
        .map_err(diagnostics::reported)?
        .is_empty()
    {
        return Err(format!(
            "no benchmarks: add {}/<name>.rs with #[yog_api::bench] functions",
            bench::BENCH_DIR
        ));
    }
    let status = Command::new("cargo")
        .current_dir(&build_dir)
        .env("CARGO_TARGET_DIR", root.join(yog_ids::project::TARGET))
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
pub(crate) fn designer_lib() -> Result<(), String> {
    let Prepared {
        root,
        meta,
        build_dir,
        ..
    } = prepare()?;
    let target_dir = root.join(yog_ids::project::TARGET).join("designer");
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&build_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args(["build", "--lib", "--color", "never"]);
    let ok = cargo_out::run(&mut cmd, &build_dir, &root)
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !ok {
        return Err("cargo build failed".to_owned());
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
/// callers like `run_target` can reuse them without re-parsing/rebuilding.
///
/// `symbols`: what the native keeps of its debug info (`cli/symbols.rs`) —
/// a release's split out to `bin/symbols/`.
/// Which platforms a build makes natives for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Platforms {
    /// `YOG_TARGETS`'s (every one when unset): `yog build`.
    Selected,
    /// This machine's only: `yog run`, which plays it here.
    Host,
}

pub(crate) fn build(
    symbols: Symbols,
    dev: bool,
    platforms: Platforms,
) -> Result<(YogToml, PathBuf), String> {
    let Prepared {
        root,
        meta,
        build_dir,
        resources,
        content,
    } = prepare()?;
    // A mod without Rust still has a native: its content's
    // (`script_lib`'s generated root).
    let bundled = build_natives(&root, &meta, &build_dir, symbols, platforms)?;
    // Save Cargo.lock as yog.lock
    let new_lock = build_dir.join("Cargo.lock");
    if new_lock.exists() {
        std::fs::copy(&new_lock, root.join(yog_ids::file::LOCK)).map_err(|e| e.to_string())?;
    }
    let ids = content.ids.clone();
    let mut assets = assets(&root, resources, content, dev)?;
    assets.extend(yog_cli::script::jvm::jars(&root, &meta, &build_dir, &ids)?);
    let bin = root.join(yog_ids::project::BIN);
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    let bundled = match symbols {
        Symbols::Split => {
            let split = symbols::split(bundled, &build_dir, &bin)?;
            eprintln!(
                "    debug info split out to {}/",
                bin.join(symbols::SYMBOLS_DIR).display()
            );
            split
        }
        Symbols::Stripped | Symbols::Kept => bundled,
    };
    let out = bin.join(yog_ids::Format::Mod.file(&meta.id));
    package(&out, &meta, &bundled, &assets)
        .map_err(|e| diagnostics::failed(build_problem::PACKAGE, e))?;
    let tags: Vec<&str> = bundled.iter().map(|(t, _)| t.as_str()).collect();
    eprintln!("==> packaged {} [{}]", out.display(), tags.join(", "));
    if bundled.len() > 1 {
        per_platform(&bin, &meta, &bundled, &assets)?;
    }
    Ok((meta, out))
}

/// Each platform's own `.yog` (`bin/<platform>/<id>.yog`): the same
/// mod with only that platform's native — what a player of it downloads,
/// a fifth of the whole.
fn per_platform(
    bin: &Path,
    meta: &YogToml,
    bundled: &[(String, PathBuf)],
    assets: &[(String, Vec<u8>)],
) -> Result<(), String> {
    for native in bundled {
        let dir = bin.join(&native.0);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let out = dir.join(yog_ids::Format::Mod.file(&meta.id));
        package(&out, meta, std::slice::from_ref(native), assets)
            .map_err(|e| diagnostics::failed(build_problem::PACKAGE, e))?;
    }
    eprintln!(
        "    and one .yog a platform, in {}/<platform>/",
        bin.display()
    );
    Ok(())
}

/// The platforms to build for: this machine's, or those `YOG_TARGETS` picks (all when unset).
fn wanted_targets(platforms: Platforms) -> Vec<&'static Platform> {
    if platforms == Platforms::Host {
        return platform::host().into_iter().collect();
    }
    let selected = selected_targets();
    platform::ALL
        .iter()
        .filter(|t| {
            selected
                .as_ref()
                .is_none_or(|sel| sel.iter().any(|s| s == t.tag || s == t.triple))
        })
        .collect()
}

/// The mod's native library built for every wanted platform whose target
/// is installed: (tag, its file). An error when none was.
fn build_natives(
    root: &Path,
    meta: &YogToml,
    build_dir: &Path,
    symbols: Symbols,
    platforms: Platforms,
) -> Result<Vec<(String, PathBuf)>, String> {
    let builder = Builder::detect();
    eprintln!(
        "==> building {} {} with `cargo {}`",
        meta.id,
        meta.version,
        builder.subcmd()
    );
    let wanted = wanted_targets(platforms);
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
        if builder.build(build_dir, t.triple, root, symbols).is_err() {
            failed += 1;
            eprintln!("    skip {} (build failed)", t.tag);
            continue;
        }
        let lib = lib_filename(&meta.id, t.os);
        // Cargo puts output under obj/target/<triple>/release/ (we set CARGO_TARGET_DIR)
        let path = root
            .join(yog_ids::project::TARGET)
            .join(t.triple)
            .join(RELEASE_DIR)
            .join(&lib);
        if path.exists() {
            eprintln!("    built {}", t.tag);
            bundled.push((t.tag.to_string(), path));
        } else {
            eprintln!("    skip {} (built but output not found: {})", t.tag, lib);
        }
    }
    if bundled.is_empty() {
        let why = if failed > 0 {
            "no platform built — the build failed (errors above)"
        } else {
            "no platform built — run `yog setup` to install cross-compilation tools"
        };
        return Err(diagnostics::failed(build_problem::NO_PLATFORM, why));
    }
    Ok(bundled)
}

/// Cargo's output folder of a release build.
const RELEASE_DIR: &str = "release";

/// What the `.yog` bundles besides the natives: the mod's files and what
/// its resources and content compiled into — and for a dev build, what it
/// registers (what hot reload checks).
fn assets(
    root: &Path,
    resources: yog_cli::yres::Compiled,
    mut content: content::Compiled,
    dev: bool,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    if resources.files > 0 {
        eprintln!(
            "    compiled {} .yres resource file(s) into {} asset file(s)",
            resources.files,
            resources.outputs.len()
        );
    }
    if content.blocks + content.items + content.recipes + content.books > 0 {
        eprintln!(
            "    compiled {} block(s), {} item(s), {} recipe(s) and {} book(s) from {}/",
            content.blocks,
            content.items,
            content.recipes,
            content.books,
            content::CONTENT_DIR
        );
    }
    let mut compiled = resources.outputs;
    // A picture an atlas took in is in the atlas, not a file of its own.
    compiled.retain(|(path, _)| !content.packed.contains(path));
    let registrations = std::mem::take(&mut content.registrations);
    content::merge_outputs(root, &mut compiled, content)?;
    if dev {
        compiled.push((
            yog_registry::REGISTRATIONS_FILE.to_string(),
            registrations.into_bytes(),
        ));
    }
    let mut assets = gather_assets(root, compiled);
    pictures::pack(&mut assets);
    if !assets.is_empty() {
        eprintln!("    bundled {} asset file(s)", assets.len());
    }
    Ok(assets)
}

/// The mod's `assets/` and `data/` files plus `compiled` ones (`.yres`
/// resources, `content/` blocks and items — replacing a file at the same
/// path).
pub(crate) fn gather_assets(
    root: &Path,
    compiled: Vec<(String, Vec<u8>)>,
) -> Vec<(String, Vec<u8>)> {
    let mut out = yog_cli::content::OutputSet::default();
    let mut left_out = 0usize;

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
            } else if is_source(&path) {
                left_out += 1;
            } else if let Ok(rel) = path.strip_prefix(root) {
                let key = yog_cli::slash_path(rel);
                if let Ok(bytes) = std::fs::read(&path) {
                    out.put(key, bytes);
                }
            }
        }
    }

    if left_out > 0 {
        eprintln!(
            "    left out {left_out} editor/source file(s) the game doesn't read ({})",
            SOURCE_FILES.join(", ")
        );
    }
    for (key, bytes) in compiled {
        out.put(key, bytes);
    }
    out.into_vec()
}

/// What a mod's `assets/` and `data/` may hold that the game never reads:
/// editors' files and scripts made them, not the game's formats.
const SOURCE_FILES: &[&str] = &[
    "aseprite", "ase", "psd", "kra", "xcf", "blend", "py", "sh", "ps1", "bat",
];

/// Whether `path` is one of [`SOURCE_FILES`].
fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SOURCE_FILES.contains(&e.to_ascii_lowercase().as_str()))
}

pub(crate) fn package(
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
        let entry = format!("{}/{tag}/{lib_name}", yog_ids::archive::NATIVES);
        let bytes = std::fs::read(native).map_err(|e| e.to_string())?;
        zip.start_file(&entry, opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    for (entry, bytes) in assets {
        zip.start_file(entry, opts).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }

    // What the loader shows of the mod and orders the mods by, without the project.
    let built = yog_formats::manifest::Built {
        id: meta.id.clone(),
        name: meta.name.clone(),
        version: meta.version.clone(),
        description: meta.description.clone(),
        authors: meta.authors.clone(),
        license: meta.license.clone(),
        platforms: bundled.iter().map(|(t, _)| t.clone()).collect(),
        name_key: meta.name_key.clone(),
        description_key: meta.description_key.clone(),
        icon: meta.icon.clone(),
        dependencies: meta
            .mod_dependencies
            .iter()
            .map(|(id, _)| id.clone())
            .collect(),
        to_server: meta.to_server,
    };
    zip.start_file(yog_ids::archive::MANIFEST, opts)
        .map_err(|e| e.to_string())?;
    zip.write_all(&yog_data::to_bytes(&built))
        .map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}
