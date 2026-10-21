//! Block and item models: Yog's model to the game's JSON (elements, faces,
//! rotations), block-state variants, the game's kinds of block's models.

mod element;
mod kind;
mod state;

use serde_json::{Map, Value};
use yog_data::Path as At;
use yog_data::field;
use yog_formats::content::Model as YogModel;
use yog_ids::model as m;
use yog_ids::units::BLOCK_PIXELS;

use crate::content::ctx::{Ctx, Loc};
use crate::content::vocab::{TEMPLATES, Template, template};
use crate::content::{num_value, valid_id};

pub(super) use kind::{KindBlock, kind_models, with_render_type};
pub(super) use state::{state_properties, turn, variant};

/// A checked model: Minecraft's JSON, its bounding box.
pub(in crate::content) struct Model {
    pub(in crate::content) json: Value,
    /// Elements' bounding box in pixels, when it has elements.
    pub(in crate::content) bounds: Option<[f64; 6]>,
}

pub(super) fn nums(ns: &[f64]) -> Value {
    Value::Array(ns.iter().map(|n| num_value(*n)).collect())
}

/// What a model's parent is: one of Yog's templates, or a model by id.
enum Parent<'t> {
    None,
    Template(&'t Template),
    Raw,
}

/// A model's JSON being built: what it says so far, its parent, its
/// textures by slot and the slots in order.
struct Building<'t> {
    out: Map<String, Value>,
    parent: Parent<'t>,
    textures: Map<String, Value>,
    slots: Vec<String>,
}

/// A model → Minecraft's model JSON. `item` = an item model (sprite templates allowed).
pub(super) fn model(
    cx: &mut Ctx,
    loc: Loc,
    at: &At,
    model: &YogModel,
    item: bool,
) -> Option<Model> {
    let (parent, out) = parent(cx, loc, at, model, item)?;
    let (textures, slots) = textures(cx, loc, at, model);
    let mut b = Building {
        out,
        parent,
        textures,
        slots,
    };
    let tpl = match b.parent {
        Parent::Template(t) => Some(t),
        _ => None,
    };
    let raw_parent = matches!(b.parent, Parent::Raw);
    if let Some(t) = tpl {
        check_template_slots(cx, loc, at, t, &b);
    }
    let bounds = elements(cx, loc, at, model, &mut b);
    let Building {
        mut out,
        mut textures,
        slots,
        ..
    } = b;
    // `particle` — the break/run particles. Default: the first texture —
    // also under a raw parent when the model brings its own elements (a
    // parent like `minecraft:block/block` names no particle texture).
    if !textures.contains_key(m::PARTICLE)
        && !tpl.is_some_and(|t| t.has_particle)
        && (!raw_parent || model.elements.is_some())
        && let Some(first) = slots.first()
    {
        let v = textures[first].clone();
        textures.insert(m::PARTICLE.into(), v);
    }
    if !textures.is_empty() {
        out.insert(m::TEXTURES.into(), Value::Object(textures));
    }
    if let Some(d) = &model.display {
        out.insert(m::DISPLAY.into(), display(d));
    }
    if let Some(b) = model.ambient_occlusion {
        out.insert(m::AMBIENT_OCCLUSION.into(), Value::Bool(b));
    }
    if let Some(g) = &model.gui_light {
        out.insert(
            m::GUI_LIGHT.into(),
            Value::String(yog_formats::content::word(g)),
        );
    }
    Some(Model {
        json: Value::Object(out),
        bounds,
    })
}

/// The model's parent written: a template's model, or a model id as it is.
/// None when it's neither.
fn parent<'t>(
    cx: &mut Ctx,
    loc: Loc,
    at: &At,
    model: &YogModel,
    item: bool,
) -> Option<(Parent<'t>, Map<String, Value>)> {
    let mut out = Map::new();
    let Some(name) = &model.parent else {
        return Some((Parent::None, out));
    };
    let here = at.key(field!(YogModel, parent));
    if let Some(t) = template(name) {
        if t.sprite && !item {
            cx.err_at(
                loc,
                &here,
                format!("{name:?} is an item sprite — block models use cube_all, cube_column, ..."),
            );
        }
        out.insert(m::PARENT.into(), Value::String(t.parent.into()));
        return Some((Parent::Template(t), out));
    }
    if name.contains(yog_ids::sep::NAMESPACE) && valid_id(name) {
        out.insert(m::PARENT.into(), Value::String(name.clone()));
        return Some((Parent::Raw, out));
    }
    let names: Vec<&str> = TEMPLATES
        .iter()
        .filter(|t| item || !t.sprite)
        .map(|t| t.name)
        .collect();
    cx.err_at(
        loc,
        &here,
        format!(
            "{name:?} isn't a template ({}) or a model id like `minecraft:block/stairs`",
            names.join(", ")
        ),
    );
    None
}

