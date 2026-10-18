//! `.ydef` definitions: ladders and levels, materials, tags, trees, ores,
//! dimensions, damage types, advancements — and the files they become.

use yog_data::Path as At;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use crate::content::Compiled;
use crate::content::Ctx;
use crate::content::Defs;
use crate::content::Lang;
use crate::content::OreOut;
use crate::content::Output;
use crate::content::block_item::text_key;
use crate::content::datapack;
use crate::content::pretty;
use crate::content::refs;
use crate::content::valid_path;


/// A dimension's data file under `.yog-build/generated/`.
pub(super) fn dimension_path(id: &str) -> String {
    format!("dimensions/{}.ydata", id.replace(':', "__"))
}

/// Minecraft versions before 1.20.5 write advancements the old way: an
/// icon's `item`, item predicates' `items` lists and `tag`s.
pub(super) fn old_advancement_format(version: &str) -> bool {
    let mut p = version.split('.').map(|n| n.parse::<u32>().unwrap_or(0));
    let (major, minor, patch) = (p.next().unwrap_or(0), p.next().unwrap_or(0), p.next().unwrap_or(0));
    (major, minor, patch) < (1, 20, 5)
}

/// A `.ydef` advancement: its JSON in the newest format, and the old one as
/// an overlay for the versions that read that. `Err((field, why))`.
pub(super) fn advancement(cx: &mut Ctx, lang: &mut Lang, a: &yog_formats::def::AdvancementDef, ns: &str) -> Result<Vec<Output>, (&'static str, String)> {
    use serde_json::json;
    let id = yog_ids::qualify_in(ns, &a.id);
    let path = id.strip_prefix(&format!("{ns}:")).filter(|p| valid_path(p)).ok_or(("id", format!("`{id}`: the mod's own, `{ns}:<name>`")))?.to_string();
    if !matches!(a.frame.as_str(), "task" | "goal" | "challenge") {
        return Err(("frame", format!("`{}`: task, goal or challenge", a.frame)));
    }
    let base_key = format!("advancement.{ns}.{}", path.replace('/', "."));
    let title = text_key(cx, lang, &a.title, ns, base_key.clone()).map_err(|e| ("title", e))?;
    let description = match &a.description {
        Some(d) => json!({ "translate": text_key(cx, lang, d, ns, format!("{base_key}.desc")).map_err(|e| ("description", e))? }),
        None => json!({ "text": "" }),
    };
    let make = |old: bool| -> Result<Value, (&'static str, String)> {
        let item_predicate = |item: &str| if old { json!({ "items": [item] }) } else { json!({ "items": item }) };
        let mut criteria = Map::new();
        let mut requirements = Vec::new();
        for (i, c) in a.criteria.iter().enumerate() {
            let kinds = c.granted as u8 + c.has_item.is_some() as u8 + c.used_item.is_some() as u8;
            if kinds != 1 {
                return Err(("criteria", format!("criteria[{i}]: one of granted, has_item, used_item")));
            }
            let name = format!("c{i}");
            let criterion = if c.granted {
                json!({ "trigger": "minecraft:impossible" })
            } else if let Some(item) = &c.has_item {
                let predicate = match item {
                    yog_data::OneOrTag::One(i) => item_predicate(&yog_ids::qualify_in("minecraft", &i.id)),
                    yog_data::OneOrTag::Tag(t) if old => json!({ "tag": yog_ids::qualify_in("minecraft", &t.id) }),
                    yog_data::OneOrTag::Tag(t) => json!({ "items": format!("#{}", yog_ids::qualify_in("minecraft", &t.id)) }),
                };
                json!({ "trigger": "minecraft:inventory_changed", "conditions": { "items": [predicate] } })
            } else {
                let item = c.used_item.as_ref().map(|i| yog_ids::qualify_in("minecraft", &i.id)).unwrap_or_default();
                json!({ "trigger": "minecraft:consume_item", "conditions": { "item": item_predicate(&item) } })
            };
            criteria.insert(name.clone(), criterion);
            requirements.push(json!([name]));
        }
        let mut display = json!({
            "icon": if old { json!({ "item": a.icon }) } else { json!({ "id": a.icon }) },
            "title": { "translate": title },
            "description": description,
            "frame": a.frame,
            "hidden": a.hidden,
            "show_toast": true,
            "announce_to_chat": true,
        });
        if let Some(bg) = &a.background {
            display["background"] = Value::String(bg.clone());
        }
        let mut v = json!({ "display": display, "criteria": criteria, "requirements": requirements });
        if let Some(parent) = &a.parent {
            v["parent"] = Value::String(yog_ids::qualify_in(ns, parent));
        }
        Ok(v)
    };
    let file = format!("data/{ns}/advancement/{path}.json");
    let mut out = vec![(file.clone(), pretty(&make(false)?))];
    for version in refs::versions().iter().filter(|v| old_advancement_format(v)) {
        out.push((datapack::for_version(version, &file), pretty(&make(true)?)));
    }
    Ok(out)
}

