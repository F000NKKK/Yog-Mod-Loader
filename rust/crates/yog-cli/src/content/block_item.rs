//! One `.yblock` / `.yitem`: its properties, model, states, drops, tags and
//! texts checked and turned into the game's files.

use yog_ids::Format;
use yog_data::Path as At;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use crate::content::BlockProps;
use crate::content::Compiled;
use crate::content::Ctx;
use crate::content::Entry;
use crate::content::ItemProps;
use crate::content::Lang;
use crate::content::Loc;
use crate::content::TAG_REF;
use crate::content::datapack;
use crate::content::model::kind_models;
use crate::content::model::model;
use crate::content::model::turn;
use crate::content::model::variant;
use crate::content::num_value;
use crate::content::pretty;
use crate::content::source::float;
use crate::content::valid_id;
use crate::content::valid_namespace;
use crate::content::valid_path;
use crate::yres;
use std::fmt::Write;
use crate::content::model::state_properties;


/// A file's id: as written, else the file's name; the mod's namespace by default.
pub(super) fn file_id(cx: &mut Ctx, line: usize, id: Option<&str>, path: &Path, mod_ns: Option<&str>) -> Option<(String, String)> {
    let raw = id.map(str::to_string).unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    let (ns, p) = match raw.split_once(':') {
        Some((ns, p)) => (ns.to_string(), p.to_string()),
        None => match mod_ns {
            Some(ns) => (ns.to_string(), raw.clone()),
            None => {
                cx.err(line, "the mod id isn't a valid Minecraft namespace (a-z, 0-9, `_`, `-`, `.`), so give the namespace: id: \"mynamespace:name\"");
                return None;
            }
        },
    };
    if !valid_namespace(&ns) || !valid_path(&p) {
        cx.err(line, format!("id `{ns}:{p}` isn't a valid Minecraft id — lower case a-z, 0-9, `_`, `-`, `.` (and `/` in the path)"));
        return None;
    }
    Some((ns, p))
}

