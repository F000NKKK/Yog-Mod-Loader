//! What every command that compiles does first: `obj/` regenerated,
//! resources and content compiled, the lock made to agree with this yog.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use yog_cli::project::bench;
use yog_cli::project::cargo_toml::generate_cargo_toml;
use yog_cli::project::manifest::{MANIFEST, YogToml, project_root};
use yog_cli::ui::yui;
use yog_cli::{content, diagnostics, yres};
use yog_ids::{build_problem, rust_crate};

use super::steps::warn_all;
use super::targets::generate_rust_toolchain;

/// A mod project made ready for cargo: `obj/` (Cargo.toml, generated
/// `.yui` screen code, toolchain, lock) written, translations compiled.
pub(crate) struct Prepared {
    pub(super) root: PathBuf,
    pub(super) meta: YogToml,
    pub(super) build_dir: PathBuf,
    pub(super) resources: yres::Compiled,
    /// `content/**/*.yblock|*.yitem` — blocks and items.
    pub(super) content: content::Compiled,
}

/// What every command that compiles does first (`yog build`, `yog check`):
/// regenerates `obj/` from `yog.toml` and `src/**/*.yui`, and checks
/// the translations.
pub(crate) fn prepare() -> Result<Prepared, String> {
    let root = project_root()?;
    let mut meta = read_meta(&root).map_err(|e| diagnostics::failed(build_problem::MANIFEST, e))?;
    // Resources first: `.yui` screens and yog.toml (name, description,
    // icon) refer to them.
    let resources = yres::compile(&root, &meta.id)?;
    warn_all(&resources.warnings);
    yres::write_sources(&root, &resources)?;
    resolve_metadata(&mut meta, &resources.refs)
        .map_err(|e| diagnostics::failed(build_problem::MANIFEST, e))?;
    yui::set_resource_refs(&resources.refs);
    // Blocks and items: models, states, loot, lang and their Rust glue.
    let content = content::compile(&root, &meta.id, &resources.refs)?;
    // Yog Script: read and resolved, its errors stop the build.
    yog_cli::script::check(&root, meta.datapack.as_deref())?;
    warn_all(&content.warnings);
    content::write_source(&root, &content)?;
    let build_dir = write_build_dir(&root, &meta)?;
    settle_lock(&root, &build_dir)?;
    Ok(Prepared {
        root,
        meta,
        build_dir,
        resources,
        content,
    })
}

/// What forbids a mod's own `Cargo.toml`: the build writes the real one.
const CARGO_TOML: &str = "Cargo.toml";
const CARGO_LOCK: &str = "Cargo.lock";

/// The mod's `yog.toml` — never a `Cargo.toml` beside it — with
/// `yog_api_path` made absolute (Cargo.toml lives one level deeper).
fn read_meta(root: &Path) -> Result<YogToml, String> {
    if root.join(CARGO_TOML).exists() {
        return Err("Cargo.toml found in mod root — this is forbidden. Use yog.toml only. Remove Cargo.toml and retry.".into());
    }
    let mut meta = YogToml::read(&root.join(MANIFEST))?;
    if let Some(rel) = &meta.yog_api_path {
        let abs = root
            .join(rel)
            .canonicalize()
            .unwrap_or_else(|_| root.join(rel));
        meta.yog_api_path = Some(abs.to_string_lossy().into_owned());
    }
    Ok(meta)
}

/// `obj/`: its `Cargo.toml`, the benches', the screens' code, the
/// toolchain; the editor pointed at it.
fn write_build_dir(root: &Path, meta: &YogToml) -> Result<PathBuf, String> {
    let build_dir = root.join(yog_ids::project::OBJ);
    std::fs::create_dir_all(&build_dir).map_err(|e| e.to_string())?;
    let benches = bench::discover(root).map_err(diagnostics::reported)?;
    bench::write_sources(root, &benches)?;
    write_file(
        &build_dir.join(CARGO_TOML),
        generate_cargo_toml(meta, &benches).as_bytes(),
    )?;
    let yui::Generated { screens, warnings } =
        yui::generate_all(root).map_err(diagnostics::reported)?;
    warn_all(&warnings);
    if screens > 0 {
        eprintln!(
            "    generated code for {screens} .yui screen(s) into {}/",
            yui::GENERATED_DIR
        );
    }
    configure_editor(root).map_err(|e| format!("configuring .vscode/settings.json: {e}"))?;
    write_file(
        &build_dir.join("rust-toolchain.toml"),
        generate_rust_toolchain().as_bytes(),
    )?;
    Ok(build_dir)
}

