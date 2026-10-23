//! `yog import ...`: another loader's content made Yog's; what it wrote,
//! what to warn of.

pub(super) const WORDS: &[&str] = &["import"];

pub(super) fn run(rest: &[String]) -> Result<(), String> {
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