/// Checks one `.yitem` or `.yblock` (`content::files`) and writes what the
/// game reads: models, block states, item definitions, loot, lang, tags.
pub(super) fn compile_file(
    cx: &mut Ctx,
    text: &str,
    kind: Format,
    path: &Path,
    mod_ns: Option<&str>,
    out: &mut Compiled,
    generated: &mut BTreeMap<String, String>,
) -> Option<Entry> {
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let loc_fn = |p: &At| doc.locate(text, p).line as usize;
    let loc: Loc = &loc_fn;
    let root = At::root();
    struct Parts {
        format: Option<u32>,
        id: Option<String>,
        name: Option<yog_data::Text>,
        tooltip: Option<yog_data::Text>,
        register: Option<yog_formats::content::Register>,
        model: Option<yog_formats::content::Model>,
        item_model: Option<yog_formats::content::Model>,
        block: Option<yog_formats::content::BlockProps>,
        states: Option<yog_formats::content::States>,
        item: Option<yog_formats::content::ItemProps>,
        /// A block's item form.
        block_item: Option<yog_formats::content::BlockItemProps>,
        requires: Vec<String>,
        tags: Vec<String>,
        /// One of the game's kinds of block.
        kind: Option<String>,
    }
    let (parts, refs, tag_refs) = match kind {
        Format::Item => match yog_data::schema::from_doc::<yog_formats::content::ItemFile>(text, &doc) {
            Ok(r) => {
                let f = r.value;
                (Parts { format: f.format, id: f.id, name: f.name, tooltip: f.tooltip, register: f.register, model: f.model, item_model: None, block: None, states: None, item: f.item, block_item: None, requires: f.requires, tags: f.tags, kind: None }, r.refs, r.tags)
            }
            Err(e) => {
                cx.schema_errors(e);
                return None;
            }
        },
        Format::Block => match yog_data::schema::from_doc::<yog_formats::content::BlockFile>(text, &doc) {
            Ok(r) => {
                let f = r.value;
                (Parts { format: f.format, id: f.id, name: f.name, tooltip: f.tooltip, register: f.register, model: Some(f.model), item_model: f.item_model, block: f.block, states: f.states, item: None, block_item: f.item, requires: f.requires, tags: f.tags, kind: f.kind }, r.refs, r.tags)
            }
            Err(e) => {
                cx.schema_errors(e);
                return None;
            }
        },
        _ => unreachable!("only blocks and items get here"),
    };
    // What it names (drops) is checked once every file is read.
    for f in &refs {
        cx.deferred.push((cx.file.clone(), loc(&f.path), f.path.to_string(), f.value.clone()));
    }
    for f in &tag_refs {
        cx.deferred.push((cx.file.clone(), loc(&f.path), f.path.to_string(), yog_data::Ref::new(format!("{TAG_REF}{}", f.value.kind), f.value.id.clone())));
    }
    // Its tags (checked like the rest): a block's are block or item tags.
    for (i, tag) in parts.tags.iter().enumerate() {
        let at = root.key("tags").index(i);
        let of = if kind == Format::Block { "block|item" } else { "item" };
        cx.deferred.push((cx.file.clone(), loc(&at), at.to_string(), yog_data::Ref::new(format!("{TAG_REF}{of}"), yog_ids::qualify_in(mod_ns.unwrap_or("minecraft"), tag.trim_start_matches('#')))));
    }
    if let Some(f) = parts.format
        && f != 1
    {
        cx.err_at(loc, &root.key("format"), "format must be 1 (this yog understands format 1)");
    }
    let (ns, p) = file_id(cx, loc(&root.key("id")), parts.id.as_deref(), path, mod_ns)?;
    cx.ns = ns.clone();
    let id = format!("{ns}:{p}");
    let auto = parts.register.unwrap_or_default() == yog_formats::content::Register::Auto;

    let potion_only = parts.item.as_ref().is_some_and(|it| it.potion.is_some());
    // Name and tooltip → lang, under the keys the game looks up.
    let key = format!("{}.{ns}.{}", kind.noun(), p.replace('/', "."));
    for (field, text, target) in (!potion_only)
        .then_some([("name", &parts.name, key.clone()), ("tooltip", &parts.tooltip, format!("{key}.desc"))])
        .into_iter()
        .flatten()
    {
        let Some(t) = text else { continue };
        let from = match t {
            yog_data::Text::Literal(s) => {
                out.lang.entry((ns.clone(), yres::NEUTRAL_LOCALE.to_string())).or_default().insert(target, s.clone());
                continue;
            }
            yog_data::Text::Lang(k) => k.clone(),
            yog_data::Text::NameOf(r) => {
                let (rns, rp) = r.id.split_once(':').unwrap_or(("minecraft", &r.id));
                format!("{}.{rns}.{}", r.kind, rp.replace('/', "."))
            }
            yog_data::Text::Res(r) => match cx.refs.texts.get(&format!("{}.{}", r.set, r.key)) {
                Some((k, _)) => k.clone(),
                None => {
                    cx.err_at(loc, &root.key(field), format!("`@{}.{}`: no text `{}.{}` in resources/*.yres", r.set, r.key, r.set, r.key));
                    continue;
                }
            },
        };
        if from != target {
            out.lang_aliases.push((from, target));
        }
    }

    let mut emit = |cx: &mut Ctx, rel: String, value: &Value| {
        let full = format!("assets/{ns}/{rel}");
        if cx.root.join(&full).is_file() {
            cx.err(1, format!("{full} is written by this file — delete the hand-written one (or bring it in with `yog import model`)"));
        }
        if let Some(other) = generated.insert(full.clone(), cx.file.clone()) {
            cx.err(1, format!("{full} is also written by {other}"));
        }
        out.outputs.push((full, pretty(value)));
    };

    let potion = parts.item.as_ref().and_then(|it| it.potion.as_ref()).map(|p| yog_registry::PotionDef {
        id: id.clone(),
        name: p.name.clone(),
        effects: p.effects.clone(),
        brewed: Vec::new(),
    });
    let mut entry = Entry {
        kind,
        id: id.clone(),
        file: cx.file.clone(),
        auto,
        requires: parts.requires.clone(),
        block: BlockProps::default(),
        item: ItemProps::default(),
        potion,
    };

    // Its tags: a block's block tag, and the item tag of the same name for its item form.
    for tag in &parts.tags {
        let tag = yog_ids::qualify_in(&cx.ns, tag.trim_start_matches('#'));
        if kind == Format::Block {
            out.tags.entry(datapack::tag_file(yog_ids::kind::BLOCK, &tag)).or_default().insert(id.clone(), true);
        }
        out.tags.entry(datapack::tag_file(yog_ids::kind::ITEM, &tag)).or_default().insert(id.clone(), true);
    }

    // A block's item form: its stack, fire and fuel.
    if let Some(it) = &parts.block_item {
        entry.item.stack_size = it.stack_size;
        entry.item.fire_resistant = it.fire_resistant;
        entry.item.fuel_ticks = it.fuel_ticks.unwrap_or(0);
    }

    if kind == Format::Item {
        if let Some(it) = &parts.item {
            let props = &mut entry.item;
            props.stack_size = it.stack_size;
            props.durability = it.durability.unwrap_or(0);
            props.fire_resistant = it.fire_resistant;
            props.fuel_ticks = it.fuel_ticks.unwrap_or(0);
            props.food = it.food.as_ref().map(|f| (f.nutrition, f.saturation, f.always_edible));
            props.tool = it.tool.as_ref().map(|t| {
                let (d, s) = t.attack();
                (t.kind.clone(), t.material.clone(), d, s)
            });
            props.armor = it.armor.as_ref().map(|a| (yog_formats::content::word(&a.slot), a.material.clone()));
            props.wearable = it.wearable.as_ref().map(yog_formats::content::word);
            props.hangs = it.hangs.as_ref().map(|h| (h.entity.clone(), h.size));
            if it.hangs.as_ref().and_then(|h| h.size).is_some_and(|[w, h]| w == 0 || h == 0) {
                cx.err_at(loc, &root.key("item").key("hangs").key("size"), "a hanging thing is at least a block wide and high");
            }
            if props.wearable.is_some() && props.armor.is_some() {
                cx.err_at(loc, &root.key("item").key("wearable"), "armor is worn already — leave wearable out");
            }
            if props.tool.is_some() && props.armor.is_some() {
                cx.err_at(loc, &root.key("item").key("armor"), "an item is a tool or armor, not both");
            }
            if props.durability > 0 && props.stack_size.is_some_and(|s| s > 1) {
                cx.warn_at(loc, &root.key("item").key("stack_size"), "ignored: an item with durability stacks to 1");
            }
        }
        if entry.potion.is_some() {
            if let Some(item_model) = parts.model.as_ref() {
                let m = model(cx, loc, &root.key("model"), item_model, true)?;
                emit(cx, format!("models/item/{p}.json"), &m.json);
            }
            return Some(entry);
        }
        let Some(item_model) = parts.model.as_ref() else {
            cx.err_at(loc, &root.key("model"), "an item without potion effects needs a model");
            return None;
        };
        let m = model(cx, loc, &root.key("model"), item_model, true)?;
        emit(cx, format!("models/item/{p}.json"), &m.json);
        emit(cx, format!("items/{p}.json"), &json!({ "model": { "type": "minecraft:model", "model": format!("{ns}:item/{p}") } }));
        return Some(entry);
    }

    // A block.
    let b = parts.block.clone().unwrap_or_default();
    let bat = root.key("block");
    let kind_of = parts.kind.as_deref().and_then(yog_formats::content::BlockKind::built_in);
    let props = &mut entry.block;
    props.hardness = b.hardness;
    props.resistance = b.resistance;
    props.sound = b.sound.as_ref().map(yog_formats::content::word);
    props.light = b.light.unwrap_or(0);
    props.tools = b.tool.iter().map(|t| t.0.clone()).collect();
    props.tool_level = b.tool_level.clone().map(|l| (l, loc(&bat.key("tool_level"))));
    props.requires_tool = b.requires_tool;
    props.no_collision = !b.collision.or(kind_of.and_then(|k| k.collision)).unwrap_or(true);
    props.no_occlusion = !b.occludes.or(kind_of.and_then(|k| k.occludes)).unwrap_or(true);
    props.slipperiness = b.slipperiness;
    props.redstone = b.redstone;
    props.accepts_items = b.accepts_items;
    props.connect_groups = b.connect_groups.clone();
    let shape_auto = matches!(b.shape, None | Some(yog_formats::content::BlockShape::Auto));
    if let Some(yog_formats::content::BlockShape::Box(bx)) = b.shape {
        props.shape = Some(bx);
    }
    if let Some(inv) = &b.inventory {
        if valid_id(inv) && inv.contains(':') {
            props.inventory = Some(inv.clone());
            let at = (cx.file.clone(), loc(&bat.key("inventory")), inv.clone());
            cx.inventories.push(at);
        } else {
            cx.err_at(loc, &bat.key("inventory"), format!("`{inv}`: the id of an inventory screen — a .yui <Screen id=\"mymod:name\"> with <Slot>s"));
        }
    }
    if props.tool_level.is_some() && props.tools.is_empty() {
        cx.warn_at(loc, &bat, "tool_level without tool — any tool of that level mines it");
    }
    // A kind of the game's: what it's made of and its set; nothing of Yog's own blocks.
    if let Some(k) = kind_of {
        props.kind = k.name.clone();
        props.base = b.base.clone();
        props.set = b.set.clone();
        props.press_ticks = b.press_ticks;
        props.mobs_only = b.triggered_by == Some(yog_formats::content::TriggeredBy::Mobs);
        let own = [
            ("shape", b.shape.is_some()),
            ("connect_groups", !b.connect_groups.is_empty()),
            ("inventory", b.inventory.is_some()),
            ("redstone", b.redstone),
            ("powered", b.powered.is_some()),
            ("comparator", b.comparator.is_some()),
            ("accepts_items", b.accepts_items),
        ];
        for (field, _) in own.iter().filter(|(_, given)| *given) {
            cx.err_at(loc, &bat.key(field), format!("{field} doesn't go with kind {} (the game's block)", k.name));
        }
        if parts.states.is_some() {
            cx.err_at(loc, &root.key("states"), format!("kind {} has the game's states — leave states out", k.name));
        }
        if k.takes("base") && b.base.is_none() {
            cx.err_at(loc, &bat, format!("kind {} is made from a block — base: \"@block:mymod:planks\"", k.name));
        }
    }
    let kind_fields = [("base", b.base.is_some()), ("set", b.set.is_some()), ("press_ticks", b.press_ticks.is_some()), ("triggered_by", b.triggered_by.is_some())];
    for (field, _) in kind_fields.iter().filter(|(_, given)| *given) {
        match kind_of {
            Some(k) if k.takes(field) => {}
            Some(k) => cx.err_at(loc, &bat.key(field), format!("kind {} doesn't take {field}", k.name)),
            None => cx.err_at(loc, &bat.key(field), "only a block of a kind takes it (kind: \"stairs\", \"door\"…)"),
        }
    }
    let drops = match &b.drops {
        None | Some(yog_formats::content::Drops::Itself) => match kind_of {
            Some(k) => kind_drops(&id, k),
            None => drops_table(&id, None),
        },
        Some(yog_formats::content::Drops::Nothing) => Value::Null,
        Some(yog_formats::content::Drops::Item(d)) => drops_table(d.item.as_deref().unwrap_or(&id), d.count),
    };
    let render_layer = b.render_layer.as_ref().map(yog_formats::content::word);

    // Its models and states: a kind's, or its own.
    let item_parent = if let Some(k) = kind_of {
        kind_models(cx, loc, k, parts.model.as_ref().expect("blocks always have a model"), b.base.as_deref(), render_layer.as_deref(), &ns, &p, &mut emit)
    } else {
        let mut m = model(cx, loc, &root.key("model"), parts.model.as_ref().expect("blocks always have a model"), false)?;
        if let (Some(layer), Value::Object(o)) = (&render_layer, &mut m.json) {
            // Forge/NeoForge read it from the model; Fabric ignores it.
            o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
        }
        if shape_auto {
            entry.block.shape = m
                .bounds
                .filter(|b| *b != [0.0, 0.0, 0.0, 16.0, 16.0, 16.0])
                .map(|b| [b[0].max(0.0), b[1].max(0.0), b[2].max(0.0), b[3].min(16.0), b[4].min(16.0), b[5].min(16.0)]);
        }
        let main = format!("{ns}:block/{p}");
        emit(cx, format!("models/block/{p}.json"), &m.json);

        // Block states.
        let mut rotations: Vec<Value> = Vec::new();
        let mut arm: Option<(String, bool)> = None;
        if let Some(s) = &parts.states {
            let sat = root.key("states");
            if let Some(rs) = &s.rotations {
                if rs.is_empty() {
                    cx.err_at(loc, &sat.key("rotations"), "a non-empty list of { x: 0, y: 90, weight: 1, uvlock: false }");
                }
                for (i, r) in rs.iter().enumerate() {
                    let here = sat.key("rotations").index(i);
                    let mut angle = |k: &str, v: Option<u32>| match v {
                        None => 0.0,
                        Some(n @ (0 | 90 | 180 | 270)) => n as f64,
                        Some(_) => {
                            cx.err_at(loc, &here.key(k), "must be 0, 90, 180 or 270");
                            0.0
                        }
                    };
                    let (x, y) = (angle("x", r.x), angle("y", r.y));
                    rotations.push(variant(&main, x, y, r.uvlock, r.weight));
                }
            }
            if let Some(c) = &s.connect {
                if let Some(am) = model(cx, loc, &sat.key("connect").key("arm"), &c.arm, false) {
                    let mut json = am.json;
                    if let (Some(layer), Value::Object(o)) = (&render_layer, &mut json) {
                        o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
                    }
                    emit(cx, format!("models/block/{p}_arm.json"), &json);
                    arm = Some((format!("{ns}:block/{p}_arm"), c.uvlock));
                }
                entry.block.connects = true;
                if entry.block.connect_groups.is_empty() {
                    entry.block.connect_groups.push(id.clone());
                }
            }
            if s.facing.is_some() || s.attach || s.waterlogged || !s.properties.0.is_empty() {
                if s.rotations.is_some() || s.connect.is_some() {
                    cx.err_at(loc, &sat, "facing, attach, waterlogged and properties can't go with rotations or connect");
                }
                let b = &mut entry.block;
                use yog_formats::content::{Facing, Placement};
                if s.attach {
                    if s.facing == Some(Facing::All) {
                        cx.err_at(loc, &sat.key("facing"), "an attached block turns four ways — leave facing out");
                    }
                    b.attach = true;
                    b.facing = "horizontal".into();
                } else if let Some(f) = s.facing {
                    b.facing = if f == Facing::All { "all" } else { "horizontal" }.into();
                }
                if !b.facing.is_empty() {
                    b.placement = if s.placement == Some(Placement::Away) { "away" } else { "toward" }.into();
                } else if s.placement.is_some() {
                    cx.err_at(loc, &sat.key("placement"), "placement needs facing (or attach)");
                }
                b.waterlogged = s.waterlogged;
                for (name, values) in &s.properties.0 {
                    let here = sat.key("properties").key(name);
                    let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
                    if !word(name) {
                        cx.err_at(loc, &here, format!("`{name}`: a property's name is lowercase letters, digits and _"));
                    } else if ["facing", "face", "waterlogged"].contains(&name.as_str()) {
                        cx.err_at(loc, &here, format!("`{name}` comes from facing, attach or waterlogged — name yours otherwise"));
                    }
                    let mut seen = std::collections::HashSet::new();
                    if values.len() < 2 || !values.iter().all(|v| word(v) && seen.insert(v.as_str())) {
                        cx.err_at(loc, &here, "two or more different values, lowercase letters, digits and _");
                    }
                    b.properties.push((name.clone(), values.clone()));
                }
            } else if s.placement.is_some() || !s.models.0.is_empty() || !s.variants.0.is_empty() {
                cx.err_at(loc, &sat, "placement, models and variants need facing, attach, waterlogged or properties");
            }
        }
        // A block's own properties with every value — facing, face and
        // waterlogged as the host makes them.
        let states: Vec<(String, Vec<String>)> = {
            let b = &entry.block;
            let words = |ws: &[&str]| ws.iter().map(|w| w.to_string()).collect::<Vec<_>>();
            let mut all = Vec::new();
            match b.facing.as_str() {
                "all" => all.push(("facing".to_string(), words(&["north", "east", "south", "west", "up", "down"]))),
                "horizontal" => all.push(("facing".to_string(), words(&["north", "east", "south", "west"]))),
                _ => {}
            }
            if b.attach {
                all.push(("face".to_string(), words(&["floor", "wall", "ceiling"])));
            }
            if b.waterlogged {
                all.push(("waterlogged".to_string(), words(&["false", "true"])));
            }
            all.extend(b.properties.iter().cloned());
            all.sort_by(|a, b| a.0.cmp(&b.0));
            all
        };
        // Yes/no properties the host drives.
        let yes_no = |name: &str| states.iter().any(|(n, v)| n == name && *v == ["false", "true"]);
        for (key, value) in [("powered", &b.powered), ("comparator", &b.comparator)] {
            if let Some(name) = value {
                if yes_no(name) {
                    if key == "powered" {
                        entry.block.powered = Some(name.clone());
                    } else {
                        entry.block.comparator = Some(name.clone());
                    }
                } else {
                    cx.err_at(loc, &bat.key(key), format!("`{name}`: a yes/no property of states.properties ([\"false\", \"true\"])"));
                }
            }
        }
        // Its other models, and which state shows which.
        let mut rules: Vec<(Vec<(String, String)>, Vec<String>)> = Vec::new();
        if let Some(s) = &parts.states {
            let sat = root.key("states");
            let mut named = std::collections::HashMap::new();
            for (name, m) in &s.models.0 {
                if let Some(mut m) = model(cx, loc, &sat.key("models").key(name), m, false) {
                    if let (Some(layer), Value::Object(o)) = (&render_layer, &mut m.json) {
                        o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
                    }
                    emit(cx, format!("models/block/{p}/{name}.json"), &m.json);
                    named.insert(name.clone(), format!("{ns}:block/{p}/{name}"));
                }
            }
            for (key, names) in &s.variants.0 {
                let here = sat.key("variants").key(key);
                let mut when = Vec::new();
                for pair in key.split(',').filter(|p| !p.is_empty()) {
                    let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                    if states.iter().any(|(n, vs)| n == k && vs.iter().any(|x| x == v)) {
                        when.push((k.to_string(), v.to_string()));
                    } else {
                        cx.err_at(loc, &here, format!("`{pair}`: no such property and value"));
                    }
                }
                let mut models = Vec::new();
                for name in &names.0 {
                    match named.get(name) {
                        Some(m) => models.push(m.clone()),
                        None => cx.err_at(loc, &here, format!("`{name}`: not in states.models")),
                    }
                }
                if names.0.is_empty() {
                    cx.err_at(loc, &here, "a model name, or a list of them");
                } else if !models.is_empty() {
                    rules.push((when, models));
                }
            }
        }
        let base: Value = match rotations.len() {
            0 => variant(&main, 0.0, 0.0, false, None),
            1 => rotations.remove(0),
            _ => Value::Array(rotations),
        };
        let uvlock = parts.states.as_ref().is_some_and(|s| s.uvlock);
        let blockstate = match &arm {
            // Every state: its model, turned by facing and face.
            None if !states.is_empty() => {
                let mut variants = Map::new();
                let mut combo: Vec<usize> = vec![0; states.len()];
                loop {
                    let at: Vec<(&str, &str)> = states.iter().zip(&combo).map(|((n, vs), &i)| (n.as_str(), vs[i].as_str())).collect();
                    let value = |k: &str| at.iter().find(|(n, _)| *n == k).map(|(_, v)| *v);
                    let models = rules
                        .iter()
                        .find(|(when, _)| when.iter().all(|(k, v)| value(k) == Some(v.as_str())))
                        .map_or(vec![main.clone()], |(_, m)| m.clone());
                    let (x, y) = turn(value("facing"), value("face"));
                    let key: Vec<String> = at.iter().map(|(n, v)| format!("{n}={v}")).collect();
                    let picks: Vec<Value> = models.iter().map(|m| variant(m, x, y, uvlock, None)).collect();
                    let v = if picks.len() == 1 { picks.into_iter().next().unwrap() } else { Value::Array(picks) };
                    variants.insert(key.join(","), v);
                    // The next state, the last property fastest.
                    let mut i = states.len();
                    loop {
                        if i == 0 {
                            break;
                        }
                        i -= 1;
                        combo[i] += 1;
                        if combo[i] < states[i].1.len() {
                            break;
                        }
                        combo[i] = 0;
                    }
                    if combo.iter().all(|&c| c == 0) {
                        break;
                    }
                }
                json!({ "variants": variants })
            }
            None => json!({ "variants": { "": base } }),
            Some((arm, uvlock)) => {
                let mut parts = vec![json!({ "apply": base })];
                for (dir, x, y) in [("north", 0.0, 0.0), ("east", 0.0, 90.0), ("south", 0.0, 180.0), ("west", 0.0, 270.0), ("up", 270.0, 0.0), ("down", 90.0, 0.0)] {
                    parts.push(json!({ "when": { dir: "true" }, "apply": variant(arm, x, y, *uvlock, None) }));
                }
                json!({ "multipart": parts })
            }
        };
        emit(cx, format!("blockstates/{p}.json"), &blockstate);
        Some(main)
    };

    // The item form: the block model, or its own.
    let item_model = match (&parts.item_model, item_parent) {
        (None, Some(parent)) => Some(json!({ "parent": parent })),
        (None, None) => {
            cx.err_at(loc, &root, format!("kind {} has no item model of its own — give item_model", parts.kind.as_deref().unwrap_or_default()));
            None
        }
        (Some(im), _) => model(cx, loc, &root.key("item_model"), im, true).map(|m| m.json),
    };
    if let Some(im) = item_model {
        emit(cx, format!("models/item/{p}.json"), &im);
    }
    emit(cx, format!("items/{p}.json"), &json!({ "model": { "type": "minecraft:model", "model": format!("{ns}:item/{p}") } }));

    if !drops.is_null() {
        let full = format!("data/{ns}/loot_table/blocks/{p}.json");
        let plural = format!("data/{ns}/loot_tables/blocks/{p}.json");
        if cx.root.join(&full).is_file() || cx.root.join(&plural).is_file() {
            cx.err(1, format!("{full} is written by this file (block.drops) — delete the hand-written one, or set drops: \"none\""));
        }
        out.outputs.push((full, pretty(&drops)));
    }
    // Tools and tiers are block tags: the block joins each.
    // (The tool level's tags come once every `.ydef` is read: `tool_level_tags`.)
    for tag in &entry.block.tools {
        out.tags.entry(datapack::tag_file(yog_ids::kind::BLOCK, tag)).or_default().insert(id.clone(), true);
    }
    Some(entry)
}

