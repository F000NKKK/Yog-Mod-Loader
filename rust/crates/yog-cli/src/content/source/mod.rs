//! The Rust the build generates for a mod's content (`yog-content.g.rs`).

mod blocks;
mod register;

use yog_ids::code_set;

use crate::content::defs::Defs;
use crate::content::entry::{BlockProps, Entry};
use crate::content::files::{BodyFile, BookFile, EntityFile, FontFile};
use crate::content::outputs::write_tags;
use crate::content::recipe::registration::Recipe;
use crate::content::{Compiled, GENERATED_FILE, table};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use yog_ids::Format;

/// [`yog_registry::REGISTRATIONS_FILE`]: what the mod registers at the
/// game's start, each with a hash of what can't change while it runs — a
/// block's properties and states, not its model, name or tags (the game
/// reloads those).
pub(super) fn registrations(
    entries: &[Entry],
    entities: &[EntityFile],
    dimensions: &[(String, Vec<u8>)],
    defs: &Defs,
) -> String {
    fn hash(text: &str) -> String {
        // FNV-1a: the same on every build of the CLI.
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0100_0000_01b3;
        let h = text
            .bytes()
            .fold(OFFSET, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME));
        format!("{h:016x}")
    }
    let mut lines = Vec::new();
    for e in entries {
        let what = match e.kind {
            Format::Block => {
                // Its tool tags and level are data (tags), reloaded with the rest.
                let b = BlockProps {
                    tools: Vec::new(),
                    tool_level: None,
                    ..e.block.clone()
                };
                format!("{b:?} {:?} {:?}", e.item, e.requires)
            }
            _ => format!("{:?} {:?}", e.item, e.requires),
        };
        lines.push(format!("{} {} {}", e.kind.noun(), e.id, hash(&what)));
    }
    for e in entities {
        lines.push(format!(
            "{} {} {}",
            yog_ids::kind::ENTITY,
            e.id,
            hash(&format!("{:?}", e.data))
        ));
    }
    for (id, data) in dimensions {
        lines.push(format!(
            "{} {id} {}",
            yog_ids::kind::DIMENSION,
            hash(&format!("{data:?}"))
        ));
    }
    for g in &defs.sound_groups {
        lines.push(format!(
            "{} {} {}",
            yog_ids::kind::SOUND_GROUP,
            g.id,
            hash(&format!("{g:?}"))
        ));
    }
    for m in &defs.tool_materials {
        lines.push(format!(
            "{} {} {}",
            yog_ids::kind::TOOL_MATERIAL,
            m.id,
            hash(&format!("{m:?}"))
        ));
    }
    for m in &defs.armor_materials {
        lines.push(format!(
            "{} {} {}",
            yog_ids::kind::ARMOR_MATERIAL,
            m.id,
            hash(&format!("{m:?}"))
        ));
    }
    lines.sort();
    lines.join("\n") + "\n"
}

/// Whether any `src/**/*.rs` has `content!(` — the include of the generated code.
pub(super) fn mentions_content_macro(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        if p.is_dir() {
            mentions_content_macro(&p)
        } else {
            p.extension().is_some_and(|x| x == "rs")
                && std::fs::read_to_string(&p).is_ok_and(|t| t.contains("content!("))
        }
    })
}

/// `ruby_ore`, `ores/ruby` → `ores_ruby`; a Rust keyword gets `r#`.
pub(super) fn rust_ident(path: &str) -> String {
    let mut s: String = path
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert(0, '_');
    }
    s
}

pub(super) fn float(n: f64) -> String {
    let s = format!("{n:?}");
    if s.contains('.') || s.contains('e') {
        s
    } else {
        format!("{s}.0")
    }
}

/// `name`, unique among those `used` so far: `ruby`, then `ruby_2`.
fn unique_ident(used: &mut HashMap<String, usize>, name: String) -> String {
    let n = used.entry(name.clone()).or_insert(0);
    *n += 1;
    if *n > 1 { format!("{name}_{n}") } else { name }
}

/// `pub struct <name>` with a `pub const <PATH>: &str = "<id>"` per id —
/// what a mod's code names its content by. Nothing for no ids.
fn id_consts<'a>(doc: &str, name: &str, ids: impl Iterator<Item = &'a str>) -> String {
    let ids: Vec<&str> = ids.collect();
    if ids.is_empty() {
        return String::new();
    }
    let mut code = format!(
        "\n/// {doc}\n#[allow(dead_code)]\npub struct {name};\n\n#[allow(dead_code, clippy::all)]\nimpl {name} {{\n"
    );
    let mut used: HashMap<String, usize> = HashMap::new();
    for id in ids {
        let ident = unique_ident(
            &mut used,
            rust_ident(yog_ids::split(id).1).to_ascii_uppercase(),
        );
        let _ = writeln!(code, "    pub const {ident}: &'static str = {id:?};");
    }
    code.push_str("}\n");
    code
}

/// What the generated code is made of: every kind of content the build read.
pub(super) struct Content<'a> {
    pub entries: &'a [Entry],
    pub recipes: &'a [Recipe],
    pub books: &'a [BookFile],
    pub bodies: &'a [BodyFile],
    pub entities: &'a [EntityFile],
    pub fonts: &'a [FontFile],
    pub defs: &'a Defs,
    pub textures: &'a std::collections::BTreeSet<String>,
}

