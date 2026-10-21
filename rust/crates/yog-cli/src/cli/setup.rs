//! `yog setup` and `yog doctor`: the build's dependencies checked, and
//! (setup) installed.

use std::io::Write as _;
use std::process::Command;

use super::targets::{
    RUST_TOOLCHAIN, TARGETS, Target, first_line_of, has_zigbuild, install_targets,
    installed_targets,
};

pub(super) fn setup() -> Result<(), String> {
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

pub(super) fn check_rust() -> bool {
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

pub(super) fn check_zigbuild() -> bool {
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

pub(super) fn check_zig() -> bool {
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

pub(super) fn try_install_zig() -> bool {
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

pub(super) fn check_targets() {
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

/// What `yog doctor` found — the same checks as `yog setup`, but read-only
/// and non-interactive, so IDEs (Yog-IDLE) can show it and install what's
/// missing themselves. `--json` is the machine-readable form; its keys are
/// a stable contract.
pub(super) struct DoctorReport {
    pub(super) cli_version: &'static str,
    pub(super) rust_toolchain: &'static str,
    pub(super) cargo: Option<String>,
    pub(super) rustup: Option<String>,
    pub(super) cargo_zigbuild: bool,
    pub(super) zig: Option<String>,
    /// `(tag, triple, installed)` for every platform a `.yog` bundles.
    pub(super) targets: Vec<(&'static str, &'static str, bool)>,
}

pub(super) fn doctor() -> DoctorReport {
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
pub(super) fn json_str(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

pub(super) fn json_opt(s: &Option<String>) -> String {
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

/// `yog doctor [--json]`: the report, for people or (`json`) for tools.
pub(super) fn doctor_cmd(json: bool) {
    let report = doctor();
    if json {
        println!("{}", report.to_json());
    } else {
        report.print();
    }
}

#[cfg(test)]
mod tests;
