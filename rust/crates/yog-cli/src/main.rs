//! `yog` — the Yog mod build tool.
//!
//! `yog build` (run inside a mod crate) wraps `cargo build --release` and packs
//! the resulting native library into `artifacts/<name>.yog` — a zip archive
//! laying natives out per platform (`natives/<os>-<arch>/`) plus a `yog.toml`
//! manifest. A `.yog` distinguishes Yog mods and lets the runtime pick the right
//! native at load time; players just drop it in their mods folder.

use std::io::Write;
use std::path::Path;
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("build") => build(),
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
         Usage: yog <command>\n\n\
         Commands:\n\
         \x20 build    Compile the current mod crate (release) and package it as\n\
         \x20          artifacts/<name>.yog"
    );
}

fn build() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let (name, version) = read_package(&root.join("Cargo.toml"))?;

    eprintln!("==> building {name} {version} (release)");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .map_err(|e| format!("running cargo: {e}"))?;
    if !status.success() {
        return Err("cargo build failed".into());
    }

    let lib = lib_filename(&name);
    let built = root.join("target/release").join(&lib);
    if !built.exists() {
        return Err(format!(
            "built library not found at {} (is the crate a cdylib?)",
            built.display()
        ));
    }

    let artifacts = root.join("artifacts");
    std::fs::create_dir_all(&artifacts).map_err(|e| e.to_string())?;
    let out = artifacts.join(format!("{name}.yog"));
    package(&out, &name, &version, &built)?;

    eprintln!("==> packaged {}", out.display());
    Ok(())
}

/// Platform tag matching the runtime's, e.g. `linux-x86_64`.
fn platform_tag() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// cdylib file name for `name` on this platform (hyphens become underscores).
fn lib_filename(name: &str) -> String {
    let stem = name.replace('-', "_");
    match std::env::consts::OS {
        "windows" => format!("{stem}.dll"),
        "macos" => format!("lib{stem}.dylib"),
        _ => format!("lib{stem}.so"),
    }
}

/// Minimal `[package]` name/version reader (avoids a TOML dependency).
fn read_package(cargo_toml: &Path) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(cargo_toml)
        .map_err(|e| format!("reading {}: {e}", cargo_toml.display()))?;
    let mut in_package = false;
    let (mut name, mut version) = (None, None);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(v) = field(line, "name") {
            name = Some(v);
        } else if let Some(v) = field(line, "version") {
            version = Some(v);
        }
    }
    Ok((
        name.ok_or("no package name in Cargo.toml")?,
        version.unwrap_or_else(|| "0.0.0".into()),
    ))
}

/// Parse `key = "value"` returning the unquoted value.
fn field(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    Some(rest.trim_matches('"').to_string())
}

/// Write the `.yog` archive: the native under `natives/<platform>/` plus a
/// manifest.
fn package(out: &Path, name: &str, version: &str, native: &Path) -> Result<(), String> {
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let lib_name = native
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad native filename")?;
    let entry = format!("natives/{}/{}", platform_tag(), lib_name);
    let bytes = std::fs::read(native).map_err(|e| e.to_string())?;
    zip.start_file(entry, opts).map_err(|e| e.to_string())?;
    zip.write_all(&bytes).map_err(|e| e.to_string())?;

    let manifest = format!(
        "name = \"{name}\"\nversion = \"{version}\"\nabi = 1\nplatform = \"{}\"\n",
        platform_tag()
    );
    zip.start_file("yog.toml", opts).map_err(|e| e.to_string())?;
    zip.write_all(manifest.as_bytes()).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}
