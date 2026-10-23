//! The platforms a mod is built for, the toolchain that builds it, and cargo
//! run for each.

use std::path::{Path, PathBuf};
use std::process::Command;

use yog_codegen::{Header, Regenerate};
use yog_ids::platform::{self, Platform};

use yog_cli::project::toml::Toml;

pub(crate) enum Builder {
    Zig,
    Cargo,
}

impl Builder {
    pub(crate) fn detect() -> Self {
        if has_zigbuild() {
            Builder::Zig
        } else {
            Builder::Cargo
        }
    }
    pub(crate) fn subcmd(&self) -> &'static str {
        match self {
            Builder::Zig => "zigbuild",
            Builder::Cargo => "build",
        }
    }

    pub(crate) fn build(
        &self,
        build_dir: &Path,
        triple: &str,
        root: &Path,
        symbols: super::symbols::Symbols,
    ) -> Result<(), ()> {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(build_dir)
            .env("CARGO_TARGET_DIR", root.join(yog_ids::project::TARGET))
            .args([self.subcmd(), "--release", "--target", triple]);
        cmd.envs(symbols.profile().iter().copied());
        match super::cargo_out::run(&mut cmd, build_dir, root) {
            Ok(true) => Ok(()),
            _ => Err(()),
        }
    }
}

/// `YOG_TARGETS` — comma-separated platform tags or triples
/// (e.g. `linux-x86_64`) to restrict `yog build` to. Unset or empty means
/// every installed target. A dev loop only needs the host platform; release
/// builds should leave it unset so the `.yog` stays multi-platform.
pub(crate) fn selected_targets() -> Option<Vec<String>> {
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
pub(crate) const RUST_TOOLCHAIN: &str = env!("CARGO_PKG_RUST_VERSION");

/// `obj/rust-toolchain.toml` — pins the toolchain and lists every
/// target so rustup fetches them on its own. Generated, never hand-written.
pub(crate) fn generate_rust_toolchain() -> String {
    let targets: Vec<String> = platform::ALL
        .iter()
        .map(|t| format!("    \"{}\",\n", t.triple))
        .collect();
    format!(
        "{}[toolchain]\n\
         channel = \"{RUST_TOOLCHAIN}\"\n\
         profile = \"minimal\"\n\
         targets = [\n{}]\n",
        Header {
            tool: "`yog build`",
            source: "yog.toml",
            regenerate: Regenerate::EveryBuild,
        }
        .comment::<Toml>(),
        targets.concat()
    )
}

/// Installs the mod toolchain plus `targets` that aren't there, so a build
/// never silently drops a platform for lack of setup. Nothing is updated
/// (`rustup toolchain install` would update a channel like `1.99` to its
/// newest patch, every build); and one `yog` installs at a time — builds
/// running side by side would break each other's downloads.
pub(crate) fn install_targets(targets: &[&Platform]) {
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
pub(crate) fn toolchain_sysroot() -> Option<PathBuf> {
    let out = Command::new("rustc")
        .args([&format!("+{RUST_TOOLCHAIN}"), "--print", "sysroot"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

/// Whether `sysroot` has the standard library for `triple`.
pub(crate) fn has_std(sysroot: &Path, triple: &str) -> bool {
    std::fs::read_dir(sysroot.join("lib/rustlib").join(triple).join("lib")).is_ok_and(|d| {
        d.flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("libstd-"))
    })
}

pub(crate) fn installed_targets() -> Vec<String> {
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

pub(crate) fn lib_filename(name: &str, os: &str) -> String {
    // Must match the `[lib] name` pinned in the generated wrapper Cargo.toml
    // (see yog_ids::mod_lib_name) — that's what cargo actually names the artifact.
    let stem = yog_ids::mod_lib_name(name);
    match os {
        "windows" => format!("{stem}.dll"),
        "macos" => format!("lib{stem}.dylib"),
        _ => format!("lib{stem}.so"),
    }
}

/// Whether `cargo zigbuild` is installed.
pub(crate) fn has_zigbuild() -> bool {
    Command::new("cargo")
        .args(["zigbuild", "--help"])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The first line `program args` prints, if it runs and succeeds.
pub(crate) fn first_line_of(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next().unwrap_or("").trim().to_string())
}
