//! `yog new`: a new mod project.

use std::path::PathBuf;

use yog_cli::project::manifest::{MANIFEST, RUST_LIB, SOURCES_DIR};
use yog_data::Schema;
use yog_formats::manifest::Language;

use crate::cli::pipeline::prepare::write_file;

/// A new mod's `yog.toml` and its first source (`src/main.ys`, or
/// `src/lib.rs` for a Rust one), their `$NAME$`s to fill.
const YOG_TOML: &str = include_str!("new/yog.toml.in");
const LIB_RS: &str = include_str!("new/lib.rs.in");
const MAIN_YS: &str = include_str!("new/main.ys.in");
/// A Yog Script mod's first source's name, without its extension.
const MAIN: &str = "main";
/// What makes `yog new` start a mod in Rust.
pub(crate) const RUST_FLAG: &str = "--rust";

/// `language` as `yog.toml` spells it (`ys`).
fn spelled(language: Language) -> String {
    match language.write() {
        yog_data::Value::Str(s) => s,
        other => yog_data::text::to_string(&other),
    }
}

/// The first source of a mod in `language`: its path under the mod, and
/// its template.
fn first_source(language: Language) -> (String, &'static str) {
    match language {
        Language::Ys => (
            format!("{SOURCES_DIR}/{}", yog_ids::Format::Script.file(MAIN)),
            MAIN_YS,
        ),
        Language::Rust => (format!("{SOURCES_DIR}/{RUST_LIB}"), LIB_RS),
    }
}

pub(crate) fn new_mod(name: Option<&str>, language: Language) -> Result<(), String> {
    let name = name.ok_or("usage: yog new <name> [--rust]")?;
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return Err(format!("invalid mod name: {name:?}"));
    }
    let root = PathBuf::from(name);
    if root.exists() {
        return Err(format!("directory {name} already exists"));
    }

    std::fs::create_dir_all(root.join(SOURCES_DIR)).map_err(|e| e.to_string())?;

    // yog.toml
    let yog_toml = YOG_TOML
        .replace("$NAME$", name)
        .replace("$DISPLAY$", &to_display_name(name))
        .replace("$LANGUAGE$", &spelled(language));
    write_file(&root.join(MANIFEST), yog_toml.as_bytes())?;

    // The first source: src/main.ys, or src/lib.rs.
    let (source, template) = first_source(language);
    let code = template
        .replace("$STRUCT$", &yog_codegen::pascal(name))
        .replace("$NAME$", name);
    write_file(&root.join(&source), code.as_bytes())?;

    // .gitignore
    write_file(
        &root.join(".gitignore"),
        yog_ids::concatcp!(yog_ids::project::OBJ, "/\n", yog_ids::project::BIN, "/\n").as_bytes(),
    )?;

    eprintln!("==> created {name}/");
    eprintln!("    yog.toml       ← edit mod metadata here");
    eprintln!("    {source:<14} ← write your mod here");
    eprintln!();
    eprintln!("Next: cd {name} && yog build");
    Ok(())
}

pub(crate) fn to_display_name(id: &str) -> String {
    id.replace('-', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().to_string())
                .unwrap_or_default()
                + c.as_str()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;
