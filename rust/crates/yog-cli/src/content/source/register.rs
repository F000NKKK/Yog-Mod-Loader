//! The generated `register`: every block, item, recipe, definition and
//! piece of data the content registers, and the hook that runs it.

use std::fmt::Write;
use yog_ids::Format;

use super::Content;
use super::blocks::{escaped, item_props};
use super::float;
use crate::content::defs::Defs;
use crate::content::defs::data::{definition_path, loot_path};
use crate::content::defs::worldgen::dimension_path;

/// The data at `path` under `.yog-build/generated/`, as the code includes it.
fn included(path: &str) -> String {
    format!("include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{path}\"))")
}

/// `stmt`, only when the mods `requires` names are all loaded (an optional integration).
fn when_loaded(requires: &[String], stmt: &str) -> String {
    if requires.is_empty() {
        return stmt.to_string();
    }
    let cond: Vec<String> = requires
        .iter()
        .map(|m| format!("::yog_api::mod_loaded({m:?})"))
        .collect();
    format!("if {} {{ {stmt} }}", cond.join(" && "))
}

/// The `register` fn, and when the content registers anything, the hook
/// that runs it as the mod's library loads.
pub(super) fn register_code(c: &Content, idents: &[String]) -> String {
    let mut code = String::from(
        "\n/// Registers every block and item of `content/` whose file doesn't say\n/// `\"register\": \"manual\"` — `yog_api::content!()` calls it after\n/// `Mod::register`.\n#[allow(unused_variables)]\npub fn register(registry: &mut ::yog_api::Registry) {\n",
    );
    entries(&mut code, c, idents);
    defs(&mut code, c.defs);
    for r in c.recipes {
        let _ = writeln!(code, "    {}", when_loaded(&r.requires, &r.registration()));
    }
    data(&mut code, c);
    code.push_str("}\n");
    if c.registers_anything() {
        code.push_str(
            "\n// Runs when the mod's library loads: `export_mod!` calls `register` after\n// `Mod::register`.\n#[::yog_api::ctor::ctor(unsafe, crate_path = ::yog_api::ctor)]\nfn __yog_content_auto_register() {\n    ::yog_api::__yog_content_registrars().lock().push(register);\n}\n",
        );
    }
    code
}

/// The blocks and items registered by themselves (a block with its item
/// form, when the file says how it stacks, burns or survives fire).
fn entries(code: &mut String, c: &Content, idents: &[String]) {
    for (e, ident) in c.entries.iter().zip(idents).filter(|(e, _)| e.auto) {
        let ident = escaped(ident);
        let stmt = if e.kind == Format::Block {
            let item = item_props(&e.item);
            if item.is_empty() {
                format!("registry.register_block(Blocks::{ident}());")
            } else {
                format!(
                    "{{ registry.register_block(Blocks::{ident}()); registry.register_item(::yog_api::ItemDef::new({:?}){}); }}",
                    e.id,
                    item.replace(super::blocks::CALL, ".")
                )
            }
        } else {
            format!("registry.register_item(Items::{ident}());")
        };
        let _ = writeln!(code, "    {}", when_loaded(&e.requires, &stmt));
    }
}

/// What the `.ydef`s define: sound groups, dimensions, biome features, materials.
fn defs(code: &mut String, defs: &Defs) {
    for g in &defs.sound_groups {
        let _ = writeln!(
            code,
            "    registry.register_sound_group(::yog_api::SoundGroup::new({:?}).sounds({:?}, {:?}, {:?}, {:?}, {:?}).volume({}).pitch({}));",
            g.id,
            g.break_sound,
            g.step,
            g.place,
            g.hit,
            g.fall,
            float(f64::from(g.volume)),
            float(f64::from(g.pitch))
        );
    }
    for id in &defs.dimensions {
        let _ = writeln!(
            code,
            "    registry.register_dimension_data({id:?}, {});",
            included(&dimension_path(id))
        );
    }
    for (feature, biomes, step) in &defs.biome_features {
        let _ = writeln!(
            code,
            "    registry.add_biome_feature({feature:?}, {biomes:?}, {step:?});"
        );
    }
    for m in &defs.tool_materials {
        let _ = writeln!(
            code,
            "    registry.register_tool_material(::yog_api::ToolMaterial {{ id: {:?}.into(), durability: {}, speed: {:?}, attack_damage: {:?}, enchantability: {}, level: {:?}.into(), repair: {:?}.into(), level_index: {}, incorrect: {:?}.into(), game_tools: ::std::string::String::new() }});",
            m.id,
            m.durability,
            m.speed,
            m.attack_damage,
            m.enchantability,
            m.level,
            m.repair,
            m.level_index,
            m.incorrect
        );
    }
    for m in &defs.armor_materials {
        let d = m.defense;
        let _ = writeln!(
            code,
            "    registry.register_armor_material(::yog_api::ArmorMaterial {{ id: {:?}.into(), durability: {}, defense: ::yog_api::ArmorDefense {{ head: {}, chest: {}, legs: {}, feet: {} }}, enchantability: {}, equip_sound: {:?}.into(), toughness: {:?}, knockback_resistance: {:?}, repair: {:?}.into(), texture: {:?}.into() }});",
            m.id,
            m.durability,
            d.head,
            d.chest,
            d.legs,
            d.feet,
            m.enchantability,
            m.equip_sound,
            m.toughness,
            m.knockback_resistance,
            m.repair,
            m.texture
        );
    }
}

/// The data the code hands over as it is: books, definitions, loot
/// modifiers, entities, bodies, fonts.
fn data(code: &mut String, c: &Content) {
    for b in c.books {
        let _ = writeln!(
            code,
            "    registry.register_book_data({:?}, {});",
            b.id,
            included(&b.path())
        );
    }
    for (kind, id) in &c.defs.definitions {
        let _ = writeln!(
            code,
            "    registry.register_definition_data({kind:?}, {id:?}, {});",
            included(&definition_path(kind, id))
        );
    }
    for id in &c.defs.loot_modifiers {
        let _ = writeln!(
            code,
            "    registry.add_loot_modifier_data({id:?}, {});",
            included(&loot_path(id))
        );
    }
    for e in c.entities {
        let _ = writeln!(
            code,
            "    registry.register_entity_data({:?}, {});",
            e.id,
            included(&e.path())
        );
    }
    for b in c.bodies {
        let what = if b.model {
            yog_ids::word::MODEL
        } else {
            BODY_ANIMS
        };
        let _ = writeln!(
            code,
            "    registry.register_body_{what}({:?}, {});",
            b.id,
            included(&b.path())
        );
    }
    for font in c.fonts {
        let _ = writeln!(
            code,
            "    registry.register_font_data({:?}, {});",
            font.id,
            included(&font.path())
        );
    }
}

/// What the registry's call for a body's clips ends with: `register_body_anims`.
const BODY_ANIMS: &str = "anims";
