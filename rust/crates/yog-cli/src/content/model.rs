//! Block and item models: Yog's model to the game's JSON (elements, faces,
//! rotations), block-state variants, the game's kinds of block's models.

use yog_data::Path as At;
use serde_json::{Map, Value, json};
use crate::content::Ctx;
use crate::content::Loc;
use crate::content::Model;
use crate::content::TEMPLATES;
use crate::content::Template;
use crate::content::num_value;
use crate::content::template;
use crate::content::valid_id;


pub(super) fn nums(ns: &[f64]) -> Value {
    Value::Array(ns.iter().map(|n| num_value(*n)).collect())
}

/// A model → Minecraft's model JSON. `item` = an item model (sprite templates allowed).
pub(super) fn model(cx: &mut Ctx, loc: Loc, at: &At, m: &yog_formats::content::Model, item: bool) -> Option<Model> {
    let mut out = Map::new();
    let mut tpl: Option<&Template> = None;
    let mut raw_parent = false;
    if let Some(name) = &m.parent {
        if let Some(t) = template(name) {
            if t.sprite && !item {
                cx.err_at(loc, &at.key("parent"), format!("{name:?} is an item sprite — block models use cube_all, cube_column, ..."));
            }
            out.insert("parent".into(), Value::String(t.parent.into()));
            tpl = Some(t);
        } else if name.contains(':') && valid_id(name) {
            out.insert("parent".into(), Value::String(name.clone()));
            raw_parent = true;
        } else {
            let names: Vec<&str> = TEMPLATES.iter().filter(|t| item || !t.sprite).map(|t| t.name).collect();
            cx.err_at(loc, &at.key("parent"), format!("{name:?} isn't a template ({}) or a model id like `minecraft:block/stairs`", names.join(", ")));
            return None;
        }
    }

    // Textures.
    let mut slots: Vec<String> = Vec::new();
    let mut textures = Map::new();
    for (slot, tex) in &m.textures.0 {
        let here = at.key("textures").key(slot);
        if slot.is_empty() || slot.starts_with('#') {
            cx.err_at(loc, &here, "slot names are plain words (`all`, `side`, `0`)");
            continue;
        }
        if let Some(t) = cx.texture(loc, &here, tex) {
            textures.insert(slot.clone(), Value::String(t));
            slots.push(slot.clone());
        }
    }
    let defined = |slot: &str, textures: &Map<String, Value>| textures.contains_key(slot);
    if let Some(t) = tpl {
        if t.sprite {
            if !defined("layer0", &textures) {
                cx.err_at(loc, at, format!("parent {:?} needs textures.layer0", t.name));
            }
            for slot in &slots {
                let n = slot.strip_prefix("layer").and_then(|n| n.parse::<usize>().ok());
                match n {
                    Some(n) if n == 0 || defined(&format!("layer{}", n - 1), &textures) => {}
                    Some(_) => cx.err_at(loc, &at.key("textures").key(slot), "without the layer before it"),
                    None if slot == "particle" => {}
                    None => cx.warn_at(loc, &at.key("textures").key(slot), "isn't used by an item sprite (layer0, layer1, ...)"),
                }
            }
        } else {
            for slot in t.slots {
                if !defined(slot, &textures) {
                    cx.err_at(loc, at, format!("parent {:?} needs textures.{slot}", t.name));
                }
            }
        }
    }

    // Elements.
    let mut bounds: Option<[f64; 6]> = None;
    if let Some(els) = &m.elements {
        if let Some(t) = tpl {
            cx.err_at(loc, &at.key("elements"), format!("elements replace the shape of parent {:?} — remove one of them", t.name));
        }
        let mut elements = Vec::new();
        for (i, el) in els.iter().enumerate() {
            if let Some(json) = element(cx, loc, &at.key("elements").index(i), el, &textures, raw_parent) {
                let b = bounds.get_or_insert([16.0, 16.0, 16.0, 0.0, 0.0, 0.0]);
                for a in 0..3 {
                    b[a] = b[a].min(el.from[a].min(el.to[a]));
                    b[a + 3] = b[a + 3].max(el.from[a].max(el.to[a]));
                }
                elements.push(json);
            }
        }
        out.insert("elements".into(), Value::Array(elements));
        // Its own shape and no parent: vanilla's block parent gives it how
        // it's held and shown in a slot — without one it's a flat face-on square.
        if m.parent.is_none() {
            out.insert("parent".into(), Value::String("minecraft:block/block".into()));
        }
    } else if tpl.is_none() && !raw_parent {
        cx.err_at(loc, at, "no parent and no elements — give a template (parent: \"cube_all\") or elements");
    }

    // `particle` — the break/run particles. Default: the first texture —
    // also under a raw parent when the model brings its own elements (a
    // parent like `minecraft:block/block` names no particle texture).
    if !textures.contains_key("particle")
        && !tpl.is_some_and(|t| t.has_particle)
        && (!raw_parent || m.elements.is_some())
        && let Some(first) = slots.first()
    {
        let v = textures[first].clone();
        textures.insert("particle".into(), v);
    }
    if !textures.is_empty() {
        out.insert("textures".into(), Value::Object(textures));
    }
    if let Some(d) = &m.display {
        let mut display = Map::new();
        for (place, t) in d.iter() {
            let mut o = Map::new();
            for (key, v) in [("rotation", t.rotation), ("translation", t.translation), ("scale", t.scale)] {
                if let Some(v) = v {
                    o.insert(key.into(), nums(&v));
                }
            }
            display.insert(place.into(), Value::Object(o));
        }
        out.insert("display".into(), Value::Object(display));
    }
    if let Some(b) = m.ambient_occlusion {
        out.insert("ambientocclusion".into(), Value::Bool(b));
    }
    if let Some(g) = &m.gui_light {
        out.insert("gui_light".into(), Value::String(yog_formats::content::word(g)));
    }
    Some(Model { json: Value::Object(out), bounds })
}

