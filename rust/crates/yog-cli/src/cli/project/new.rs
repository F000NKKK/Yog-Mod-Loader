//! `yog new`: a new mod project.

use std::path::PathBuf;

use yog_cli::manifest::MANIFEST;
use yog_cli::yui;

use crate::cli::build::prepare::write_file;

/// A new mod's `yog.toml` and `src/lib.rs`, their `$NAME$`s to fill.
const YOG_TOML: &str = include_str!("new/yog.toml.in");
const LIB_RS: &str = include_str!("new/lib.rs.in");

pub(crate) fn new_mod(name: Option<&str>) -> Result<(), String> {
    let name = name.ok_or("usage: yog new <name>")?;
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return Err(format!("invalid mod name: {name:?}"));
    }
    let root = PathBuf::from(name);
    if root.exists() {
        return Err(format!("directory {name} already exists"));
    }

    std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;

    // yog.toml
    let yog_toml = YOG_TOML
        .replace("$NAME$", name)
        .replace("$DISPLAY$", &to_display_name(name));
    write_file(&root.join(MANIFEST), yog_toml.as_bytes())?;

    // src/lib.rs
    let lib_rs = LIB_RS
        .replace("$STRUCT$", &yui::pascal(name))
        .replace("$NAME$", name);
    write_file(&root.join("src/lib.rs"), lib_rs.as_bytes())?;

    // .gitignore
    write_file(
        &root.join(".gitignore"),
        b".yog-build/\ntarget/\nartifacts/\n",
    )?;

    eprintln!("==> created {name}/");
    eprintln!("    yog.toml       ← edit mod metadata here");
    eprintln!("    src/lib.rs     ← write your mod here");
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