/// The loot table dropping `item` (`count` of it), when it survives the explosion.
pub(super) fn drops_table(item: &str, count: Option<yog_formats::content::DropCount>) -> Value {
    let mut entry = json!({ "type": "minecraft:item", "name": item });
    if let Some(c) = count {
        let count = match c {
            yog_formats::content::DropCount::Exactly(n) => num_value(n),
            yog_formats::content::DropCount::Between(min, max) => json!({ "type": "minecraft:uniform", "min": num_value(min), "max": num_value(max) }),
        };
        entry["functions"] = json!([{ "function": "minecraft:set_count", "count": count }]);
    }
    json!({
        "type": "minecraft:block",
        "pools": [{
            "rolls": 1,
            "entries": [entry],
            "conditions": [{ "condition": "minecraft:survives_explosion" }]
        }]
    })
}

/// What a block of a game's kind drops: itself — only in its `drops_when`
/// states (a door's bottom half), two in its `drops_two_when` (a double slab).
pub(super) fn kind_drops(id: &str, k: &yog_formats::content::BlockKind) -> Value {
    let mut table = drops_table(id, None);
    let when = |state: &str| json!({ "condition": "minecraft:block_state_property", "block": id, "properties": state_properties(state) });
    let pool = &mut table["pools"][0];
    if let Some(state) = &k.drops_when
        && let Some(conditions) = pool["conditions"].as_array_mut()
    {
        conditions.insert(0, when(state));
    }
    if let Some(state) = &k.drops_two_when {
        // As the game's slab: two when double, and explosions take their share.
        pool.as_object_mut().map(|o| o.remove("conditions"));
        pool["entries"][0]["functions"] = json!([
            { "function": "minecraft:set_count", "count": 2, "conditions": [when(state)] },
            { "function": "minecraft:explosion_decay" }
        ]);
    }
    table
}

