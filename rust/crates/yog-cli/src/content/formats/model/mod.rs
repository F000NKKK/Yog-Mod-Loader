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

use crate::content::pipeline::ctx::{Ctx, Loc};
use crate::content::{num_value, valid_id};
use yog_formats::content::vocab::{Template, template};

pub(in crate::content) use kind::{KindBlock, kind_models, with_render_type};
pub(in crate::content) use state::{state_properties, turn, variant};

/// A checked model: Minecraft's JSON, its bounding box.
pub(in crate::content) struct Model {
    pub(in crate::content) json: Value,
    /// Elements' bounding box in pixels, when it has elements.
    pub(in crate::content) bounds: Option<[f64; 6]>,
}

pub(in crate::content) fn nums(ns: &[f64]) -> Value {
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
pub(in crate::content) fn model(
    cx: &mut Ctx,
    loc: Loc,
    at: &At,
    model: &YogModel,
) -> Option<Model> {
    let (parent, out) = parent(model)?;
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
    let bounds = elements(model, &mut b);
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
fn parent<'t>(model: &YogModel) -> Option<(Parent<'t>, Map<String, Value>)> {
    let mut out = Map::new();
    let Some(name) = &model.parent else {
        return Some((Parent::None, out));
    };
    // A parent that's neither (or a sprite for a block) is yog_formats::rules' to report.
    if let Some(t) = template(name) {
        out.insert(m::PARENT.into(), Value::String(t.parent.into()));
        return Some((Parent::Template(t), out));
    }
    if name.contains(yog_ids::sep::NAMESPACE) && valid_id(name) {
        out.insert(m::PARENT.into(), Value::String(name.clone()));
        return Some((Parent::Raw, out));
    }
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
            continue;
        }
        if let Some(t) = cx.texture(loc, &here, tex) {
            textures.insert(slot.clone(), Value::String(t));
            slots.push(slot.clone());
        }
    }
    (textures, slots)
}

/// The model's own elements written; their bounding box in pixels.
fn elements(model: &YogModel, b: &mut Building) -> Option<[f64; 6]> {
    let els = model.elements.as_ref()?;
    let full = f64::from(BLOCK_PIXELS);
    let mut bounds: Option<[f64; 6]> = None;
    let mut written = Vec::new();
    for el in els {
        let json = element::element(el);
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
