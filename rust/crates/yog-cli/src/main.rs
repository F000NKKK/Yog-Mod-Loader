//! `yog` — the Yog mod build tool.
//!
//! `yog build` (run inside a mod crate) compiles the mod for every supported
//! platform it can and packs the results into `artifacts/<name>.yog` — a zip
//! laying natives out per platform (`natives/<os>-<arch>/`) plus a `yog.toml`
//! manifest that lists the bundled platforms. A `.yog` distinguishes Yog mods
//! and lets the runtime pick the right native at load time; players just drop it
//! in their mods folder.
//!
//! Cross-compilation uses `cargo-zigbuild` when available (zig as the linker,
//! covering linux/windows/macos from any host); otherwise it falls back to
//! `cargo build` and only the targets with a working toolchain are bundled.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A platform Yog can target.
struct Target {
    /// Rust target triple.
    triple: &'static str,
    /// Platform tag used in archives, e.g. `linux-x86_64`.
    tag: &'static str,
    /// `linux` | `windows` | `macos` — selects the native file naming.
    os: &'static str,
}

const TARGETS: &[Target] = &[
    Target { triple: "x86_64-unknown-linux-gnu", tag: "linux-x86_64", os: "linux" },
    Target { triple: "aarch64-unknown-linux-gnu", tag: "linux-aarch64", os: "linux" },
    Target { triple: "x86_64-pc-windows-gnu", tag: "windows-x86_64", os: "windows" },
    Target { triple: "x86_64-apple-darwin", tag: "macos-x86_64", os: "macos" },
    Target { triple: "aarch64-apple-darwin", tag: "macos-aarch64", os: "macos" },
];

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
         \x20 build    Cross-compile the current mod crate (release) for all\n\
         \x20          supported platforms and package it as artifacts/<name>.yog\n\n\
         Cross-compiling all platforms needs cargo-zigbuild + rustup targets;\n\
         without them only the toolchains you have are bundled."
    );
}

fn build() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let (name, version) = read_package(&root.join("Cargo.toml"))?;

    let builder = Builder::detect();
    eprintln!("==> building {name} {version} (release) with `cargo {}`", builder.subcmd());

    let installed = installed_targets();
    let mut bundled: Vec<(String, PathBuf)> = Vec::new();

    for t in TARGETS {
        if !installed.iter().any(|s| s == t.triple) {
            eprintln!("    skip {} (rustup target {} not installed)", t.tag, t.triple);
            continue;
        }
        match builder.build(&root, t.triple) {
            Ok(()) => {
                let lib = lib_filename(&name, t.os);
                let path = root.join("target").join(t.triple).join("release").join(&lib);
                if path.exists() {
                    eprintln!("    built {} ({})", t.tag, t.triple);
                    bundled.push((t.tag.to_string(), path));
                } else {
                    eprintln!("    skip {} (no output {})", t.tag, lib);
                }
            }
            Err(_) => eprintln!("    skip {} (build failed)", t.tag),
        }
    }

    if bundled.is_empty() {
        return Err("no platform built — install cargo-zigbuild and rustup targets".into());
    }

    let artifacts = root.join("artifacts");
    std::fs::create_dir_all(&artifacts).map_err(|e| e.to_string())?;
    let out = artifacts.join(format!("{name}.yog"));
    package(&out, &name, &version, &bundled)?;

    let tags: Vec<&str> = bundled.iter().map(|(t, _)| t.as_str()).collect();
    eprintln!("==> packaged {} [{}]", out.display(), tags.join(", "));
    Ok(())
}

/// Which cargo subcommand cross-compiles for us.
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

    fn build(&self, root: &Path, triple: &str) -> Result<(), ()> {
        let status = Command::new("cargo")
            .current_dir(root)
            .args([self.subcmd(), "--release", "--target", triple])
            .status();
        match status {
            Ok(s) if s.success() => Ok(()),
            _ => Err(()),
        }
    }
}

/// Targets reported by `rustup target list --installed`.
fn installed_targets() -> Vec<String> {
    Command::new("rustup")
        .args(["target", "list", "--installed"])
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

/// cdylib file name for `name` on `os` (hyphens become underscores).
fn lib_filename(name: &str, os: &str) -> String {
    let stem = name.replace('-', "_");
    match os {
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

/// Write the `.yog` archive: each platform's native under `natives/<tag>/` plus
/// a manifest listing the bundled platforms.
fn package(out: &Path, name: &str, version: &str, bundled: &[(String, PathBuf)]) -> Result<(), String> {
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for (tag, native) in bundled {
        let lib_name = native
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("bad native filename")?;
        let entry = format!("natives/{tag}/{lib_name}");
        let bytes = std::fs::read(native).map_err(|e| e.to_string())?;
        zip.start_file(entry, opts).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    let platforms = bundled
        .iter()
        .map(|(t, _)| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = format!(
        "name = \"{name}\"\nversion = \"{version}\"\nabi = 1\nplatforms = [{platforms}]\n"
    );
    zip.start_file("yog.toml", opts).map_err(|e| e.to_string())?;
    zip.write_all(manifest.as_bytes()).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}
