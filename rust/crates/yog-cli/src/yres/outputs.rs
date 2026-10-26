//! What a mod's resources become besides code: lang files and
//! `sounds.json` (merged with hand-written ones), and the generated
//! sources written out.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use super::{Compiled, GENERATED_DIR, GENERATED_FILE, parse};
use crate::diagnostics::Diagnostic;
use crate::yres::problem;
use yog_codegen::{Header, Regenerate};
use yog_codegen_rust::Rust;

/// The lang JSON of `entries` (key, text), order kept.
fn lang_json(entries: &[(String, String)]) -> String {
    let mut out = String::from("{");
    for (n, (k, v)) in entries.iter().enumerate() {
        out.push_str(if n == 0 { "\n  " } else { ",\n  " });
        out.push_str(&serde_json::to_string(k).unwrap_or_default());
        out.push_str(": ");
        out.push_str(&serde_json::to_string(v).unwrap_or_default());
    }
    out.push_str(if entries.is_empty() { "}\n" } else { "\n}\n" });
    out
}

/// Locale `locale`'s lang file of the mod: its `entries` after a
/// hand-written file's, whose keys mustn't be the `.yres` texts' (`origin`:
/// where each of those is).
pub(super) fn lang_file(
    root: &Path,
    mod_id: &str,
    locale: &str,
    entries: Vec<(String, String)>,
    origin: &HashMap<String, String>,
) -> Result<(String, Vec<u8>), Vec<Diagnostic>> {
    let path = yog_ids::asset_file(mod_id, &yog_ids::lang_file(locale));
    let Ok(text) = std::fs::read_to_string(root.join(&path)) else {
        return Ok((path, lang_json(&entries).into_bytes()));
    };
    let hand = parse(&text).map_err(|found| {
        found
            .0
            .iter()
            .map(|l| l.clone().in_file(&path))
            .collect::<Vec<_>>()
    })?;
    let twice: Vec<Diagnostic> = hand
        .iter()
        .filter_map(|h| {
            origin.get(&h.name).map(|at| {
                problem(format!("{:?} is also defined at {at}", h.name))
                    .in_file(&path)
                    .at_line(h.line)
            })
        })
        .collect();
    if !twice.is_empty() {
        return Err(twice);
    }
    let mut merged: Vec<(String, String)> = hand
        .iter()
        .map(|h| (h.name.clone(), h.text().to_string()))
        .collect();
    merged.extend(entries);
    Ok((path, lang_json(&merged).into_bytes()))
}

/// The generated sources' header, `source` what each is generated from.
fn banner(source: &str) -> String {
    Header {
        tool: "`yog build`",
        source,
        regenerate: Regenerate::EveryBuild,
    }
    .comment::<Rust>()
        + "\n"
}
/// What a generated source's file name ends with.
const SOURCE_EXT: &str = ".g.rs";

/// Writes `obj/generated/resources/<Set>.g.rs` per set and
/// `obj/generated/resources.g.rs` including them all (what
/// `yog_api::resources!()` pulls in as `crate::res`); deletes those of sets
/// that are gone.
pub fn write_sources(root: &Path, compiled: &Compiled) -> Result<(), String> {
    let dir = root.join(GENERATED_DIR);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name
                .strip_suffix(SOURCE_EXT)
                .is_some_and(|set| !compiled.sources.contains_key(set))
            {
                crate::files::remove(&entry.path());
            }
        }
    }
    let mut index = banner("resources/**/*.yres");
    index.push_str("// Included by `yog_api::resources!()` as `crate::res`.\n");
    for (set, code) in &compiled.sources {
        let source = code
            .lines()
            .next()
            .and_then(|l| l.split('`').nth(1))
            .unwrap_or("resources/");
        let mut file = banner(source);
        file.push_str(code);
        crate::write_if_changed(&dir.join(format!("{set}{SOURCE_EXT}")), &file)?;
        let _ = writeln!(
            index,
            "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/resources/{set}{SOURCE_EXT}\"));"
        );
    }
    crate::write_if_changed(&root.join(GENERATED_FILE), &index)?;
    // The same sets for the mod's Yog Script.
    let script = root.join(super::GENERATED_SCRIPT);
    match super::script::source(&compiled.scripts) {
        Some(code) => crate::write_if_changed(&script, &code),
        None => {
            crate::files::remove(&script);
            Ok(())
        }
    }
}