// ── Recipes ──────────────────────────────────────────────────────────────────

/// The builder calls (`.max_stack(16)`…) for an item's properties; empty
/// when it has none of its own.
pub(super) fn item_props(p: &ItemProps) -> String {
    let mut b = String::new();
    if let Some(s) = p.stack_size {
        let _ = write!(b, "\n            .max_stack({s})");
    }
    if p.durability > 0 {
        let _ = write!(b, "\n            .max_damage({})", p.durability);
    }
    if p.fire_resistant {
        b.push_str("\n            .fire_resistant()");
    }
    if p.fuel_ticks > 0 {
        let _ = write!(b, "\n            .fuel({})", p.fuel_ticks);
    }
    if let Some((kind, material, d, s)) = &p.tool {
        let _ = write!(b, "\n            .tool({kind:?}, {material:?}, {d:?}, {s:?})");
    }
    if let Some(slot) = &p.wearable {
        let _ = write!(b, "\n            .wearable({slot:?})");
    }
    if let Some((entity, size)) = &p.hangs {
        let _ = write!(b, "\n            .hangs({entity:?}, {size:?})");
    }
    if let Some((slot, material)) = &p.armor {
        let _ = write!(b, "\n            .armor({slot:?}, {material:?})");
    }
    if let Some((n, s, always)) = p.food {
        let _ = write!(
            b,
            "\n            .food(::yog_api::FoodDef::new({n}, {}){})",
            float(s),
            if always { ".can_always_eat()" } else { "" }
        );
    }
    b
}

