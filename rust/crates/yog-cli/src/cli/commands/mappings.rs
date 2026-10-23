//! `yog mappings <mc> <runtime-ns> <dir>`: a host's mapping table
//! (`yog_cli::mappings`) written into `dir` for its jar
//! (`yog_ids::mapping_file::NAME`); none when the names are Mojang's.

/// The words that name it.
pub(super) const WORDS: &[&str] = &["mappings"];
const USAGE: &str = "usage: yog mappings <minecraft version> <mojang|intermediary|srg> <dir>";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let [mc, runtime_ns, dir] = rest else {
        return Err(USAGE.into());
    };
    let table = yog_cli::mappings::table(mc, runtime_ns)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    if table.classes.is_empty() {
        return Ok(());
    }
    let out = std::path::Path::new(dir).join(yog_ids::mapping_file::NAME);
    let bytes = yog_data::to_bytes(&table);
    std::fs::write(&out, &bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    let out = out.display();
    eprintln!(
        "==> {out}: {} classes renamed ({} bytes)",
        table.classes.len(),
        bytes.len()
    );
    Ok(())
}
