//! `yog add` / `yog remove`: a dependency in `yog.toml`.

use std::path::PathBuf;

use yog_cli::manifest::{DEPS_SECTION, EXPORT_DEPS_SECTION, MANIFEST, project_root};

/// `yog add`/`yog remove`'s operand, the `yog.toml` it edits and the
/// section: `[dependencies]` (plain crates.io crates) or
/// `[dependencies.exports]` (other Yog mods, mapped to their generated
/// exports crate) — matched by exact header, never as a substring.
pub(super) fn dep_edit<'a>(
    verb: &str,
    crate_name: Option<&'a str>,
    is_mod: bool,
) -> Result<(&'a str, PathBuf, &'static str), String> {
    let name = crate_name.ok_or_else(|| {
        if is_mod {
            format!("usage: yog {verb} --mod <yog-mod-id>")
        } else {
            format!("usage: yog {verb} <crate>")
        }
    })?;
    let section = if is_mod {
        EXPORT_DEPS_SECTION
    } else {
        DEPS_SECTION
    };
    Ok((name, project_root()?.join(MANIFEST), section))
}

pub(super) fn add_dep(crate_name: Option<&str>, is_mod: bool) -> Result<(), String> {
    let (name, yog_toml_path, target_section) = dep_edit("add", crate_name, is_mod)?;

    let text = std::fs::read_to_string(&yog_toml_path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

    let has_section = lines.iter().any(|l| l.trim() == target_section);

    if !has_section {
        // Add the section before any [dev] section or at the end
        let insert_idx = lines
            .iter()
            .position(|l| l.trim().starts_with('[') && l.trim() != "[mod]")
            .unwrap_or(lines.len());
        lines.insert(insert_idx, target_section.to_string());
        lines.insert(insert_idx + 1, format!("{} = \"*\"", name));
    } else {
        // Find the section and add the crate at its end
        let mut in_section = false;
        let mut inserted = false;
        for i in 0..lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with('[') {
                if in_section && trimmed != target_section {
                    lines.insert(i, format!("{} = \"*\"", name));
                    inserted = true;
                    break;
                }
                in_section = trimmed == target_section;
            }
        }
        if !inserted {
            // The section was at the end of the file
            lines.push(format!("{} = \"*\"", name));
        }
    }

    let new_text = lines.join("\n") + "\n";
    std::fs::write(&yog_toml_path, new_text).map_err(|e| e.to_string())?;
    eprintln!(
        "==> added {} to yog.toml [{}]",
        name,
        &target_section[1..target_section.len() - 1]
    );
    Ok(())
}

pub(super) fn remove_dep(crate_name: Option<&str>, is_mod: bool) -> Result<(), String> {
    let (name, yog_toml_path, target_section) = dep_edit("remove", crate_name, is_mod)?;

    let text = std::fs::read_to_string(&yog_toml_path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = Vec::new();
    let mut removed = false;
    let mut in_section = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == target_section;
        }
        // Remove lines like: crate-name = "..." or crate-name = { ... },
        // only within the targeted section and only `name` itself (not
        // every crate whose name it prefixes).
        if in_section
            && trimmed
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == name)
        {
            removed = true;
            continue;
        }
        lines.push(line.to_string());
    }

    if !removed {
        return Err(format!(
            "dependency {} not found in yog.toml {}",
            name, target_section
        ));
    }

    let new_text = lines.join("\n") + "\n";
    std::fs::write(&yog_toml_path, new_text).map_err(|e| e.to_string())?;
    eprintln!("==> removed {} from yog.toml {}", name, target_section);
    Ok(())
}
