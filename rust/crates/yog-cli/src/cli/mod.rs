//! The `yog` command line: each command in its own module, picked here by
//! its first word.

mod build;
mod cargo_out;
mod check;
mod control;
mod data;
mod deps;
mod mappings;
mod new;
mod pictures;
mod prepare;
mod publish;
mod run;
mod setup;
mod symbols;
mod targets;

/// What `yog help` prints.
const USAGE: &str = include_str!("usage.txt");

/// The flags commands take.
const DEV: &str = "--dev";
const DEBUG_FLAG: &str = "--debug";
const LIST: &str = "--list";
const MOD: &str = "--mod";
const DRY_RUN: &str = "--dry-run";
const HELP: [&str; 2] = ["-h", "--help"];
/// What a flag starts with.
const FLAG: &str = "--";

/// `yog schema <format>`: the format's shape (`yog_data::Shape::describe`)
/// as JSON — what editors and docs are generated from.
fn schema(format: Option<&str>) -> Result<(), String> {
    let all = || yog_formats::names().collect::<Vec<_>>().join(", ");
    let name = format.ok_or_else(|| format!("usage: yog schema <format> — {}", all()))?;
    let shape =
        yog_formats::shape_of(name).ok_or_else(|| format!("no format `{name}` — {}", all()))?;
    println!("{}", yog_data::text::to_string_pretty(&shape.describe()));
    Ok(())
}

/// Runs the command `args` (the program's own name first) names; an error
/// ends the program.
pub(crate) fn main(args: &[String]) {
    let rest = args.get(2..).unwrap_or_default();
    let has = |flag: &str| rest.iter().any(|a| a == flag);
    let result = match args.get(1).map(String::as_str) {
        // `yog build debug`: the same `--release` build with DWARF debug
        // info kept for `yog-symbols`/`yog-debugger`; `yog build release`:
        // optimized, the debug info split out beside it; `--dev`: for a game
        // that hot-reloads it (Yog-IDLE, dev.sh) — the `.yog` carries what
        // hot reload checks.
        Some("build") => {
            let symbols = symbols::Symbols::of(rest.first().map(String::as_str));
            build::build(symbols, has(DEV)).map(|_| ())
        }
        Some("check") => build::check(),
        Some("schema") => schema(rest.first().map(String::as_str)),
        Some("bench") => build::run_benches(rest),
        Some("serve") => yog_cli::serve::run(),
        Some("designer-lib") => build::designer_lib(),
        Some(yog_cli::design::host::COMMAND) => {
            yog_cli::design::host::serve();
            Ok(())
        }
        Some("new") => new::new_mod(rest.first().map(String::as_str)),
        Some("setup") => setup::setup(),
        Some("doctor") => {
            setup::doctor_cmd();
            Ok(())
        }
        Some("-V" | "--version" | "version") => {
            println!("yog {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("add") => deps::add_dep(dep_name(rest), has(MOD)),
        Some("remove") => deps::remove_dep(dep_name(rest), has(MOD)),
        Some("run") => run_cmd(rest),
        Some("control") => control::control(rest),
        Some(data::COMMAND) => data::data(rest),
        Some(mappings::COMMAND) => mappings::mappings(rest),
        Some("publish") => publish_cmd(rest),
        Some("res") => sub(rest, "check", check::res_check, "usage: yog res check"),
        Some("content") => sub(
            rest,
            "check",
            check::content_check,
            "usage: yog content check",
        ),
        Some("import") => import(rest),
        Some("ui") => match rest.first().map(String::as_str) {
            Some("gen") => check::ui_gen(rest.get(1..).unwrap_or_default()),
            _ => Err("usage: yog ui gen [file.yui ...]  (no files: every src/**/*.yui)".into()),
        },
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}")),
    };
    if let Err(e) = result {
        // Said already as what it is, or said here as a failure.
        if yog_cli::diagnostics::errors_reported() == 0 {
            yog_cli::diagnostics::failed(yog_ids::build_problem::FAILED, &e);
        }
        eprintln!("yog: error: {e}");
        std::process::exit(1);
    }
}

/// `yog import ...`: what it wrote, what to warn of.
fn import(rest: &[String]) -> Result<(), String> {
    let imported = yog_cli::content::import_command(rest)?;
    for w in &imported.warnings {
        eprintln!("warning: {w}");
    }
    for f in &imported.written {
        eprintln!("==> wrote {f}");
    }
    eprintln!(
        "    `yog content check` validates them; register with `yog_api::content!();` in src/lib.rs"
    );
    Ok(())
}

/// `yog <cmd> <word>`: `run` when the word is `word`, else `usage`.
fn sub(
    rest: &[String],
    word: &str,
    run: fn() -> Result<(), String>,
    usage: &str,
) -> Result<(), String> {
    match rest.first() {
        Some(w) if w == word => run(),
        _ => Err(usage.into()),
    }
}

/// `yog add`/`remove`'s crate: the first word that isn't `--mod`.
fn dep_name(rest: &[String]) -> Option<&str> {
    rest.iter().map(String::as_str).find(|a| *a != MOD)
}

/// `yog run <target> | <loader> <mc> [--debug] [--dev]`, `yog run --list [--json]`.
fn run_cmd(rest: &[String]) -> Result<(), String> {
    let has = |flag: &str| rest.iter().any(|a| a == flag);
    if has(LIST) {
        return run::list_run_targets();
    }
    let symbols = if has(DEBUG_FLAG) {
        symbols::Symbols::Kept
    } else {
        symbols::Symbols::Stripped
    };
    let dev = has(DEV);
    let words: Vec<&str> = rest
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with(FLAG))
        .collect();
    match words.as_slice() {
        // `yog run fabric 26.2` is `yog run fabric-26.2`.
        [loader, mc] => run::run_target(&[*loader, mc].join(yog_ids::sep::HYPHEN), symbols, dev),
        [id] => run::run_target(id, symbols, dev),
        _ => Err(
            "usage: yog run <target> | <loader> <mc> [--debug] [--dev]; yog run --list [--json]"
                .into(),
        ),
    }
}

/// `yog publish exports [--dry-run]`.
fn publish_cmd(rest: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: yog publish exports [--dry-run]";
    // Publishing can't be undone: anything but `--dry-run` (a typo, `--help`) stops here.
    let flags: Vec<&str> = rest.iter().skip(1).map(String::as_str).collect();
    let other = flags.iter().find(|a| **a != DRY_RUN);
    match (rest.first().map(String::as_str), other) {
        (Some("exports"), Some(o)) if HELP.contains(o) => {
            println!(
                "{USAGE}\n\nPublishes this mod's exports crate (`#[yog_export]` items) to crates.io; --dry-run only generates and packages it."
            );
            Ok(())
        }
        (Some("exports"), Some(o)) => Err(format!("unknown flag {o} — {USAGE}")),
        (Some("exports"), None) => publish::publish_exports(!flags.is_empty()),
        (Some(h), _) if HELP.contains(&h) => {
            println!("{USAGE}");
            Ok(())
        }
        (Some(other), _) => Err(format!("unknown publish subcommand: {other}")),
        (None, _) => Err(USAGE.into()),
    }
}
