//! `yog build`, `yog check`, `yog bench`, `yog designer-lib`: the mod
//! compiled, its assets gathered, the `.yog` packaged.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use yog_cli::manifest::{MANIFEST, YogToml};
use yog_cli::{bench, content};

use super::prepare::{Prepared, prepare};
use super::targets::{
    Builder, TARGETS, Target, filter_benign_warnings, install_targets, installed_targets,
    lib_filename, selected_targets,
};

/// `yog check` — `cargo check` of the mod for the host platform, after the
/// same generation `yog build` does; nothing is packaged. Quick feedback
/// (and fresh `.yog-build/generated/` for rust-analyzer) without a build.
pub(super) fn check() -> Result<(), String> {
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
pub(super) fn run_benches(args: &[String]) -> Result<(), String> {
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
pub(super) fn designer_lib() -> Result<(), String> {
    let Prepared {
        root,
        meta,
        build_dir,
        ..
    } = prepare()?;
    let target_dir = root.join("target").join("designer");
    let out = Command::new("cargo")
        .current_dir(&build_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args(["build", "--lib", "--color", "never"])
        .stdout(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    // The toolchain's own noise left out, as `yog build` does.
    eprint!(
        "{}",
        filter_benign_warnings(&String::from_utf8_lossy(&out.stderr))
    );
    if !out.status.success() {
        return Err(format!("cargo build failed ({})", out.status));
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
/// `debugging_symbols` (`yog build debug` / `yog run <target> --debug` at the
/// CLI surface): still a `--release` build (same output layout `package()`
/// expects), but unoptimized and with debug info retained instead of
/// stripped — for `yog-symbols`/`yog-debugger` to have something to load
/// and step through line by line. No separate artifact layout: the native
/// this produces is simply larger, slower and symbol-ful, packaged the same
/// way.
pub(super) fn build(debugging_symbols: bool, dev: bool) -> Result<(YogToml, PathBuf), String> {
    let Prepared {
        root,
        meta,
        build_dir,
        resources,
        content,
    } = prepare()?;
    let bundled = build_natives(&root, &meta, &build_dir, debugging_symbols)?;
    // Save Cargo.lock as yog.lock
    let new_lock = build_dir.join("Cargo.lock");
    if new_lock.exists() {
        std::fs::copy(&new_lock, root.join(yog_ids::file::LOCK)).map_err(|e| e.to_string())?;
    }
    let assets = assets(&root, resources, content, dev)?;
    let artifacts = root.join(ARTIFACTS_DIR);
    std::fs::create_dir_all(&artifacts).map_err(|e| e.to_string())?;
    let out = artifacts.join(yog_ids::Format::Mod.file(&meta.id));
    package(&out, &meta, &bundled, &assets)?;
    let tags: Vec<&str> = bundled.iter().map(|(t, _)| t.as_str()).collect();
    eprintln!("==> packaged {} [{}]", out.display(), tags.join(", "));
    Ok((meta, out))
}

/// Where `yog build` puts the `.yog`, under the mod's root.
pub(super) const ARTIFACTS_DIR: &str = "artifacts";

/// The platforms `YOG_TARGETS` picks (all when unset).
fn wanted_targets() -> Vec<&'static Target> {
    let selected = selected_targets();
    TARGETS
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
    debugging_symbols: bool,
) -> Result<Vec<(String, PathBuf)>, String> {
    let builder = Builder::detect();
    eprintln!(
        "==> building {} {} with `cargo {}`",
        meta.id,
        meta.version,
        builder.subcmd()
    );
    let wanted = wanted_targets();
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
        if builder
            .build(build_dir, t.triple, root, debugging_symbols)
            .is_err()
        {
            failed += 1;
            eprintln!("    skip {} (build failed)", t.tag);
            continue;
        }
        let lib = lib_filename(&meta.id, t.os);
        // Cargo puts output under project-root/target/<triple>/release/ (we set CARGO_TARGET_DIR)
        let path = root
            .join(TARGET_DIR)
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
        return Err(if failed > 0 {
            "no platform built — the build failed (errors above)".into()
        } else {
            "no platform built — run `yog setup` to install cross-compilation tools".into()
        });
    }
    Ok(bundled)
}

/// Cargo's output folders: its target folder (the mod root's), a profile's.
const TARGET_DIR: &str = "target";
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
    let assets = gather_assets(root, compiled);
    if !assets.is_empty() {
        eprintln!("    bundled {} asset file(s)", assets.len());
    }
    Ok(assets)
}

/// The mod's `assets/` and `data/` files plus `compiled` ones (`.yres`
/// resources, `content/` blocks and items — replacing a file at the same
/// path).
pub(super) fn gather_assets(
    root: &Path,
    compiled: Vec<(String, Vec<u8>)>,
) -> Vec<(String, Vec<u8>)> {
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
                let key = yog_cli::slash_path(rel);
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

pub(super) fn package(
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
