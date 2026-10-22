//! `yog publish exports`: the mod's `#[yog_export]` items as a crate on crates.io.

use yog_cli::exports::{self, ExportItem, collect_rs_files};
use yog_cli::manifest::{
    DEFAULT_EDITION, DEFAULT_LICENSE, MANIFEST, YogToml, latest_yog_api_version, project_root,
};

use super::prepare::write_file;

/// What a mod's exports crate is made of.
struct Crate {
    name: String,
    mod_id: String,
    version: String,
    items: Vec<ExportItem>,
    /// The mod's sources, for the imports its items need.
    sources: Vec<String>,
}

/// The mod's `#[yog_export]` items, and what they're published under.
fn scan(proj: &std::path::Path, manifest: &YogToml) -> Result<Crate, String> {
    let src_dir = proj.join(yog_ids::project::SRC);
    if !src_dir.exists() {
        return Err("no src/ directory — nothing to export".into());
    }
    let sources = collect_rs_files(&src_dir)?
        .iter()
        .map(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let items = sources.iter().flat_map(|src| exports::scan(src)).collect();
    Ok(Crate {
        name: exports::crate_name(&manifest.id),
        mod_id: manifest.id.clone(),
        version: manifest.version.clone(),
        items,
        sources,
    })
}

/// Writes the exports crate into `dir`: its `Cargo.toml` and `src/lib.rs`.
fn write_crate(dir: &std::path::Path, c: &Crate, manifest: &YogToml) -> Result<(), String> {
    let (system_uses, type_uses) = exports::collect_uses(&c.items, &c.sources);
    eprintln!("==> generating {} ({} export(s))", c.name, c.items.len());
    eprintln!("    system_uses: {system_uses:?}");
    eprintln!("    type_uses: {type_uses:?}");
    yog_cli::files::remove(dir);
    std::fs::create_dir_all(dir.join(yog_ids::project::SRC)).map_err(|e| e.to_string())?;
    // Exports crates default to MIT OR Apache-2.0 regardless of the mod's own
    // license — an AGPL mod would otherwise force every consumer's dependency
    // tree under AGPL too, which kills adoption. `[mod] exports_license = "..."`
    // opts into a different license.
    let license = manifest
        .exports_license
        .clone()
        .unwrap_or_else(|| DEFAULT_LICENSE.to_string());
    // Resolve versions dynamically (same logic as `yog build`)
    let latest = latest_yog_api_version();
    let cargo_toml = exports::cargo_toml(&exports::CrateMeta {
        name: &c.name,
        version: &c.version,
        edition: manifest.edition.as_deref().unwrap_or(DEFAULT_EDITION),
        authors: &manifest.authors.join(", "),
        license: &license,
        mod_id: &c.mod_id,
        yog_api: manifest.yog_api_version.as_deref().unwrap_or(&latest),
    });
    write_file(&dir.join("Cargo.toml"), cargo_toml.as_bytes())?;
    let lib_rs = exports::lib_rs(&c.mod_id, &c.version, &c.items, &system_uses, &type_uses);
    write_file(
        &dir.join(yog_ids::project::SRC).join("lib.rs"),
        lib_rs.as_bytes(),
    )
}

/// Scans the mod for `#[yog_export]` items, generates an `-exports` crate,
/// and publishes it to crates.io (`dry_run`: only generates it).
pub(super) fn publish_exports(dry_run: bool) -> Result<(), String> {
    let proj = project_root()?;
    let manifest = YogToml::read(&proj.join(MANIFEST))?;
    let c = scan(&proj, &manifest)?;
    if c.items.is_empty() {
        eprintln!("==> no #[yog_export] items found — nothing to publish");
        return Ok(());
    }
    // Generated in .yog-build/exports/<crate>/.
    let dir = proj
        .join(yog_cli::manifest::BUILD_DIR)
        .join(EXPORTS_DIR)
        .join(&c.name);
    write_crate(&dir, &c, &manifest)?;
    if dry_run {
        eprintln!("==> dry-run: skipping cargo publish");
        eprintln!("    generated crate is at: {}", dir.display());
        eprintln!("    Run `cargo publish` manually from that directory, or");
        eprintln!("    re-run without --dry-run to publish automatically.");
        return Ok(());
    }
    eprintln!("==> cargo publish (--allow-dirty)");
    let status = std::process::Command::new("cargo")
        .args(["publish", "--allow-dirty"])
        .current_dir(&dir)
        .status()
        .map_err(|e| format!("cargo publish failed: {e}"))?;
    if !status.success() {
        return Err(format!("cargo publish exited with {status}"));
    }
    yog_cli::files::remove(&dir);
    eprintln!("==> published {} v{}", c.name, c.version);
    Ok(())
}

/// Where exports crates are generated, under `.yog-build/`.
const EXPORTS_DIR: &str = "exports";