impl Content<'_> {
    /// Whether the content registers anything: its code's `register` must run.
    pub(super) fn registers_anything(&self) -> bool {
        let d = self.defs;
        self.entries.iter().any(|e| e.auto)
            || !self.recipes.is_empty()
            || !self.books.is_empty()
            || !self.bodies.is_empty()
            || !self.entities.is_empty()
            || !self.fonts.is_empty()
            || !d.sound_groups.is_empty()
            || !d.tool_materials.is_empty()
            || !d.armor_materials.is_empty()
            || !d.biome_features.is_empty()
            || !d.dimensions.is_empty()
            || !d.loot_modifiers.is_empty()
            || !d.definitions.is_empty()
    }
}

/// The content's code, part by part.
pub(super) fn rust_source(c: &Content) -> Source {
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut part = |name: &str, code: String| {
        if !code.trim().is_empty() {
            parts.push((name.to_string(), format!("{GENERATED_HEADER}{code}")));
        }
    };
    let doc = "The entity types of `content/` (`.yentity`), by id.";
    part(
        "entities",
        id_consts(doc, code_set::ENTITIES, c.entities.iter().map(|e| e.id.as_str())),
    );
    let doc = "The advancements of `content/` (`.ydef` kind `advancement`), by id\n/// (`yog_api::advancements::grant`/`done`).";
    part(
        "advancements",
        id_consts(
            doc,
            code_set::ADVANCEMENTS,
            c.defs.advancements.iter().map(String::as_str),
        ),
    );
    for (model, set, what) in [
        (true, code_set::MODELS, "models (`.ymodel`)"),
        (false, code_set::ANIMS, "animations (`.yanim`)"),
    ] {
        let doc = format!("The {what} of `content/`, by id (`yog_api::body::model`/`clip`).");
        part(
            &set.to_ascii_lowercase(),
            id_consts(
                &doc,
                set,
                c.bodies
                    .iter()
                    .filter(|b| b.model == model)
                    .map(|b| b.id.as_str()),
            ),
        );
    }
    let doc = "The textures of `resources/` (`.ytexture`), by id — what the mod's code draws.";
    part(
        "textures",
        id_consts(doc, code_set::TEXTURES, c.textures.iter().map(String::as_str)),
    );
    let idents = blocks::entry_idents(c.entries);
    for (name, code) in blocks::entries_code(c.entries, &idents) {
        part(&name, code);
    }
    part("register", register::register_code(c, &idents));
    part("tables", c.defs.tables.iter().map(table::rust).collect());
    let mut root = String::from(GENERATED_HEADER);
    for (name, _) in &parts {
        let _ = writeln!(
            root,
            "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{GENERATED_PARTS_DIR}/{name}.g.rs\"));"
        );
    }
    Source { root, parts }
}


/// Writes `compiled.source` to [`GENERATED_FILE`] (only when it changed);
/// deletes it when the mod has no content files.
pub fn write_source(root: &Path, compiled: &Compiled) -> Result<(), String> {
    write_tags(root, compiled)?;
    let out = root.join(GENERATED_FILE);
    let dir = out.parent().unwrap_or(root);
    let parts_dir = dir.join(GENERATED_PARTS_DIR);
    let Some(source) = &compiled.source else {
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_dir_all(&parts_dir);
        return Ok(());
    };
    std::fs::create_dir_all(&parts_dir).map_err(|e| format!("{}: {e}", parts_dir.display()))?;
    let wanted: Vec<std::path::PathBuf> = source
        .parts
        .iter()
        .map(|(name, _)| parts_dir.join(format!("{name}.g.rs")))
        .collect();
    for ((_, code), file) in source.parts.iter().zip(&wanted) {
        crate::write_if_changed(file, code)?;
    }
    // Parts the content no longer has.
    for entry in std::fs::read_dir(&parts_dir)
        .map_err(|e| format!("{}: {e}", parts_dir.display()))?
        .flatten()
    {
        if !wanted.contains(&entry.path()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    crate::write_if_changed(&out, &source.root)?;
    for (path, bytes) in &compiled.data_files {
        let file = dir.join(path);
        if std::fs::read(&file).ok().as_deref() != Some(bytes.as_slice()) {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            std::fs::write(&file, bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    Ok(())
}

/// The generated files' header.
const GENERATED_HEADER: &str = "// <auto-generated>\n//     Generated by `yog build` from content/** — don't edit, it's rewritten\n//     on every build. Included by `yog_api::content!()`.\n// </auto-generated>\n";

/// Where the content's code goes besides the root file, under
/// `.yog-build/generated/`: one file a part (`items.g.rs`, `register.g.rs`, …).
pub const GENERATED_PARTS_DIR: &str = "content";

/// The content's code: the root file ([`GENERATED_FILE`]), which only
/// includes the parts, and the parts — (name, code) each.
#[derive(Debug, Clone, Default)]
pub struct Source {
    pub root: String,
    pub parts: Vec<(String, String)>,
}

impl Source {
    /// Part `name`'s code (`"items"`).
    #[cfg(test)]
    pub fn part(&self, name: &str) -> &str {
        self.parts
            .iter()
            .find(|(n, _)| n == name)
            .map_or("", |(_, c)| c.as_str())
    }

    /// Every part's code, for tests that look for a line wherever it is.
    #[cfg(test)]
    pub fn all(&self) -> String {
        self.parts.iter().map(|(_, c)| c.as_str()).collect()
    }
}