/// A definition the game registers (`attribute`, `effect`, `potion`): the
/// mod's own id, kept as data for the code to register.
#[allow(clippy::too_many_arguments)]
pub(super) fn content_def<T: yog_data::Schema>(
    cx: &mut Ctx,
    doc: &yog_data::text::Doc,
    text: &str,
    ns: &str,
    kind: &'static str,
    id: String,
    def: &T,
    ok: bool,
    definitions: &mut Vec<(&'static str, String, Vec<u8>)>,
) {
    if id.split_once(':').is_none_or(|(n, p)| n != ns || !valid_path(p)) {
        cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, format!("id: `{id}`: the mod's own, `{ns}:<name>`"));
        return;
    }
    if ok {
        definitions.push((kind, id, yog_data::binary::encode(&def.write())));
    }
}

/// A definition's file under `.yog-build/generated/`.
pub(super) fn definition_path(kind: &str, id: &str) -> String {
    format!("{kind}/{}.ydata", id.replace([':', '/'], "__"))
}

/// A loot modifier's file under `.yog-build/generated/`.
pub(super) fn loot_path(id: &str) -> String {
    format!("loot/{}.ydata", id.replace([':', '/'], "__"))
}


/// A `.ydef` tag: where it's written and its entries (`optional` ones
/// kept only if present). What it names was checked with the file's refs.
pub(super) fn tag_entries(t: &yog_formats::def::TagDef, ns: &str) -> Result<(String, BTreeMap<String, bool>), Vec<String>> {
    use yog_data::AnyRefOrTag;
    let id = yog_ids::qualify_in(ns, &t.id);
    let mut problems = Vec::new();
    let mut entries = BTreeMap::new();
    for (list, required) in [(&t.values, true), (&t.optional, false)] {
        for v in list {
            if v.kind() != t.of {
                problems.push(format!("`{v}` in a tag of {}", t.of));
                continue;
            }
            let (entry, required) = match v {
                AnyRefOrTag::One(r) => (yog_ids::qualify_in("minecraft", &r.id), required),
                // A tag only some versions or loaders have: where it isn't, the rest still counts.
                AnyRefOrTag::Tag(r) => {
                    let tag = yog_ids::qualify_in("minecraft", &r.id);
                    let everywhere = !matches!(refs::tag_hosts(yog_ids::registry_of(&t.of), &tag), Some(Some(_)));
                    (format!("#{tag}"), required && everywhere)
                }
            };
            entries.insert(entry, required);
        }
    }
    if problems.is_empty() {
        Ok((datapack::tag_file(&t.of, &id), entries))
    } else {
        Err(problems)
    }
}