/// One element → its JSON.
pub(super) fn element(cx: &mut Ctx, loc: Loc, at: &At, el: &yog_formats::content::Element, textures: &Map<String, Value>, raw_parent: bool) -> Option<Value> {
    let mut out = Map::new();
    out.insert("from".into(), nums(&el.from));
    out.insert("to".into(), nums(&el.to));
    if let Some(r) = &el.rotation {
        if (r.angle / 22.5).fract() != 0.0 {
            cx.err_at(loc, &at.key("rotation").key("angle"), format!("{} — Minecraft rotates elements by -45, -22.5, 0, 22.5 or 45 degrees", r.angle));
        }
        let mut rot = Map::new();
        rot.insert("origin".into(), nums(&r.origin.unwrap_or([8.0, 8.0, 8.0])));
        rot.insert("axis".into(), Value::String(yog_formats::content::word(&r.axis)));
        rot.insert("angle".into(), num_value(r.angle));
        if r.rescale {
            rot.insert("rescale".into(), Value::Bool(true));
        }
        out.insert("rotation".into(), Value::Object(rot));
    }
    if el.shade == Some(false) {
        out.insert("shade".into(), Value::Bool(false));
    }
    let mut faces = Map::new();
    match &el.faces {
        None => cx.warn_at(loc, at, "no faces — it's invisible"),
        Some(f) => {
            for (dir, face) in f.iter() {
                let here = at.key("faces").key(dir);
                let mut fo = Map::new();
                let slot = face.texture.strip_prefix('#').unwrap_or(&face.texture);
                if !textures.contains_key(slot) && !raw_parent {
                    cx.err_at(loc, &here.key("texture"), format!("`#{slot}`: no such slot in textures"));
                }
                fo.insert("texture".into(), Value::String(format!("#{slot}")));
                if let Some(uv) = face.uv {
                    fo.insert("uv".into(), nums(&uv));
                }
                match face.rotation {
                    None | Some(0) => {}
                    Some(n @ (90 | 180 | 270)) => {
                        fo.insert("rotation".into(), Value::from(n));
                    }
                    Some(_) => cx.err_at(loc, &here.key("rotation"), "must be 0, 90, 180 or 270"),
                }
                if let Some(c) = &face.cullface {
                    fo.insert("cullface".into(), Value::String(yog_formats::content::word(c)));
                }
                if let Some(t) = face.tint
                    && t >= 0
                {
                    fo.insert("tintindex".into(), Value::from(t));
                }
                faces.insert(dir.into(), Value::Object(fo));
            }
        }
    }
    out.insert("faces".into(), Value::Object(faces));
    Some(Value::Object(out))
}

