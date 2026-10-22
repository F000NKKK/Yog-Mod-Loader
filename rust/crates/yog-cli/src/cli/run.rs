//! `yog run`: the mod built, exported into a target's game, the game launched.

use super::symbols::Symbols;
use std::path::{Path, PathBuf};
use std::process::Command;

use yog_cli::manifest::{MANIFEST, project_root};

use super::build::{ARTIFACTS_DIR, build};
use super::prepare::write_file;

/// `yog run --list`: every run target, a line each. `--json`, for editors:
/// `{"targets": [...], "sources": [folders they're read from]}`.
pub(super) fn list_run_targets() -> Result<(), String> {
    let home = yog_home::Home::locate()?;
    let resolver = yog_home::targets::Resolver::new(&home)?;
    for t in &resolver.targets()? {
        println!("{:<20} {}", t.id, t.label);
    }
    Ok(())
}

/// Build, export the artifact into the target's game, then launch it —
/// the target from Yog's home (`yog_home::targets`): a loader on a
/// Minecraft version, or one of the machine's own.
pub(super) fn run_target(id: &str, symbols: Symbols, dev: bool) -> Result<(), String> {
    let root = project_root()?;
    let resolver = yog_home::targets::Resolver::new(&yog_home::Home::locate()?)?;
    let target = resolver.target(id)?;
    let plan = resolver.plan(&target);

    // It's played here: this machine's platform is all it needs.
    let (meta, artifact) = build(symbols, dev, super::build::Platforms::Host)?;

    if let Some(dir) = &plan.export_dir {
        std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        let dest = dir.join(format!("{}.yog", meta.id));
        std::fs::copy(&artifact, &dest)
            .map_err(|e| format!("copying to {}: {e}", dest.display()))?;
        write_file(
            &source_marker(dir, &meta.id),
            root.display().to_string().as_bytes(),
        )?;
        eprintln!("==> exported {} -> {}", meta.id, dest.display());
        refresh_exported(dir, &meta.id, symbols);
    }

    let Some(command) = &plan.command else {
        eprintln!("==> {id} has no command — export only, nothing to launch.");
        return Ok(());
    };

    eprintln!(
        "==> launching {}: {command} {}",
        target.label,
        plan.args.join(" ")
    );
    let mut proc = Command::new(command);
    proc.args(&plan.args);
    if let Some(cwd) = &plan.cwd {
        proc.current_dir(cwd);
    }
    for (k, v) in &plan.env {
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
pub(super) fn source_marker(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.source"))
}

/// Rebuilds every other mod `yog run` exported into `dir` (the ones with a
/// `<id>.source`) and re-exports it — mods from an older build would carry
/// an older ABI, and the runtime refuses those. A mod that fails to build
/// keeps its old artifact (with a warning); the run goes on.
pub(super) fn refresh_exported(dir: &Path, current: &str, symbols: Symbols) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut others: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let id = path
                .file_name()?
                .to_str()?
                .strip_suffix(".source")?
                .to_string();
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
        cmd.arg("build")
            .current_dir(&source)
            .env(yog_ids::env::TARGETS, yog_ids::platform::host_tag());
        if let Some(word) = symbols.word() {
            cmd.arg(word);
        }
        let built = cmd.status().is_ok_and(|s| s.success());
        let artifact = source
            .join(ARTIFACTS_DIR)
            .join(yog_ids::Format::Mod.file(&id));
        if !built || !artifact.is_file() {
            eprintln!("warning: {id} didn't build — keeping its previous {id}.yog");
            continue;
        }
        if let Err(e) = std::fs::copy(&artifact, dir.join(format!("{id}.yog"))) {
            eprintln!("warning: {id}: copying {}: {e}", artifact.display());
        }
    }
}