/// A `.ydef` tree: the game's `minecraft:tree` configured feature.
pub(super) fn tree(t: &yog_formats::def::TreeDef, ns: &str) -> Result<Output, String> {
    use serde_json::json;
    let id = yog_ids::qualify_in(ns, &t.id);
    let (tns, tpath) = id.split_once(':').unwrap_or((ns, &id));
    if tns != ns || !valid_path(tpath) {
        return Err(format!("id: `{}`: the mod's own tree, `{ns}:<name>`", t.id));
    }
    // One block, or blocks by weight.
    let provider = |blocks: &[yog_formats::def::WeightedBlock]| match blocks {
        [one] => json!({ "type": "minecraft:simple_state_provider", "state": { "Name": one.block.id } }),
        many => json!({
            "type": "minecraft:weighted_state_provider",
            "entries": many.iter().map(|b| json!({ "data": { "Name": b.block.id }, "weight": b.weight })).collect::<Vec<_>>()
        }),
    };
    // The game's placers by their short name (`fancy`), a mod's by id.
    let placer = |shape: &str, what: &str| if shape.contains(':') { shape.to_string() } else { format!("minecraft:{shape}_{what}_placer") };
    if t.trunk.blocks.is_empty() || t.leaves.blocks.is_empty() {
        return Err("a tree needs trunk and leaves blocks".into());
    }
    let mut foliage = json!({ "type": placer(&t.leaves.shape, "foliage"), "radius": t.leaves.radius, "offset": t.leaves.offset });
    if let Some(h) = t.leaves.height {
        foliage["height"] = h.into();
    }
    let mut size = json!({ "type": "minecraft:two_layers_feature_size", "limit": 1, "lower_size": 0, "upper_size": 1 });
    if let Some(h) = t.min_clipped_height {
        size = json!({ "type": "minecraft:two_layers_feature_size", "limit": 0, "lower_size": 0, "upper_size": 0, "min_clipped_height": h });
    }
    let dirt = t.dirt.as_ref().map_or("minecraft:dirt", |b| b.id.as_str());
    let dirt_block = json!({ "type": "minecraft:simple_state_provider", "state": { "Name": dirt } });
    let below_trunk = if t.force_dirt {
        dirt_block.clone()
    } else {
        json!({ "type": "minecraft:rule_based_state_provider", "rules": [{
            "if_true": { "type": "minecraft:not", "predicate": { "type": "minecraft:matching_block_tag", "tag": "minecraft:cannot_replace_below_tree_trunk" } },
            "then": dirt_block,
        }] })
    };
    let v = json!({
        "type": "minecraft:tree",
        "config": {
            "trunk_provider": provider(&t.trunk.blocks),
            "trunk_placer": { "type": placer(&t.trunk.shape, "trunk"), "base_height": t.trunk.base_height, "height_rand_a": t.trunk.random_a, "height_rand_b": t.trunk.random_b },
            "foliage_provider": provider(&t.leaves.blocks),
            "foliage_placer": foliage,
            "dirt_provider": dirt_block,
            // 26.x: what goes under the trunk — the dirt, but not over what can't be replaced (unless forced).
            "below_trunk_provider": below_trunk,
            "minimum_size": size,
            "decorators": [],
            "ignore_vines": t.ignore_vines,
            "force_dirt": t.force_dirt
        }
    });
    Ok((format!("data/{tns}/worldgen/configured_feature/{tpath}.json"), pretty(&v)))
}

/// A `.ydef` damage type: its `damage_type/` file, and the damage type
/// tags it joins (path, its id).
pub(super) fn damage_type(d: &yog_formats::def::DamageTypeDef, ns: &str) -> Result<(Output, Vec<(String, String)>), String> {
    let id = yog_ids::qualify_in(ns, &d.id);
    let (dns, dpath) = id.split_once(':').unwrap_or((ns, &id));
    if dns != ns || !valid_path(dpath) {
        return Err(format!("id: `{}`: the mod's own damage type, `{ns}:<name>`", d.id));
    }
    let mut v = serde_json::json!({
        "message_id": d.message_id.clone().unwrap_or_else(|| format!("{dns}.{dpath}")),
        "scaling": d.scaling,
        "exhaustion": d.exhaustion,
    });
    if let Some(e) = &d.effects {
        v["effects"] = Value::String(e.clone());
    }
    if let Some(m) = &d.death_message_type {
        v["death_message_type"] = Value::String(m.clone());
    }
    let tags = d
        .tags
        .iter()
        .map(|t| (datapack::tag_file(yog_ids::kind::DAMAGE_TYPE, t), id.clone()))
        .collect();
    Ok(((format!("data/{dns}/damage_type/{dpath}.json"), pretty(&v)), tags))
}

