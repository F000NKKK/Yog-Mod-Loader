//! The Rust the build generates for a mod's content (`yog-content.g.rs`).

use yog_ids::Format;
use std::collections::HashMap;
use std::path::Path;
use crate::content::BlockProps;
use crate::content::BodyFile;
use crate::content::BookFile;
use crate::content::Compiled;
use crate::content::Defs;
use crate::content::EntityFile;
use crate::content::Entry;
use crate::content::FontFile;
use crate::content::GENERATED_FILE;
use crate::content::GENERATED_HEADER;
use crate::content::GENERATED_PARTS_DIR;
use crate::content::Recipe;
use crate::content::Source;
use crate::content::block_item::item_props;
use crate::content::defs::definition_path;
use crate::content::defs::dimension_path;
use crate::content::defs::loot_path;
use crate::content::outputs::write_tags;
use crate::content::table;
use std::fmt::Write;


/// [`yog_registry::REGISTRATIONS_FILE`]: what the mod registers at the
/// game's start, each with a hash of what can't change while it runs — a
/// block's properties and states, not its model, name or tags (the game
/// reloads those).
pub(super) fn registrations(entries: &[Entry], entities: &[EntityFile], dimensions: &[(String, Vec<u8>)], defs: &Defs) -> String {
    fn hash(text: &str) -> String {
        // FNV-1a: the same on every build of the CLI.
        let h = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        format!("{h:016x}")
    }
    let mut lines = Vec::new();
    for e in entries {
        let what = match e.kind {
            Format::Block => {
                // Its tool tags and level are data (tags), reloaded with the rest.
                let b = BlockProps { tools: Vec::new(), tool_level: None, ..e.block.clone() };
                format!("{b:?} {:?} {:?}", e.item, e.requires)
            }
            _ => format!("{:?} {:?}", e.item, e.requires),
        };
        lines.push(format!("{} {} {}", e.kind.noun(), e.id, hash(&what)));
    }
    for e in entities {
        lines.push(format!("entity {} {}", e.id, hash(&format!("{:?}", e.data))));
    }
    for (id, data) in dimensions {
        lines.push(format!("dimension {id} {}", hash(&format!("{data:?}"))));
    }
    for g in &defs.sound_groups {
        lines.push(format!("sound_group {} {}", g.id, hash(&format!("{g:?}"))));
    }
    for m in &defs.tool_materials {
        lines.push(format!("tool_material {} {}", m.id, hash(&format!("{m:?}"))));
    }
    for m in &defs.armor_materials {
        lines.push(format!("armor_material {} {}", m.id, hash(&format!("{m:?}"))));
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

/// `stmt`, only when the mods `requires` names are all loaded (an optional integration).
pub(super) fn when_loaded(requires: &[String], stmt: &str) -> String {
    if requires.is_empty() {
        return stmt.to_string();
    }
    let cond: Vec<String> = requires.iter().map(|m| format!("::yog_api::mod_loaded({m:?})")).collect();
    format!("if {} {{ {stmt} }}", cond.join(" && "))
}

/// Moves what `code` has so far into part `name`.
pub(super) fn flush(parts: &mut Vec<(String, String)>, code: &mut String, name: &str) {
    if !code.trim().is_empty() {
        parts.push((name.to_string(), format!("{GENERATED_HEADER}{}", std::mem::take(code))));
    }
    code.clear();
}

pub(super) fn rust_source(entries: &[Entry], recipes: &[Recipe], books: &[BookFile], bodies: &[BodyFile], entities: &[EntityFile], fonts: &[FontFile], defs: &Defs) -> Source {
    let sound_groups = &defs.sound_groups;
    let mut files: Vec<(String, String)> = Vec::new();
    let mut code = String::new();
    if !entities.is_empty() {
        code.push_str("\n/// The entity types of `content/` (`.yentity`), by id.\n#[allow(dead_code)]\npub struct Entities;\n\n#[allow(dead_code, clippy::all)]\nimpl Entities {\n");
        let mut used: HashMap<String, usize> = HashMap::new();
        for e in entities {
            let path = e.id.split_once(':').map_or(e.id.as_str(), |(_, p)| p);
            let mut ident = rust_ident(path).to_ascii_uppercase();
            let n = used.entry(ident.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let _ = write!(ident, "_{n}");
            }
            let _ = writeln!(code, "    pub const {ident}: &'static str = {:?};", e.id);
        }
        code.push_str("}\n");
        flush(&mut files, &mut code, "entities");
    }
    if !defs.advancements.is_empty() {
        code.push_str("\n/// The advancements of `content/` (`.ydef` kind `advancement`), by id\n/// (`yog_api::advancements::grant`/`done`).\n#[allow(dead_code)]\npub struct Advancements;\n\n#[allow(dead_code, clippy::all)]\nimpl Advancements {\n");
        let mut used: HashMap<String, usize> = HashMap::new();
        for id in &defs.advancements {
            let path = id.split_once(':').map_or(id.as_str(), |(_, p)| p);
            let mut ident = rust_ident(path).to_ascii_uppercase();
            let n = used.entry(ident.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let _ = write!(ident, "_{n}");
            }
            let _ = writeln!(code, "    pub const {ident}: &'static str = {id:?};");
        }
        code.push_str("}\n");
        flush(&mut files, &mut code, "advancements");
    }
    for (model, set, what) in [(true, "Models", "models (`.ymodel`)"), (false, "Anims", "animations (`.yanim`)")] {
        let mine: Vec<&BodyFile> = bodies.iter().filter(|b| b.model == model).collect();
        if mine.is_empty() {
            continue;
        }
        let _ = writeln!(code, "\n/// The {what} of `content/`, by id (`yog_api::body::model`/`clip`).\n#[allow(dead_code)]\npub struct {set};\n\n#[allow(dead_code, clippy::all)]\nimpl {set} {{");
        let mut used: HashMap<String, usize> = HashMap::new();
        for b in mine {
            let path = b.id.split_once(':').map_or(b.id.as_str(), |(_, p)| p);
            let mut ident = rust_ident(path).to_ascii_uppercase();
            let n = used.entry(ident.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let _ = write!(ident, "_{n}");
            }
            let _ = writeln!(code, "    pub const {ident}: &'static str = {:?};", b.id);
        }
        code.push_str("}\n");
        flush(&mut files, &mut code, &set.to_ascii_lowercase());
    }
    for (kind, set, def) in [
        (Format::Block, "Blocks", "BlockDef"),
        (Format::Item, "Items", "ItemDef"),
    ] {
        let mine: Vec<&Entry> = entries.iter().filter(|e| e.kind == kind).collect();
        let _ = writeln!(
            code,
            "\n/// The {} of `content/`: the id and a [`::yog_api::{def}`] of each.\n#[allow(dead_code)]\npub struct {set};\n\n#[allow(dead_code, clippy::all)]\nimpl {set} {{",
            if kind == Format::Block {
                "blocks"
            } else {
                "items"
            }
        );
        let mut used: HashMap<String, usize> = HashMap::new();
        for e in mine {
            let path = e.id.split_once(':').map_or(e.id.as_str(), |(_, p)| p);
            let mut ident = rust_ident(path);
            let n = used.entry(ident.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let _ = write!(ident, "_{n}");
            }
            let upper = ident.to_ascii_uppercase();
            let ident = if is_keyword(&ident) {
                format!("r#{ident}")
            } else {
                ident
            };
            let _ = writeln!(
                code,
                "    /// `{}` — `{}`.\n    pub const {upper}: &'static str = {:?};",
                e.id, e.file, e.id
            );
            let mut b = format!("::yog_api::{def}::new(Self::{upper})");
            match kind {
                Format::Block => {
                    let p = &e.block;
                    if p.hardness.is_some() || p.resistance.is_some() {
                        let _ = write!(
                            b,
                            "\n            .strength({}, {})",
                            float(p.hardness.unwrap_or(1.5)),
                            float(p.resistance.unwrap_or(6.0))
                        );
                    }
                    if let Some(s) = &p.sound {
                        let _ = write!(b, "\n            .sound({s:?})");
                    }
                    if p.light > 0 {
                        let _ = write!(b, "\n            .light_level({})", p.light);
                    }
                    if p.requires_tool {
                        b.push_str("\n            .requires_tool()");
                    }
                    if p.no_collision {
                        b.push_str("\n            .no_collision()");
                    }
                    if let Some(s) = p.slipperiness {
                        let _ = write!(b, "\n            .slipperiness({})", float(s));
                    }
                    if let Some(s) = p.shape {
                        let args: Vec<String> = s.iter().map(|n| float(*n)).collect();
                        let _ = write!(b, "\n            .shape({})", args.join(", "));
                    }
                    if p.connects {
                        b.push_str("\n            .connects_to_neighbors()");
                    }
                    if !p.connect_groups.is_empty() {
                        let groups: Vec<String> =
                            p.connect_groups.iter().map(|g| format!("{g:?}")).collect();
                        let _ =
                            write!(b, "\n            .connect_groups(&[{}])", groups.join(", "));
                    }
                    if let Some(inv) = &p.inventory {
                        let _ = write!(b, "\n            .inventory({inv:?})");
                    }
                    if p.redstone {
                        b.push_str("\n            .redstone_source()");
                    }
                    if !p.facing.is_empty() {
                        let _ = write!(b, "\n            .facing({:?}, {:?})", p.facing, p.placement);
                    }
                    if p.attach {
                        b.push_str("\n            .attach()");
                    }
                    if p.waterlogged {
                        b.push_str("\n            .waterlogged()");
                    }
                    for (name, values) in &p.properties {
                        let vs: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
                        let _ = write!(b, "\n            .property({name:?}, &[{}])", vs.join(", "));
                    }
                    if let Some(name) = &p.powered {
                        let _ = write!(b, "\n            .powered({name:?})");
                    }
                    if let Some(name) = &p.comparator {
                        let _ = write!(b, "\n            .comparator({name:?})");
                    }
                    if p.accepts_items {
                        b.push_str("\n            .accepts_items()");
                    }
                    if p.no_occlusion {
                        b.push_str("\n            .no_occlusion()");
                    }
                    if !p.kind.is_empty() {
                        let _ = write!(b, "\n            .kind({:?})", p.kind);
                    }
                    if let Some(base) = &p.base {
                        let _ = write!(b, "\n            .base({base:?})");
                    }
                    if let Some(set) = &p.set {
                        let _ = write!(b, "\n            .set({set:?})");
                    }
                    if let Some(t) = p.press_ticks {
                        let _ = write!(b, "\n            .press_ticks({t})");
                    }
                    if p.mobs_only {
                        b.push_str("\n            .mobs_only()");
                    }
                }
                Format::Item => b.push_str(&item_props(&e.item)),
                _ => unreachable!("only blocks and items get here"),
            }
            let _ = writeln!(
                code,
                "    /// The definition from `{}` (name and tooltip come from the lang files).\n    pub fn {ident}() -> ::yog_api::{def} {{\n        {b}\n    }}",
                e.file
            );
        }
        code.push_str("}\n");
        flush(&mut files, &mut code, &set.to_ascii_lowercase());
    }

    code.push_str("\n/// Registers every block and item of `content/` whose file doesn't say\n/// `\"register\": \"manual\"` — `yog_api::content!()` calls it after\n/// `Mod::register`.\n#[allow(unused_variables)]\npub fn register(registry: &mut ::yog_api::Registry) {\n");
    let mut used: HashMap<(Format, String), usize> = HashMap::new();
    for e in entries {
        let path = e.id.split_once(':').map_or(e.id.as_str(), |(_, p)| p);
        let mut ident = rust_ident(path);
        let n = used.entry((e.kind, ident.clone())).or_insert(0);
        *n += 1;
        if *n > 1 {
            let _ = write!(ident, "_{n}");
        }
        let ident = if is_keyword(&ident) {
            format!("r#{ident}")
        } else {
            ident
        };
        if !e.auto {
            continue;
        }
        let _ = match e.kind {
            Format::Block => {
                // Its item form, when the file says how it stacks, burns or survives fire.
                let item = item_props(&e.item);
                let stmt = if item.is_empty() {
                    format!("registry.register_block(Blocks::{ident}());")
                } else {
                    format!("{{ registry.register_block(Blocks::{ident}()); registry.register_item(::yog_api::ItemDef::new({:?}){}); }}", e.id, item.replace("\n            ", ""))
                };
                writeln!(code, "    {}", when_loaded(&e.requires, &stmt))
            }
            Format::Item => writeln!(code, "    {}", when_loaded(&e.requires, &format!("registry.register_item(Items::{ident}());"))),
            _ => unreachable!("only blocks and items get here"),
        };
    }
    for g in sound_groups {
        let _ = writeln!(
            code,
            "    registry.register_sound_group(::yog_api::SoundGroup::new({:?}).sounds({:?}, {:?}, {:?}, {:?}, {:?}).volume({}).pitch({}));",
            g.id,
            g.break_sound,
            g.step,
            g.place,
            g.hit,
            g.fall,
            float(g.volume as f64),
            float(g.pitch as f64)
        );
    }
    for id in &defs.dimensions {
        let _ = writeln!(
            code,
            "    registry.register_dimension_data({id:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            dimension_path(id)
        );
    }
    for (feature, biomes, step) in &defs.biome_features {
        let _ = writeln!(code, "    registry.add_biome_feature({feature:?}, {biomes:?}, {step:?});");
    }
    for m in &defs.tool_materials {
        let _ = writeln!(
            code,
            "    registry.register_tool_material(::yog_api::ToolMaterial {{ id: {:?}.into(), durability: {}, speed: {:?}, attack_damage: {:?}, enchantability: {}, level: {:?}.into(), repair: {:?}.into(), level_index: {}, incorrect: {:?}.into() }});",
            m.id, m.durability, m.speed, m.attack_damage, m.enchantability, m.level, m.repair, m.level_index, m.incorrect
        );
    }
    for m in &defs.armor_materials {
        let d = m.defense;
        let _ = writeln!(
            code,
            "    registry.register_armor_material(::yog_api::ArmorMaterial {{ id: {:?}.into(), durability: {}, defense: ::yog_api::ArmorDefense {{ head: {}, chest: {}, legs: {}, feet: {} }}, enchantability: {}, equip_sound: {:?}.into(), toughness: {:?}, knockback_resistance: {:?}, repair: {:?}.into(), texture: {:?}.into() }});",
            m.id, m.durability, d.head, d.chest, d.legs, d.feet, m.enchantability, m.equip_sound, m.toughness, m.knockback_resistance, m.repair, m.texture
        );
    }
    for r in recipes {
        let _ = writeln!(code, "    {}", when_loaded(&r.requires, &r.registration()));
    }
    for b in books {
        let _ = writeln!(
            code,
            "    registry.register_book_data({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            b.id,
            b.path()
        );
    }
    for (kind, id) in &defs.definitions {
        let _ = writeln!(
            code,
            "    registry.register_definition_data({kind:?}, {id:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            definition_path(kind, id)
        );
    }
    for id in &defs.loot_modifiers {
        let _ = writeln!(
            code,
            "    registry.add_loot_modifier_data({id:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            loot_path(id)
        );
    }
    for e in entities {
        let _ = writeln!(
            code,
            "    registry.register_entity_data({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            e.id,
            e.path()
        );
    }
    for b in bodies {
        let _ = writeln!(
            code,
            "    registry.register_body_{}({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            if b.model { "model" } else { "anims" },
            b.id,
            b.path()
        );
    }
    for font in fonts {
        let _ = writeln!(
            code,
            "    registry.register_font_data({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            font.id,
            font.path()
        );
    }
    code.push_str("}\n");
    if entries.iter().any(|e| e.auto) || !recipes.is_empty() || !books.is_empty() || !bodies.is_empty() || !entities.is_empty() || !fonts.is_empty() || !sound_groups.is_empty() || !defs.tool_materials.is_empty() || !defs.armor_materials.is_empty() || !defs.biome_features.is_empty() || !defs.dimensions.is_empty() {
        code.push_str(
            "\n// Runs when the mod's library loads: `export_mod!` calls `register` after\n// `Mod::register`.\n#[::yog_api::ctor::ctor(unsafe, crate_path = ::yog_api::ctor)]\nfn __yog_content_auto_register() {\n    ::yog_api::__yog_content_registrars()\n        .lock()\n        .unwrap_or_else(|e| e.into_inner())\n        .push(register);\n}\n",
        );
    }
    flush(&mut files, &mut code, "register");
    for t in &defs.tables {
        code.push_str(&table::rust(t));
    }
    flush(&mut files, &mut code, "tables");
    let mut root = String::from(GENERATED_HEADER);
    for (name, _) in &files {
        let _ = writeln!(root, "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{GENERATED_PARTS_DIR}/{name}.g.rs\"));");
    }
    Source { root, parts: files }
}

pub(super) fn is_keyword(s: &str) -> bool {
    const KW: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
        "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final",
        "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
    ];
    KW.contains(&s)
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
    let wanted: Vec<std::path::PathBuf> = source.parts.iter().map(|(name, _)| parts_dir.join(format!("{name}.g.rs"))).collect();
    for ((_, code), file) in source.parts.iter().zip(&wanted) {
        crate::write_if_changed(file, code)?;
    }
    // Parts the content no longer has.
    for entry in std::fs::read_dir(&parts_dir).map_err(|e| format!("{}: {e}", parts_dir.display()))?.flatten() {
        if !wanted.contains(&entry.path()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    crate::write_if_changed(&out, &source.root)?;
    for (path, bytes) in &compiled.data_files {
        let file = dir.join(path);
        if std::fs::read(&file).ok().as_deref() != Some(bytes.as_slice()) {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            std::fs::write(&file, bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    Ok(())
}