/// The JSON of one block state: `{"model": ..., "x": .., "y": .., ...}`.
/// How a state turns its model (x, then y, in degrees): `facing` is where
/// its front (the model's north side) looks; `face`, where an attached
/// block (its model lying on the floor) hangs.
pub(super) fn turn(facing: Option<&str>, face: Option<&str>) -> (f64, f64) {
    let yaw = |f: &str| match f {
        "east" => 90.0,
        "south" => 180.0,
        "west" => 270.0,
        _ => 0.0,
    };
    match (face, facing) {
        (Some("wall"), Some(f)) => (90.0, yaw(f)),
        (Some("ceiling"), Some(f)) => (180.0, (yaw(f) + 180.0) % 360.0),
        (_, Some("up")) => (270.0, 0.0),
        (_, Some("down")) => (90.0, 0.0),
        (_, Some(f)) => (0.0, yaw(f)),
        _ => (0.0, 0.0),
    }
}

pub(super) fn variant(model: &str, x: f64, y: f64, uvlock: bool, weight: Option<f64>) -> Value {
    let mut v = Map::new();
    v.insert("model".into(), Value::String(model.into()));
    if x != 0.0 {
        v.insert("x".into(), num_value(x));
    }
    if y != 0.0 {
        v.insert("y".into(), num_value(y));
    }
    if uvlock {
        v.insert("uvlock".into(), Value::Bool(true));
    }
    if let Some(w) = weight {
        v.insert("weight".into(), num_value(w));
    }
    Value::Object(v)
}

/// A block of one of the game's kinds: the kind's template models with the
/// file's textures, its states as the kind's, and the model its item shows
/// (none: the file gives it).
#[allow(clippy::too_many_arguments)]
pub(super) fn kind_models(
    cx: &mut Ctx,
    loc: Loc,
    k: &yog_formats::content::BlockKind,
    m: &yog_formats::content::Model,
    base: Option<&str>,
    render_layer: Option<&str>,
    ns: &str,
    p: &str,
    emit: &mut dyn FnMut(&mut Ctx, String, &Value),
) -> Option<String> {
    let at = At::root().key("model");
    if m.parent.is_some() || m.elements.is_some() || m.display.is_some() || m.ambient_occlusion.is_some() || m.gui_light.is_some() {
        cx.err_at(loc, &at, format!("kind {} has the game's models — give only textures ({})", k.name, k.slots.join(", ")));
    }
    let mut textures = Map::new();
    for (slot, tex) in &m.textures.0 {
        let here = at.key("textures").key(slot);
        if slot != "particle" && !k.slots.contains(slot) {
            cx.warn_at(loc, &here, format!("kind {} has no slot `{slot}` ({})", k.name, k.slots.join(", ")));
        }
        if let Some(t) = cx.texture(loc, &here, tex) {
            textures.insert(slot.clone(), Value::String(t));
        }
    }
    for slot in k.slots.iter().filter(|s| !textures.contains_key(*s)) {
        cx.err_at(loc, &at, format!("kind {} needs textures.{slot}", k.name));
    }
    // `""` is the block's own model, `base` its base block's, another name `<block>_<name>`.
    let path = |name: &str| if name.is_empty() { p.to_string() } else { format!("{p}_{name}") };
    let id = |name: &str| match (name, base) {
        ("base", Some(b)) => {
            let (bns, bp) = b.split_once(':').unwrap_or(("minecraft", b));
            format!("{bns}:block/{bp}")
        }
        _ => format!("{ns}:block/{}", path(name)),
    };
    for (name, parent) in &k.models.0 {
        let mut json = json!({ "parent": parent, "textures": textures });
        if let (Some(layer), Value::Object(o)) = (render_layer, &mut json) {
            o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
        }
        emit(cx, yog_ids::model_file(&format!("{}{}", yog_ids::res::BLOCK_DIR, path(name))), &json);
    }
    let shown = |km: &yog_formats::content::KindModel| variant(&id(&km.model), km.x.unwrap_or(0) as f64, km.y.unwrap_or(0) as f64, km.uvlock, None);
    let blockstate = if k.multipart.is_empty() {
        let variants: Map<String, Value> = k.variants.0.iter().map(|(state, km)| (state.clone(), shown(km))).collect();
        json!({ "variants": variants })
    } else {
        let parts: Vec<Value> = k
            .multipart
            .iter()
            .map(|part| {
                let mut o = Map::new();
                o.insert("apply".into(), shown(&part.apply));
                if let Some(when) = &part.when {
                    o.insert("when".into(), Value::Object(state_properties(when)));
                }
                Value::Object(o)
            })
            .collect();
        json!({ "multipart": parts })
    };
    emit(cx, yog_ids::blockstate_file(p), &blockstate);
    k.item.as_deref().map(id)
}

/// `half=lower,open=true` → `{"half": "lower", "open": "true"}`.
pub(super) fn state_properties(state: &str) -> Map<String, Value> {
    state
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
        .collect()
}