/// A dimension as the runtime and the hosts read it (Yog Data, binary):
/// its type, biome, flags, the generator that makes its chunks — its
/// `terrain`'s (the runtime's own, under the dimension's id) or the mod's.
pub(super) fn dimension(d: yog_formats::def::DimensionDef, ns: &str) -> (String, Vec<u8>) {
    use yog_data::{Map, Schema, Value as V};
    let id = if d.id.contains(':') { d.id.clone() } else { format!("{ns}:{}", d.id) };
    let mut m = Map::new();
    m.insert("id", V::from(id.as_str()));
    m.insert("dimension_type", d.dimension_type.write());
    m.insert("biome", V::from(d.biome.map_or("minecraft:plains".to_string(), |b| b.id).as_str()));
    m.insert("structures", V::Bool(d.structures));
    m.insert("features", V::Bool(d.features));
    m.insert("mob_spawning", V::Bool(d.mob_spawning));
    if let Some(t) = &d.terrain {
        m.insert("generator_type", V::from(id.as_str()));
        m.insert("terrain", t.write());
    } else if let Some(g) = &d.generator {
        m.insert("generator_type", V::from(if g.contains(':') { g.clone() } else { format!("{ns}:{g}") }.as_str()));
    }
    (id, yog_data::binary::encode(&V::Map(m)))
}

/// An ore: its configured and placed features (the same on every version),
/// the biome modifiers Forge and NeoForge read (each its own), and the
/// biome feature the build registers for Fabric's host.
pub(super) fn ore(o: yog_formats::def::OreDef, ns: &str) -> OreOut {
    use serde_json::json;
    let p = o.id.split_once(':').map_or(o.id.as_str(), |(_, p)| p).to_string();
    let id = format!("{ns}:{p}");
    let targets: Vec<Value> = o
        .targets
        .iter()
        .map(|t| {
            let target = match t.replaces.strip_prefix('#') {
                Some(tag) => json!({ "predicate_type": "minecraft:tag_match", "tag": tag }),
                None => json!({ "predicate_type": "minecraft:block_match", "block": t.replaces }),
            };
            json!({ "target": target, "state": { "Name": t.block.id } })
        })
        .collect();
    let configured = json!({
        "type": "minecraft:ore",
        "config": { "size": o.size, "discard_chance_on_air_exposure": o.air_exposure, "targets": targets }
    });
    let shape = match o.height.shape {
        yog_formats::def::HeightShape::Uniform => "minecraft:uniform",
        yog_formats::def::HeightShape::Triangle => "minecraft:trapezoid",
    };
    let placed = json!({
        "feature": id,
        "placement": [
            { "type": "minecraft:count", "count": o.per_chunk },
            { "type": "minecraft:in_square" },
            { "type": "minecraft:height_range", "height": { "type": shape, "min_inclusive": { "absolute": o.height.min }, "max_inclusive": { "absolute": o.height.max } } },
            { "type": "minecraft:biome" }
        ]
    });
    let biomes = o.biomes.map(|b| b.id).unwrap_or_else(|| "minecraft:is_overworld".into());
    let modifier = |kind: &str| json!({ "type": kind, "biomes": format!("#{biomes}"), "features": id, "step": "underground_ores" });
    let mut files: Vec<Output> = Vec::new();
    let mut put = |path: String, v: Value| files.push((path, pretty(&v)));
    put(format!("data/{ns}/worldgen/configured_feature/{p}.json"), configured);
    put(format!("data/{ns}/worldgen/placed_feature/{p}.json"), placed);
    for (dir, kind) in [("when/forge/any", "forge:add_features"), ("when/neoforge/1.20", "forge:add_features")] {
        put(format!("{dir}/data/{ns}/forge/biome_modifier/{p}.json"), modifier(kind));
    }
    put(format!("when/neoforge/1.21/data/{ns}/neoforge/biome_modifier/{p}.json"), modifier("neoforge:add_features"));
    (files, (id, biomes, "underground_ores".into()))
}

