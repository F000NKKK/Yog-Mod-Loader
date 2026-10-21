//! `.ydef` ladders and what's placed on them — levels, tool materials —
//! and the other definitions blocks and items name: armor materials,
//! sound groups.

use std::collections::BTreeMap;
use yog_data::ItemIdOrTag;
use yog_data::OneOrTag;
use yog_formats::def::{
    ArmorMaterial, ArmorMaterialDef, Def, SoundGroup, ToolMaterial, ToolMaterialDef,
};
use yog_ids::sep::{TAG, WORD};
use yog_registry::ToolLadder;

use crate::content::Compiled;
use crate::content::datapack;
use crate::content::defs::{DefFile, Defs};

/// What the tag of what a tool level above the game's can't mine is named:
/// `<ns>:incorrect_for_<level>_tool`, as the game's are.
const INCORRECT_FOR: &str = "incorrect_for";
const TOOL: &str = "tool";

/// A level, before it's placed on its ladder.
struct Level {
    file: String,
    line: usize,
    id: String,
    ladder: String,
    above: Option<String>,
}

/// What the ladder kinds of `.ydef` say, as read.
struct Found<'a> {
    ns: &'a str,
    order: BTreeMap<String, Vec<String>>,
    levels: Vec<Level>,
    /// Tool materials and their files: their levels are placed first.
    tools: Vec<(String, ToolMaterialDef)>,
    armor_materials: Vec<ArmorMaterial>,
    sound_groups: Vec<SoundGroup>,
}

/// What a material is repaired with: an item, or `#` and a tag.
fn repair(r: &ItemIdOrTag) -> String {
    match r {
        OneOrTag::One(i) => i.id.clone(),
        OneOrTag::Tag(t) => format!("{TAG}{}", t.id),
    }
}

impl Found<'_> {
    /// `id` with the mod's namespace when it has none.
    fn full(&self, id: &str) -> String {
        yog_ids::qualify_in(self.ns, id)
    }

    fn take(&mut self, f: &DefFile, errors: &mut Vec<String>) {
        let file = &f.shown;
        match &f.read.value {
            Def::Ladder { id } => {
                let id = self.full(id);
                if self.order.insert(id.clone(), Vec::new()).is_some() {
                    errors.push(format!("{file}:1: ladder {id} is defined twice"));
                }
            }
            Def::Level { id, ladder, above } => {
                let line = f.located.line_of(yog_ids::word::ID);
                self.levels.push(Level {
                    file: file.clone(),
                    line,
                    id: self.full(id),
                    ladder: ladder.clone(),
                    above: above.clone(),
                });
            }
            Def::SoundGroup(g) => self.sound_group(file, g.clone(), errors),
            Def::ToolMaterial(m) => {
                let mut m = m.clone();
                m.id = self.full(&m.id);
                self.tools.push((file.clone(), m));
            }
            Def::ArmorMaterial(m) => self.armor_material(file, m, errors),
            // Checked with the content they name (`compile_def`).
            _ => {}
        }
    }

    fn sound_group(&mut self, file: &str, mut g: SoundGroup, errors: &mut Vec<String>) {
        // Short ids — the group's and its sounds' — are the mod's.
        for s in [
            &mut g.id,
            &mut g.break_sound,
            &mut g.step,
            &mut g.place,
            &mut g.hit,
            &mut g.fall,
        ] {
            *s = yog_ids::qualify_in(self.ns, s);
        }
        if SoundGroup::built_in(&g.id).is_some() {
            errors.push(format!("{file}:1: sound group {} is the game's", g.id));
        } else if self.sound_groups.iter().any(|x| x.id == g.id) {
            errors.push(format!("{file}:1: sound group {} is defined twice", g.id));
        } else {
            self.sound_groups.push(g);
        }
    }

    fn armor_material(&mut self, file: &str, m: &ArmorMaterialDef, errors: &mut Vec<String>) {
        let id = self.full(&m.id);
        if ArmorMaterial::built_in(&id).is_some() || self.armor_materials.iter().any(|x| x.id == id)
        {
            errors.push(format!(
                "{file}:1: armor material {id} is the game's or defined twice"
            ));
            return;
        }
        self.armor_materials.push(ArmorMaterial {
            texture: m
                .texture
                .as_deref()
                .map_or_else(|| id.clone(), |t| self.full(t)),
            equip_sound: self.full(&m.equip_sound),
            durability: m.durability,
            defense: m.defense,
            enchantability: m.enchantability,
            toughness: m.toughness,
            knockback_resistance: m.knockback_resistance,
            repair: repair(&m.repair),
            id,
        });
    }
}

/// Reads the `.ydef`s' ladders, the levels placed on them (`above`, in any
/// order of files), tool and armor materials and sound groups.
pub(in crate::content) fn read_defs(files: &[DefFile], ns: &str, errors: &mut Vec<String>) -> Defs {
    let game = ToolLadder::game();
    let order = BTreeMap::from([(
        game.ladder.clone(),
        game.ids().map(str::to_string).collect(),
    )]);
    let mut found = Found {
        ns,
        order,
        levels: Vec::new(),
        tools: Vec::new(),
        armor_materials: Vec::new(),
        sound_groups: Vec::new(),
    };
    for f in files {
        found.take(f, errors);
    }
    place_levels(&mut found.order, found.levels, errors);
    let tool_materials = tool_materials(&found.order, found.tools, errors);
    Defs {
        order: found.order,
        sound_groups: found.sound_groups,
        tool_materials,
        armor_materials: found.armor_materials,
        ..Defs::default()
    }
}

