//! A model's element: faces to show, a rotation Minecraft takes, faces
//! turned at right angles and textured from the model's slots.

use yog_data::{Diagnostic, Ordered, Path, field};

use super::{error, warning};
use crate::content::vocab::{ELEMENT_ROTATION_MAX, ELEMENT_ROTATION_STEP, RIGHT_ANGLES};
use crate::content::{Element, ElementRotation, Face, SLOT_MARK, Texture};

/// Element `el`'s rules, it being at `at`; `textures` the model's slots,
/// `raw_parent` whether a model by id gives slots this file can't see.
pub(crate) fn element_rules(
    el: &Element,
    textures: &Ordered<Texture>,
    raw_parent: bool,
    at: Path,
    out: &mut Vec<Diagnostic>,
) {
    if let Some(r) = &el.rotation
        && (r.angle / ELEMENT_ROTATION_STEP).fract() != 0.0
    {
        out.push(error(
            at.key(field!(Element, rotation)).key(field!(ElementRotation, angle)),
            format!(
                "{} — Minecraft rotates elements by -{ELEMENT_ROTATION_MAX}, -{ELEMENT_ROTATION_STEP}, 0, {ELEMENT_ROTATION_STEP} or {ELEMENT_ROTATION_MAX} degrees",
                r.angle
            ),
        ));
    }
    let Some(faces) = &el.faces else {
        out.push(warning(at, "no faces — it's invisible"));
        return;
    };
    for (dir, face) in faces.iter() {
        face_rules(
            face,
            textures,
            raw_parent,
            at.key(field!(Element, faces)).key(dir),
            out,
        );
    }
}

fn face_rules(
    face: &Face,
    textures: &Ordered<Texture>,
    raw_parent: bool,
    at: Path,
    out: &mut Vec<Diagnostic>,
) {
    let slot = face
        .texture
        .strip_prefix(SLOT_MARK)
        .unwrap_or(&face.texture);
    if !raw_parent && !textures.0.iter().any(|(s, _)| s == slot) {
        out.push(error(
            at.key(field!(Face, texture)),
            format!("`{SLOT_MARK}{slot}`: no such slot in textures"),
        ));
    }
    if face.rotation.is_some_and(|n| !RIGHT_ANGLES.contains(&n)) {
        out.push(error(
            at.key(field!(Face, rotation)),
            "must be 0, 90, 180 or 270",
        ));
    }
}
