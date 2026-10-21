//! `yog import` — turns models made elsewhere into `content/` files:
//!
//! - `yog import resourcepack <zip|dir> [--model <id>] [--all]` — a
//!   resource pack's (or a mod's `assets/`) block and item models: parents
//!   resolved (a vanilla template stays a template, other parents are
//!   flattened from the pack or the Minecraft client jar), textures copied
//!   into `resources/`, the blockstate's rotations kept;
//! - `yog import bbmodel <file.bbmodel> [--id <name>] [--item]` — a
//!   Blockbench project in the Java block/item format, textures included;
//! - `yog import font <yog:font/name> [--id <path>] [--chars <ranges>]` — a copy of
//!   one of the game's fonts as the mod's own `.yfont` ([`font`]).

mod bbmodel;
mod blocks;
mod build;
mod fields;
mod font;
mod model;
mod pack;
mod target;

pub(crate) use font::copy_path as font_copy_path;

use target::Target;

const USAGE: &str = "usage:\n  yog import resourcepack <pack.zip|dir> [--model <ns:block/name|ns:item/name>] [--all] [--force]\n  yog import bbmodel <file.bbmodel> [--id <name>] [--item] [--force]\n  yog import font <yog:font/name> [--id <path>] [--chars <hex ranges>] [--force]";

/// The flags `yog import` takes: those with a value after them, and the rest.
const MODEL: &str = "--model";
const ID: &str = "--id";
const CHARS: &str = "--chars";
const WITH_VALUE: [&str; 3] = [MODEL, ID, CHARS];
const FORCE: &str = "--force";
const ALL: &str = "--all";
const ITEM: &str = "--item";
/// What a flag starts with.
const FLAG: &str = "--";
/// What the subcommands import.
const RESOURCEPACK: &str = "resourcepack";
const BBMODEL: &str = "bbmodel";

/// The words of `args` after the subcommand that aren't flags or a
/// flag's value.
fn positional(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args.iter().skip(1) {
        if skip {
            skip = false;
        } else if WITH_VALUE.contains(&a.as_str()) {
            skip = true;
        } else if !a.starts_with(FLAG) {
            out.push(a.as_str());
        }
    }
    out
}

/// What an import wrote, and what to warn of.
pub struct Imported {
    pub written: Vec<String>,
    pub warnings: Vec<String>,
}

/// `yog import ...` (args after `import`); an error when nothing was written.
pub fn import_command(args: &[String]) -> Result<Imported, String> {
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    let positional = positional(args);
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let mod_id = read_mod_id(&root)?;
    let mut target = Target {
        root: root.clone(),
        ns: super::valid_namespace(&mod_id).then_some(mod_id),
        force: flag(FORCE),
        written: Vec::new(),
        warnings: Vec::new(),
    };
    match (args.first().map(String::as_str), positional.first()) {
        (Some(RESOURCEPACK), Some(src)) => {
            let pack = pack::Pack::open(&root.join(src))?;
            blocks::import_pack(&mut target, &pack, value(MODEL), flag(ALL))?;
        }
        (Some(BBMODEL), Some(file)) => {
            bbmodel::import_bbmodel(&mut target, &root.join(file), value(ID), flag(ITEM))?
        }
        (Some(kind), Some(alias)) if kind == yog_ids::kind::FONT => {
            font::import_font(&mut target, alias, value(ID), value(CHARS))?
        }
        _ => return Err(USAGE.into()),
    }
    if target.written.is_empty() {
        return Err(format!(
            "nothing imported{}",
            target
                .warnings
                .iter()
                .map(|w| format!("\n  {w}"))
                .collect::<String>()
        ));
    }
    Ok(Imported {
        written: target.written,
        warnings: target.warnings,
    })
}

/// The mod id `yog.toml` in `root` gives.
fn read_mod_id(root: &std::path::Path) -> Result<String, String> {
    let path = root.join(yog_ids::file::MANIFEST);
    if !path.is_file() {
        return Err("no yog.toml in the current directory — run `yog import` in a mod".into());
    }
    crate::manifest::YogToml::read(&path).map(|m| m.id)
}

#[cfg(test)]
mod tests;