/// The translation key a text names (`@Lang.…`, `@lang:…`, an item's name),
/// or for words, `key` with the words put in the mod's lang.
pub(super) fn text_key(cx: &mut Ctx, lang: &mut Lang, t: &yog_data::Text, ns: &str, key: String) -> Result<String, String> {
    Ok(match t {
        yog_data::Text::Literal(words) => {
            lang.entry((ns.to_string(), yres::NEUTRAL_LOCALE.to_string())).or_default().insert(key.clone(), words.clone());
            key
        }
        yog_data::Text::Lang(k) => k.clone(),
        yog_data::Text::NameOf(r) => {
            let (rns, rp) = r.id.split_once(':').unwrap_or(("minecraft", &r.id));
            format!("{}.{rns}.{}", r.kind, rp.replace('/', "."))
        }
        yog_data::Text::Res(r) => cx
            .refs
            .texts
            .get(&format!("{}.{}", r.set, r.key))
            .map(|(k, _)| k.clone())
            .ok_or_else(|| format!("`@{}.{}`: no text `{}.{}` in resources/*.yres", r.set, r.key, r.set, r.key))?,
    })
}

/// Ids of the `.yui` screens under `dir` that lay out an inventory (have
/// `<Slot>`s or a `<PlayerInventory>`).
pub(super) fn inventory_screens(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            out.extend(inventory_screens(&path));
            continue;
        }
        if !Format::Ui.is(&path) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if !text.contains("<Slot") && !text.contains("<PlayerInventory") {
            continue;
        }
        if let Some(screen) = text.find("<Screen")
            && let Some(at) = text[screen..].find(" id=\"")
        {
            let rest = &text[screen + at + 5..];
            if let Some(end) = rest.find('"') {
                out.insert(rest[..end].to_string());
            }
        }
    }
    out
}
