//! One element of a model (a box) as the game's JSON: its corners, rotation
//! and faces.

use serde_json::{Map, Value};
use yog_data::Path as At;
use yog_data::field;
use yog_formats::content::{Element, ElementRotation, Face, SLOT_MARK, word};
use yog_ids::model as m;
use yog_ids::units::BLOCK_PIXELS;

use super::{Building, Parent, nums};
use crate::content::ctx::{Ctx, Loc};
use crate::content::num_value;

/// The angles the game turns an element by are multiples of this (-45 to 45).
const ROTATION_STEP: f64 = 22.5;
const MAX_ROTATION: f64 = 2.0 * ROTATION_STEP;
/// The angles a face's texture turns by.
const FACE_ROTATIONS: [u32; 3] = [90, 180, 270];

/// One element → its JSON; `b`: the model's textures, and whether its
/// parent is a model by id (whose slots aren't known).
pub(super) fn element(
    cx: &mut Ctx,
    loc: Loc,
    at: &At,
    el: &Element,
    b: &Building,
) -> Option<Value> {
    let mut out = Map::new();
    out.insert(m::FROM.into(), nums(&el.from));
    out.insert(m::TO.into(), nums(&el.to));
    if let Some(r) = &el.rotation {
        out.insert(m::ROTATION.into(), rotation(cx, loc, at, r));
    }
    if el.shade == Some(false) {
        out.insert(m::SHADE.into(), Value::Bool(false));
    }
    let mut faces = Map::new();
    match &el.faces {
        None => cx.warn_at(loc, at, "no faces — it's invisible"),
        Some(f) => {
            for (dir, face) in f.iter() {
                let here = at.key(field!(Element, faces)).key(dir);
                faces.insert(dir.into(), self::face(cx, loc, &here, face, b));
            }
        }
    }
    out.insert(m::FACES.into(), Value::Object(faces));
    Some(Value::Object(out))
}

/// An element's rotation: about a point (the block's middle by default) on
/// one axis, by a multiple of [`ROTATION_STEP`].
fn rotation(cx: &mut Ctx, loc: Loc, at: &At, r: &ElementRotation) -> Value {
    if (r.angle / ROTATION_STEP).fract() != 0.0 {
        let here = at
            .key(field!(Element, rotation))
            .key(field!(ElementRotation, angle));
        cx.err_at(
            loc,
            &here,
            format!(
                "{} — Minecraft rotates elements by -{MAX_ROTATION}, -{ROTATION_STEP}, 0, {ROTATION_STEP} or {MAX_ROTATION} degrees",
                r.angle
            ),
        );
    }
    let middle = f64::from(BLOCK_PIXELS) / 2.0;
    let mut rot = Map::new();
    rot.insert(m::ORIGIN.into(), nums(&r.origin.unwrap_or([middle; 3])));
    rot.insert(m::AXIS.into(), Value::String(word(&r.axis)));
    rot.insert(m::ANGLE.into(), num_value(r.angle));
    if r.rescale {
        rot.insert(m::RESCALE.into(), Value::Bool(true));
    }
    Value::Object(rot)
}

/// A face: its texture slot (one of the model's, unless the parent's not
/// known), part of it, turn, the side that hides it, its tint.
fn face(cx: &mut Ctx, loc: Loc, here: &At, face: &Face, b: &Building) -> Value {
    let mut fo = Map::new();
    let slot = face
        .texture
        .strip_prefix(SLOT_MARK)
        .unwrap_or(&face.texture);
    if !b.textures.contains_key(slot) && !matches!(b.parent, Parent::Raw) {
        cx.err_at(
            loc,
            &here.key(field!(Face, texture)),
            format!("`{SLOT_MARK}{slot}`: no such slot in textures"),
        );
    }
    fo.insert(
        m::TEXTURE.into(),
        Value::String(format!("{SLOT_MARK}{slot}")),
    );
    if let Some(uv) = face.uv {
        fo.insert(m::UV.into(), nums(&uv));
    }
    match face.rotation {
        None | Some(0) => {}
        Some(n) if FACE_ROTATIONS.contains(&n) => {
            fo.insert(m::ROTATION.into(), Value::from(n));
        }
        Some(_) => cx.err_at(
            loc,
            &here.key(field!(Face, rotation)),
            "must be 0, 90, 180 or 270",
        ),
    }
    if let Some(c) = &face.cullface {
        fo.insert(m::CULLFACE.into(), Value::String(word(c)));
    }
    if let Some(t) = face.tint
        && t >= 0
    {
        fo.insert(m::TINTINDEX.into(), Value::from(t));
    }
    Value::Object(fo)
}
