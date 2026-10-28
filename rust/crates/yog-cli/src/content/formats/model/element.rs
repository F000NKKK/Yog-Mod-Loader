//! One element of a model (a box) as the game's JSON: its corners, rotation
//! and faces.

use serde_json::{Map, Value};
use yog_formats::content::vocab::RIGHT_ANGLES;
use yog_formats::content::{Element, ElementRotation, Face, SLOT_MARK, word};
use yog_ids::model as m;
use yog_ids::units::BLOCK_PIXELS;

use crate::content::formats::model::nums;
use crate::content::num_value;

/// One element → its JSON (what's wrong with it is yog_formats::rules' to report).
pub(super) fn element(el: &Element) -> Value {
    let mut out = Map::new();
    out.insert(m::FROM.into(), nums(&el.from));
    out.insert(m::TO.into(), nums(&el.to));
    if let Some(r) = &el.rotation {
        out.insert(m::ROTATION.into(), rotation(r));
    }
    if el.shade == Some(false) {
        out.insert(m::SHADE.into(), Value::Bool(false));
    }
    let mut faces = Map::new();
    for (dir, face) in el.faces.iter().flat_map(|f| f.iter()) {
        faces.insert(dir.into(), self::face(face));
    }
    out.insert(m::FACES.into(), Value::Object(faces));
    Value::Object(out)
}

/// An element's rotation: about a point (the block's middle by default) on
/// one axis.
fn rotation(r: &ElementRotation) -> Value {
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

/// A face: its texture slot, part of it, turn, the side that hides it, its tint.
fn face(face: &Face) -> Value {
    let mut fo = Map::new();
    let slot = face
        .texture
        .strip_prefix(SLOT_MARK)
        .unwrap_or(&face.texture);
    fo.insert(
        m::TEXTURE.into(),
        Value::String(format!("{SLOT_MARK}{slot}")),
    );
    if let Some(uv) = face.uv {
        fo.insert(m::UV.into(), nums(&uv));
    }
    if let Some(n) = face
        .rotation
        .filter(|n| *n != 0 && RIGHT_ANGLES.contains(n))
    {
        fo.insert(m::ROTATION.into(), Value::from(n));
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
