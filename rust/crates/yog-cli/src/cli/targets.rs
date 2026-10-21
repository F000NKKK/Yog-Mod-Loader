//! The platforms a mod is built for, the toolchain that builds it, and cargo
//! run for each.

use std::path::{Path, PathBuf};
use std::process::Command;

use yog_cli::manifest::to_snake_name;

/// A platform Yog can target.
pub(super) struct Target {
    pub(super) triple: &'static str,
    pub(super) tag: &'static str,
    pub(super) os: &'static str,
}

pub(super) const TARGETS: &[Target] = &[
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

pub(super) enum Builder {
    Zig,
    Cargo,
}

impl Builder {
    pub(super) fn detect() -> Self {
        if has_zigbuild() {
            Builder::Zig
        } else {
            Builder::Cargo
        }
    }
    pub(super) fn subcmd(&self) -> &'static str {
        match self {
            Builder::Zig => "zigbuild",
            Builder::Cargo => "build",
        }
    }

    pub(super) fn build(
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
pub(super) fn is_benign_warning(t: &str) -> bool {
    t.starts_with("warning:")
        && ((t.contains("xcrun") && t.contains("failed"))
            || (t.contains("linker stderr")
                && t.contains("deprecated linker optimization setting")))
}

/// Drop each [`is_benign_warning`] block, along with the matching cargo
/// "generated 1 warning" line.
pub(super) fn filter_benign_warnings(stderr: &str) -> String {
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
        // (`generated 1 warning (1 duplicate)` when it came twice.)
        let only_one =
            t.ends_with("generated 1 warning") || t.ends_with("generated 1 warning (1 duplicate)");
        if suppressed > 0 && t.starts_with("warning:") && only_one {
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
pub(super) fn selected_targets() -> Option<Vec<String>> {
    let raw = std::env::var(yog_ids::env::TARGETS).ok()?;
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
pub(super) const RUST_TOOLCHAIN: &str = env!("CARGO_PKG_RUST_VERSION");

/// `.yog-build/rust-toolchain.toml` — pins the toolchain and lists every
/// target so rustup fetches them on its own. Generated, never hand-written.
pub(super) fn generate_rust_toolchain() -> String {
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

/// Installs the mod toolchain plus `targets` that aren't there, so a build
/// never silently drops a platform for lack of setup. Nothing is updated
/// (`rustup toolchain install` would update a channel like `1.99` to its
/// newest patch, every build); and one `yog` installs at a time — builds
/// running side by side would break each other's downloads.
pub(super) fn install_targets(targets: &[&Target]) {
    if targets.is_empty() {
        return;
    }
    let lock = std::env::temp_dir().join("yog-rustup.lock");
    let _lock = std::fs::File::create(&lock)
        .ok()
        .filter(|f| f.lock().is_ok());
    let rustup = |args: &[&str]| {
        Command::new("rustup")
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    let ok = match toolchain_sysroot() {
        // The toolchain is there: add the targets it lacks (their files, not
        // just rustup's word: an interrupted install leaves them listed).
        Some(sysroot) => {
            let missing: Vec<&str> = targets
                .iter()
                .map(|t| t.triple)
                .filter(|t| !has_std(&sysroot, t))
                .collect();
            missing.is_empty() || {
                // Listed but broken: rustup only re-adds what it doesn't list.
                rustup(
                    &[
                        &["target", "remove", "--toolchain", RUST_TOOLCHAIN][..],
                        &missing,
                    ]
                    .concat(),
                );
                rustup(
                    &[
                        &["target", "add", "--toolchain", RUST_TOOLCHAIN][..],
                        &missing,
                    ]
                    .concat(),
                )
            }
        }
        None => {
            let list: Vec<&str> = targets.iter().map(|t| t.triple).collect();
            rustup(&[
                "toolchain",
                "install",
                RUST_TOOLCHAIN,
                "--profile",
                "minimal",
                "--no-self-update",
                "--target",
                &list.join(","),
            ])
        }
    };
    if !ok {
        eprintln!("    warning: could not install Rust {RUST_TOOLCHAIN} targets via rustup");
    }
}

/// The mod toolchain's sysroot, if it's installed.
pub(super) fn toolchain_sysroot() -> Option<PathBuf> {
    let out = Command::new("rustc")
        .args([&format!("+{RUST_TOOLCHAIN}"), "--print", "sysroot"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

/// Whether `sysroot` has the standard library for `triple`.
pub(super) fn has_std(sysroot: &Path, triple: &str) -> bool {
    std::fs::read_dir(sysroot.join("lib/rustlib").join(triple).join("lib")).is_ok_and(|d| {
        d.flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("libstd-"))
    })
}

pub(super) fn installed_targets() -> Vec<String> {
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

pub(super) fn lib_filename(name: &str, os: &str) -> String {
    // Must match the `[lib] name` pinned in the generated wrapper Cargo.toml
    // (see to_snake_name) — that's what cargo actually names the artifact.
    let stem = to_snake_name(name);
    match os {
        "windows" => format!("{stem}.dll"),
        "macos" => format!("lib{stem}.dylib"),
        _ => format!("lib{stem}.so"),
    }
}

/// Whether `cargo zigbuild` is installed.
pub(super) fn has_zigbuild() -> bool {
    Command::new("cargo")
        .args(["zigbuild", "--help"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The first line `program args` prints, if it runs and succeeds.
pub(super) fn first_line_of(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

#[cfg(test)]
mod tests;