/// Reads the `.ydef`s: ladders, levels placed on them (`above`, in any
/// order of files) and sound groups.
pub(super) fn read_defs(defs: &[(String, String)], mod_ns: Option<&str>, errors: &mut Vec<String>) -> Defs {
    let mut order: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut sound_groups: Vec<yog_formats::def::SoundGroup> = Vec::new();
    let game = yog_registry::ToolLadder::game();
    order.insert(game.ladder.clone(), game.ids().map(str::to_string).collect());
    let ns = mod_ns.unwrap_or("minecraft");
    let full = |id: &str| if id.contains(':') { id.to_string() } else { format!("{ns}:{id}") };
    // (file, line, id, ladder, above)
    let mut levels: Vec<(String, usize, String, String, Option<String>)> = Vec::new();
    let mut tools: Vec<(String, yog_formats::def::ToolMaterialDef)> = Vec::new();
    let mut armor_materials: Vec<yog_formats::def::ArmorMaterial> = Vec::new();
    let repair = |r: &yog_data::ItemIdOrTag| match r {
        yog_data::OneOrTag::One(i) => i.id.clone(),
        yog_data::OneOrTag::Tag(t) => format!("#{}", t.id),
    };
    for (file, text) in defs {
        let doc = match yog_data::text::parse(text) {
            Ok(d) => d,
            Err(e) => {
                errors.push(format!("{file}:{}: {}", e.at.line, e.message));
                continue;
            }
        };
        match yog_data::schema::from_doc::<yog_formats::def::Def>(text, &doc) {
            Ok(r) => match r.value {
                yog_formats::def::Def::Ladder { id } => {
                    if order.insert(full(&id), Vec::new()).is_some() {
                        errors.push(format!("{file}:1: ladder {} is defined twice", full(&id)));
                    }
                }
                yog_formats::def::Def::Level { id, ladder, above } => levels.push((file.clone(), doc.locate(text, &At::root().key("id")).line as usize, full(&id), ladder, above)),
                yog_formats::def::Def::SoundGroup(mut g) => {
                    // Short ids — the group's and its sounds' — are the mod's.
                    for s in [&mut g.id, &mut g.break_sound, &mut g.step, &mut g.place, &mut g.hit, &mut g.fall] {
                        *s = full(s);
                    }
                    if yog_formats::def::SoundGroup::built_in(&g.id).is_some() {
                        errors.push(format!("{file}:1: sound group {} is the game's", g.id));
                    } else if sound_groups.iter().any(|x| x.id == g.id) {
                        errors.push(format!("{file}:1: sound group {} is defined twice", g.id));
                    } else {
                        sound_groups.push(g);
                    }
                }
                // Read with the recipes (`compile_layout`).
                yog_formats::def::Def::RecipeKind(_) | yog_formats::def::Def::Ore(_) | yog_formats::def::Def::Dimension(_) | yog_formats::def::Def::Table(_) | yog_formats::def::Def::Tag(_) | yog_formats::def::Def::DamageType(_) | yog_formats::def::Def::Tree(_) | yog_formats::def::Def::LootModifier(_) | yog_formats::def::Def::LootTable(_) | yog_formats::def::Def::Attribute(_) | yog_formats::def::Def::Effect(_) | yog_formats::def::Def::Advancement(_) => {}
                // Its level is placed on the ladder below.
                yog_formats::def::Def::ToolMaterial(mut m) => {
                    m.id = full(&m.id);
                    tools.push((file.clone(), m));
                }
                yog_formats::def::Def::ArmorMaterial(m) => {
                    let id = full(&m.id);
                    if yog_formats::def::ArmorMaterial::built_in(&id).is_some() || armor_materials.iter().any(|x| x.id == id) {
                        errors.push(format!("{file}:1: armor material {id} is the game's or defined twice"));
                        continue;
                    }
                    armor_materials.push(yog_formats::def::ArmorMaterial {
                        texture: m.texture.as_deref().map(full).unwrap_or_else(|| id.clone()),
                        equip_sound: full(&m.equip_sound),
                        durability: m.durability,
                        defense: m.defense,
                        enchantability: m.enchantability,
                        toughness: m.toughness,
                        knockback_resistance: m.knockback_resistance,
                        repair: repair(&m.repair),
                        id,
                    });
                }
            },
            Err(es) => {
                for e in es.0 {
                    let l = e.at.map_or(1, |a| a.line as usize);
                    errors.push(if e.path.is_root() { format!("{file}:{l}: {}", e.message) } else { format!("{file}:{l}: {}: {}", e.path, e.message) });
                }
            }
        }
    }
    // Place each level once what it's above is placed.
    loop {
        let before = levels.len();
        levels.retain(|(file, line, id, ladder, above)| {
            let Some(list) = order.get_mut(ladder) else {
                errors.push(format!("{file}:{line}: level {id}: no ladder {ladder}"));
                return false;
            };
            let at = match above {
                None => 0,
                Some(a) => match list.iter().position(|x| x == a) {
                    Some(i) => i + 1,
                    None => return true, // later
                },
            };
            if list.contains(id) {
                errors.push(format!("{file}:{line}: level {id} is defined twice"));
            } else {
                list.insert(at, id.clone());
            }
            false
        });
        if levels.is_empty() || levels.len() == before {
            break;
        }
    }
    for (file, line, id, ladder, above) in levels {
        let above = above.unwrap_or_default();
        errors.push(format!("{file}:{line}: level {id}: above {above}, which isn't a level of {ladder} (or the levels go round in a circle)"));
    }
    // Tool materials: what their level is on each version — 1.20's level
    // number and 1.21's tag of what they can't mine, as blocks of their
    // level are tagged (`tool_level_tags`: the weakest of the game's levels
    // at or above it).
    let ladder = order.get(&yog_registry::ToolLadder::game().ladder).cloned().unwrap_or_default();
    let mut tool_materials: Vec<yog_formats::def::ToolMaterial> = Vec::new();
    for (file, m) in tools {
        if yog_formats::def::ToolMaterial::built_in(&m.id).is_some() || tool_materials.iter().any(|x| x.id == m.id) {
            errors.push(format!("{file}:1: tool material {} is the game's or defined twice", m.id));
            continue;
        }
        let Some(at) = ladder.iter().position(|l| l == &m.level) else {
            errors.push(format!("{file}:1: level: {} isn't a level of the tool ladder", m.level));
            continue;
        };
        let game = yog_registry::ToolLadder::game();
        let (level_index, incorrect) = match game_level_at(&ladder, at) {
            Some(i) => (i as u32, game.levels[i].incorrect.clone()),
            // Above the game's levels: a tag of its own (the build writes it).
            None => {
                let (n, p) = yog_ids::split(&m.level);
                (game.levels.len() as u32, yog_ids::join(n, &format!("incorrect_for_{p}_tool")))
            }
        };
        tool_materials.push(yog_formats::def::ToolMaterial {
            id: m.id,
            durability: m.durability,
            speed: m.speed,
            attack_damage: m.attack_damage,
            enchantability: m.enchantability,
            level: m.level,
            repair: repair(&m.repair),
            level_index,
            incorrect,
        });
    }
    Defs { order, sound_groups, tool_materials, armor_materials, biome_features: Vec::new(), dimensions: Vec::new(), tables: Vec::new(), loot_modifiers: Vec::new(), definitions: Vec::new(), advancements: Vec::new() }
}

