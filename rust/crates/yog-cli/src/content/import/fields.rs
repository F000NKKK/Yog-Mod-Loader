//! The fields of the content files an import writes, as their formats name them.

use yog_data::field;
use yog_formats::content::{
    BlockFile, BlockProps, Element, ElementRotation, Face, Model, Rotation, States,
};

pub(super) const ID: &str = field!(BlockFile, id);
pub(super) const NAME: &str = field!(BlockFile, name);
pub(super) const MODEL: &str = field!(BlockFile, model);
pub(super) const ITEM_MODEL: &str = field!(BlockFile, item_model);
pub(super) const BLOCK: &str = field!(BlockFile, block);
pub(super) const STATES: &str = field!(BlockFile, states);
pub(super) const ROTATIONS: &str = field!(States, rotations);
pub(super) const RENDER_LAYER: &str = field!(BlockProps, render_layer);

pub(super) const PARENT: &str = field!(Model, parent);
pub(super) const TEXTURES: &str = field!(Model, textures);
pub(super) const ELEMENTS: &str = field!(Model, elements);
pub(super) const DISPLAY: &str = field!(Model, display);
pub(super) const AMBIENT_OCCLUSION: &str = field!(Model, ambient_occlusion);
pub(super) const GUI_LIGHT: &str = field!(Model, gui_light);

pub(super) const EL_NAME: &str = field!(Element, name);
pub(super) const FROM: &str = field!(Element, from);
pub(super) const TO: &str = field!(Element, to);
pub(super) const ROTATION: &str = field!(Element, rotation);
pub(super) const SHADE: &str = field!(Element, shade);
pub(super) const FACES: &str = field!(Element, faces);

pub(super) const AXIS: &str = field!(ElementRotation, axis);
pub(super) const ANGLE: &str = field!(ElementRotation, angle);
pub(super) const ORIGIN: &str = field!(ElementRotation, origin);
pub(super) const RESCALE: &str = field!(ElementRotation, rescale);

pub(super) const TEXTURE: &str = field!(Face, texture);
pub(super) const UV: &str = field!(Face, uv);
pub(super) const FACE_ROTATION: &str = field!(Face, rotation);
pub(super) const CULLFACE: &str = field!(Face, cullface);
pub(super) const TINT: &str = field!(Face, tint);

pub(super) const X: &str = field!(Rotation, x);
pub(super) const Y: &str = field!(Rotation, y);
pub(super) const WEIGHT: &str = field!(Rotation, weight);
pub(super) const UVLOCK: &str = field!(Rotation, uvlock);