/// Places each level once what it's above is placed.
fn place_levels(
    order: &mut BTreeMap<String, Vec<String>>,
    mut levels: Vec<Level>,
    errors: &mut Vec<String>,
) {
    loop {
        let before = levels.len();
        levels.retain(|l| {
            let Some(list) = order.get_mut(&l.ladder) else {
                errors.push(format!(
                    "{}:{}: level {}: no ladder {}",
                    l.file, l.line, l.id, l.ladder
                ));
                return false;
            };
            let at = match &l.above {
                None => 0,
                Some(a) => match list.iter().position(|x| x == a) {
                    Some(i) => i + 1,
                    None => return true, // later
                },
            };
            if list.contains(&l.id) {
                errors.push(format!(
                    "{}:{}: level {} is defined twice",
                    l.file, l.line, l.id
                ));
            } else {
                list.insert(at, l.id.clone());
            }
            false
        });
        if levels.is_empty() || levels.len() == before {
            break;
        }
    }
    for l in levels {
        let above = l.above.unwrap_or_default();
        errors.push(format!("{}:{}: level {}: above {above}, which isn't a level of {} (or the levels go round in a circle)", l.file, l.line, l.id, l.ladder));
    }
}

/// Tool materials: what their level is on each version — 1.20's level
/// number and 1.21's tag of what they can't mine, as blocks of their
/// level are tagged ([`tool_level_tags`]: the weakest of the game's levels
/// at or above it).
fn tool_materials(
    order: &BTreeMap<String, Vec<String>>,
    tools: Vec<(String, ToolMaterialDef)>,
    errors: &mut Vec<String>,
) -> Vec<ToolMaterial> {
    let ladder = order
        .get(&ToolLadder::game().ladder)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<ToolMaterial> = Vec::new();
    for (file, m) in tools {
        if ToolMaterial::built_in(&m.id).is_some() || out.iter().any(|x| x.id == m.id) {
            errors.push(format!(
                "{file}:1: tool material {} is the game's or defined twice",
                m.id
            ));
            continue;
        }
        let Some(at) = ladder.iter().position(|l| l == &m.level) else {
            errors.push(format!(
                "{file}:1: level: {} isn't a level of the tool ladder",
                m.level
            ));
            continue;
        };
        let (level_index, incorrect) = game_level(&ladder, at, &m.level);
        out.push(ToolMaterial {
            id: m.id,
            durability: m.durability,
            speed: m.speed,
            attack_damage: m.attack_damage,
            enchantability: m.enchantability,
            level: m.level,
            repair: repair(&m.repair),
            level_index,
            incorrect,
            game_tools: String::new(),
        });
    }
    out
}

/// Level `level`, at place `at` on the tool ladder: the game's level number
/// it counts as, and its tag of what it can't mine — above the game's
/// levels, a tag of its own (the build writes it).
fn game_level(ladder: &[String], at: usize, level: &str) -> (u32, String) {
    let game = ToolLadder::game();
    match game_level_at(ladder, at).and_then(|i| Some((i, game.levels.get(i)?))) {
        Some((i, l)) => (i as u32, l.incorrect.clone()),
        None => {
            let (ns, path) = yog_ids::split(level);
            (
                game.levels.len() as u32,
                yog_ids::join(ns, &[INCORRECT_FOR, path, TOOL].join(WORD)),
            )
        }
    }
}

/// The tags that make block `id` need tool level `level` on every loader and
/// version: those of the weakest of the game's levels at or above it
/// ([`ToolLadder`]), or of a level above them all. `Err` when `level`
/// isn't on the tool ladder; with a caveat to warn of when it's above it.
pub(in crate::content) fn tool_level_tags(
    id: &str,
    level: &str,
    ladders: &Defs,
    out: &mut Compiled,
) -> Result<Option<String>, String> {
    let game = ToolLadder::game();
    let Some((ladder, at)) = ladders.level(level) else {
        return Ok(None);
    };
    if ladder != game.ladder {
        return Err(format!(
            "{level} is a level of {ladder}, not of the tool ladder ({})",
            game.ladder
        ));
    }
    let tools = ladders
        .order
        .get(&game.ladder)
        .map_or(&[][..], Vec::as_slice);
    let (needs, caveat) = match game_level_at(tools, at).and_then(|l| game.levels.get(l)) {
        Some(l) => (&l.needs, None),
        None => (&game.above.needs, Some(game.above.caveat.clone())),
    };
    for t in needs {
        out.tags
            .entry(datapack::overlay(
                &t.when,
                &datapack::tag_file(yog_ids::kind::BLOCK, &t.tag),
            ))
            .or_default()
            .insert(id.to_string(), true);
    }
    Ok(caveat)
}

/// The index of the weakest of the game's tool levels at or above place `at`
/// on the tool ladder `tools` (with the mods' levels in it); `None` above them all.
fn game_level_at(tools: &[String], at: usize) -> Option<usize> {
    let game = ToolLadder::game();
    game.levels.iter().position(|l| {
        tools
            .iter()
            .position(|x| *x == l.id)
            .is_some_and(|p| p >= at)
    })
}