/// The lock cargo builds with: `yog.lock` restored, old loader crates
/// moved up, settled as cargo will build it — one version of each loader crate.
fn settle_lock(root: &Path, build_dir: &Path) -> Result<(), String> {
    let yog_lock = root.join(yog_ids::file::LOCK);
    let cargo_lock = build_dir.join(CARGO_LOCK);
    if yog_lock.exists() && !cargo_lock.exists() {
        std::fs::copy(&yog_lock, &cargo_lock).map_err(|e| e.to_string())?;
    }
    update_old_deps(build_dir, &cargo_lock);
    // The lock as cargo will build with it: made, or — a dependency the
    // manifest changed (yog-api pinned up, a patch) — moved, what's left
    // over gone. When it can't, the build says why (cargo's own error).
    let args: &[&str] = if cargo_lock.exists() {
        &["metadata", "-q", "--format-version", "1"]
    } else {
        &["generate-lockfile", "-q"]
    };
    let locked = Command::new("cargo")
        .args(args)
        .current_dir(build_dir)
        .stdout(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !locked {
        eprintln!(
            "warning: `cargo {}` failed — the build will say why",
            args[0]
        );
    }
    one_loader_version(&cargo_lock)
}

/// The oldest versions of the loader crates this yog's generated code and
/// the current runtime's ABI (1.3) need — a mod's lock pinning older ones
/// moves up (the runtime refuses a mod of another ABI; screens generated
/// for a newer yog-kit don't compile).
pub(crate) const MIN_VERSIONS: &[(&str, (u64, u64, u64))] =
    &[(rust_crate::API, (0, 93, 0)), (rust_crate::KIT, (0, 4, 0))];

/// The loader's crates a mod links, one version each.
pub(crate) const LOADER_CRATES: [&str; 6] = [
    rust_crate::API,
    rust_crate::ABI,
    rust_crate::UI,
    rust_crate::KIT,
    rust_crate::GFX,
    rust_crate::REGISTRY,
];

/// Moves crates of the lock at `cargo_lock` older than [`MIN_VERSIONS`] up
/// to the newest compatible ones (with what they need).
/// The loader's crates a mod links must be one version each: two
/// `yog-api`s (a dependency built against another minor) keep two sets of
/// the loader's statics, and what one registers the other never sees
/// (cross-mod imports go unbound, …).
pub(crate) fn one_loader_version(cargo_lock: &Path) -> Result<(), String> {
    let Ok(lock) = std::fs::read_to_string(cargo_lock) else {
        return Ok(());
    };
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        let Some(name) = line
            .trim()
            .strip_prefix("name = \"")
            .and_then(|n| n.strip_suffix('"'))
        else {
            continue;
        };
        if !LOADER_CRATES.contains(&name) {
            continue;
        }
        if let Some(v) = lines
            .next()
            .and_then(|l| l.trim().strip_prefix("version = \""))
        {
            seen.entry(name.to_string())
                .or_default()
                .push(v.trim_end_matches('"').to_string());
        }
    }
    let twice: Vec<String> = seen
        .iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(name, v)| format!("{name} {}", v.join(" and ")))
        .collect();
    if twice.is_empty() {
        return Ok(());
    }
    Err(format!(
        "the mod would link two versions of the loader's crates: {} — a dependency (an exports crate, …) is built against another minor; update it (or this mod's yog.toml) so they agree",
        twice.join("; ")
    ))
}

pub(crate) fn update_old_deps(build_dir: &Path, cargo_lock: &Path) {
    let Ok(lock) = std::fs::read_to_string(cargo_lock) else {
        return;
    };
    let mut lines = lock.lines();
    let mut old = Vec::new();
    while let Some(line) = lines.next() {
        let Some(name) = line
            .trim()
            .strip_prefix("name = \"")
            .and_then(|n| n.strip_suffix('"'))
        else {
            continue;
        };
        let Some((_, min)) = MIN_VERSIONS.iter().find(|(n, _)| *n == name) else {
            continue;
        };
        if let Some(v) = lines
            .next()
            .and_then(|l| l.trim().strip_prefix("version = \""))
        {
            let mut n = v
                .trim_end_matches('"')
                .split('.')
                .map(|p| p.parse::<u64>().unwrap_or(0));
            let ver = (
                n.next().unwrap_or(0),
                n.next().unwrap_or(0),
                n.next().unwrap_or(0),
            );
            if ver < *min {
                old.push(name.to_string());
            }
        }
    }
    if old.is_empty() {
        return;
    }
    eprintln!(
        "==> updating {} (the lock pins versions older than this yog needs)",
        old.join(", ")
    );
    let mut cmd = Command::new("cargo");
    cmd.args(["update", "-q", "--recursive"])
        .current_dir(build_dir);
    for name in &old {
        cmd.args(["-p", name]);
    }
    if !cmd.status().is_ok_and(|s| s.success()) {
        eprintln!("warning: `cargo update` failed — the build may not match the runtime");
    }
}

/// Make the editor "just work" for a `yog.toml`-only project: rust-analyzer
/// has nothing to find at the project root (a `Cargo.toml` there is
/// forbidden — `obj/Cargo.toml` is the real one), so
/// `rust-analyzer.linkedProjects` in `.vscode/settings.json` points it at
/// the generated one.
///
/// Merges into whatever the author already has — an existing `.vscode/
/// settings.json` (their own editor prefs) is read and only that key is
/// touched, keeping every entry the author already listed.
pub(crate) fn configure_editor(root: &Path) -> Result<(), String> {
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
        let wanted = serde_json::json!(yog_ids::concatcp!(
            yog_ids::project::OBJ,
            yog_ids::sep::PATH,
            "Cargo.toml"
        ));
        if !arr.contains(&wanted) {
            arr.push(wanted);
        }
    }

    let pretty = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    write_file(&settings_path, (pretty + "\n").as_bytes())
}

/// `yog.toml`'s `name`/`description = "@<Set>.<name>"` → the neutral text
/// (the key goes into the packaged manifest), `icon = "@<Set>.<name>"` →
/// the image source.
pub(crate) fn resolve_metadata(meta: &mut YogToml, refs: &yres::Refs) -> Result<(), String> {
    let at = |e: String| format!("yog.toml: {e}");
    (meta.name, meta.name_key) = yres::resolve_text(refs, &meta.name).map_err(at)?;
    (meta.description, meta.description_key) =
        yres::resolve_text(refs, &meta.description).map_err(at)?;
    if let Some(icon) = &meta.icon {
        meta.icon = Some(yres::resolve_image(refs, icon).map_err(at)?);
    }
    Ok(())
}

pub(crate) fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("writing {}: {e}", path.display()))
}