/// Slot → texture (as the game's model says it), and the slots in order.
fn textures(
    cx: &mut Ctx,
    loc: Loc,
    at: &At,
    model: &YogModel,
) -> (Map<String, Value>, Vec<String>) {
    let mut slots: Vec<String> = Vec::new();
    let mut textures = Map::new();
    for (slot, tex) in &model.textures.0 {
        let here = at.key(field!(YogModel, textures)).key(slot);
        if slot.is_empty() || slot.starts_with(yog_formats::content::SLOT_MARK) {
            cx.err_at(
                loc,
                &here,
                "slot names are plain words (`all`, `side`, `0`)",
            );
            continue;
        }
        if let Some(t) = cx.texture(loc, &here, tex) {
            textures.insert(slot.clone(), Value::String(t));
            slots.push(slot.clone());
        }
    }
    (textures, slots)
}

/// A template's slots given: a sprite's layers in a row from `layer0`, a
/// block template's every slot.
fn check_template_slots(cx: &mut Ctx, loc: Loc, at: &At, t: &Template, b: &Building) {
    let (textures, slots) = (&b.textures, &b.slots);
    let layer = |n: usize| format!("{}{n}", m::LAYER);
    if !t.sprite {
        for slot in t.slots.iter().filter(|s| !textures.contains_key(**s)) {
            cx.err_at(
                loc,
                at,
                format!("parent {:?} needs textures.{slot}", t.name),
            );
        }
        return;
    }
    if !textures.contains_key(m::LAYER0) {
        cx.err_at(
            loc,
            at,
            format!("parent {:?} needs textures.{}", t.name, m::LAYER0),
        );
    }
    for slot in slots {
        let here = at.key(field!(YogModel, textures)).key(slot);
        match slot
            .strip_prefix(m::LAYER)
            .and_then(|n| n.parse::<usize>().ok())
        {
            Some(n) if n == 0 || textures.contains_key(&layer(n - 1)) => {}
            Some(_) => cx.err_at(loc, &here, "without the layer before it"),
            None if slot == m::PARTICLE => {}
            None => cx.warn_at(
                loc,
                &here,
                "isn't used by an item sprite (layer0, layer1, ...)",
            ),
        }
    }
}

/// The model's own elements written; their bounding box in pixels.
fn elements(
    cx: &mut Ctx,
    loc: Loc,
    at: &At,
    model: &YogModel,
    b: &mut Building,
) -> Option<[f64; 6]> {
    let Some(els) = &model.elements else {
        if matches!(b.parent, Parent::None) {
            cx.err_at(
                loc,
                at,
                "no parent and no elements — give a template (parent: \"cube_all\") or elements",
            );
        }
        return None;
    };
    let here = at.key(field!(YogModel, elements));
    if let Parent::Template(t) = b.parent {
        cx.err_at(
            loc,
            &here,
            format!(
                "elements replace the shape of parent {:?} — remove one of them",
                t.name
            ),
        );
    }
    let full = f64::from(BLOCK_PIXELS);
    let mut bounds: Option<[f64; 6]> = None;
    let mut written = Vec::new();
    for (i, el) in els.iter().enumerate() {
        let Some(json) = element::element(cx, loc, &here.index(i), el, b) else {
            continue;
        };
        let bx = bounds.get_or_insert([full, full, full, 0.0, 0.0, 0.0]);
        for a in 0..3 {
            bx[a] = bx[a].min(el.from[a].min(el.to[a]));
            bx[a + 3] = bx[a + 3].max(el.from[a].max(el.to[a]));
        }
        written.push(json);
    }
    b.out.insert(m::ELEMENTS.into(), Value::Array(written));
    // Its own shape and no parent: vanilla's block parent gives it how
    // it's held and shown in a slot — without one it's a flat face-on square.
    if model.parent.is_none() {
        b.out
            .insert(m::PARENT.into(), Value::String(m::BLOCK_PARENT.into()));
    }
    bounds
}

/// How the model sits in each place it's shown.
fn display(d: &yog_formats::content::Display) -> Value {
    let mut display = Map::new();
    for (place, t) in d.iter() {
        let mut o = Map::new();
        for (key, v) in [
            (m::ROTATION, t.rotation),
            (m::TRANSLATION, t.translation),
            (m::SCALE, t.scale),
        ] {
            if let Some(v) = v {
                o.insert(key.into(), nums(&v));
            }
        }
        display.insert(place.into(), Value::Object(o));
    }
    Value::Object(display)
}