/// The tags that make block `id` need tool level `level` on every loader and
/// version: those of the weakest of the game's levels at or above it
/// (`yog_registry::ToolLadder`), or of a level above them all.
pub(super) fn tool_level_tags(id: &str, level: &str, ladders: &Defs, out: &mut Compiled, err: &mut dyn FnMut(String), warn: &mut dyn FnMut(String)) {
    let game = yog_registry::ToolLadder::game();
    let Some((ladder, at)) = ladders.level(level) else { return };
    if ladder != game.ladder {
        err(format!("{level} is a level of {ladder}, not of the tool ladder ({})", game.ladder));
        return;
    }
    let needs = match game_level_at(&ladders.order[&game.ladder], at) {
        Some(l) => &game.levels[l].needs,
        None => {
            warn(game.above.caveat.clone());
            &game.above.needs
        }
    };
    for t in needs {
        out.tags.entry(datapack::overlay(&t.when, &datapack::tag_file(yog_ids::kind::BLOCK, &t.tag))).or_default().insert(id.to_string(), true);
    }
}

/// The index of the weakest of the game's tool levels at or above place `at`
/// on the tool ladder `tools` (with the mods' levels in it); `None` above them all.
pub(super) fn game_level_at(tools: &[String], at: usize) -> Option<usize> {
    let game = yog_registry::ToolLadder::game();
    game.levels.iter().position(|l| tools.iter().position(|x| *x == l.id).is_some_and(|p| p >= at))
}
