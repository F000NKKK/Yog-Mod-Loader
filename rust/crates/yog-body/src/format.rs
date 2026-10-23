//! The source formats: `.ymodel` (a skeleton of bones with cubes) and
//! `.yanim` (clips of keyframes). Java model space — pixels, y down, as
//! `CubeListBuilder` and `PartPose` (what Blockbench's "Modded Entity"
//! makes); angles in degrees.

use yog_data::{Ordered, Schema};

/// A `.ymodel`.
#[derive(Schema, Debug, Clone)]
pub struct ModelFile {
    /// The texture's id, written `@texture:<ns>:<path>` — a `.ytexture`
    /// of the mod (`@texture:mymod:entity/wings`) or the game's
    /// (`@texture:minecraft:entity/elytra`).
    #[data(with = "yog_data::with::RefTo<yog_data::kinds::Texture>")]
    pub texture: String,
    /// Shown instead of the texture: a video of the mod
    /// (`@video:mymod:screen`), playing while the model is drawn — its UVs
    /// over the video's frame as over the texture.
    #[data(with = "yog_data::with::Opt<yog_data::with::RefTo<yog_data::kinds::Video>>")]
    pub video: Option<String>,
    /// The texture's size in pixels, as its UVs count them.
    #[data(default = "default_texture_size")]
    pub texture_size: [u32; 2],
    /// The bones, parents before their children.
    pub bones: Vec<BoneDef>,
}

fn default_texture_size() -> [u32; 2] {
    [64, 32]
}

/// A bone: a pivot, a rest turn, cubes, and points to follow (locators).
#[derive(Schema, Debug, Clone)]
pub struct BoneDef {
    pub name: String,
    /// The bone it hangs on — one of this model's, or nothing (the anchor).
    pub parent: Option<String>,
    /// Where it turns about, in its parent's space (`PartPose` offset).
    #[data(default)]
    pub pivot: [f32; 3],
    /// Its rest turn, degrees.
    #[data(default)]
    pub rotation: [f32; 3],
    #[data(default)]
    pub cubes: Vec<CubeDef>,
    /// Named points in the bone's space (a camera's eye, a hand's grip).
    #[data(default)]
    pub locators: Ordered<[f32; 3]>,
}

/// A box (`addBox`), textured box-UV from `uv`.
#[derive(Schema, Debug, Clone)]
pub struct CubeDef {
    /// Its corner, in the bone's space.
    pub origin: [f32; 3],
    pub size: [f32; 3],
    /// Where its box-UV starts on the texture (`texOffs`).
    #[data(default)]
    pub uv: [f32; 2],
    /// Grown on every side by this much (`CubeDeformation`), without
    /// changing the UVs.
    #[data(default)]
    pub inflate: f32,
    /// Its UVs mirrored left to right (`mirror()`).
    #[data(default)]
    pub mirror: bool,
}

/// A `.yanim`: clips by name.
#[derive(Schema, Debug, Clone, Default)]
pub struct AnimFile(pub Ordered<ClipDef>);

/// What a clip does when it runs past its end.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Loop {
    /// Stops: back to rest.
    #[default]
    Once,
    /// Stays at its last frame.
    Hold,
    /// Starts over.
    Loop,
}

/// One animation.
#[derive(Schema, Debug, Clone)]
pub struct ClipDef {
    /// Seconds.
    pub length: f32,
    #[data(rename = "loop", default)]
    pub repeat: Loop,
    /// Each bone's tracks — this model's bones, or the vanilla model's
    /// parts (`head`, `body`, `right_arm`…).
    pub bones: Ordered<TracksDef>,
}

/// A bone's keyframes: time in seconds (`"0.25"`) → value.
#[derive(Schema, Debug, Clone, Default)]
pub struct TracksDef {
    /// Turn, degrees, on top of the rest turn.
    #[data(default)]
    pub rotation: Ordered<KeyDef>,
    /// Move, pixels.
    #[data(default)]
    pub position: Ordered<KeyDef>,
    /// Scale, a factor.
    #[data(default)]
    pub scale: Ordered<KeyDef>,
}

/// How a keyframe goes to the next one.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ease {
    #[default]
    Linear,
    /// Smooth through its neighbours (Catmull-Rom).
    Smooth,
    /// Jumps at the next keyframe.
    Step,
}

/// A keyframe: `[x, y, z]`, or `{ "value": [x, y, z], "ease": "smooth" }`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyDef {
    pub value: [f32; 3],
    pub ease: Ease,
}

#[derive(Schema)]
struct KeyObject {
    value: [f32; 3],
    #[data(default)]
    ease: Ease,
}

impl Schema for KeyDef {
    fn shape() -> yog_data::Shape {
        yog_data::Shape::Any
    }

    fn read(v: &yog_data::ValueRef<'_>, cx: &mut yog_data::Reader) -> Option<Self> {
        if v.as_list().is_some() {
            return <[f32; 3]>::read(v, cx).map(|value| KeyDef {
                value,
                ease: Ease::Linear,
            });
        }
        KeyObject::read(v, cx).map(|k| KeyDef {
            value: k.value,
            ease: k.ease,
        })
    }

    fn write(&self) -> yog_data::Value {
        if self.ease == Ease::Linear {
            return self.value.write();
        }
        KeyObject {
            value: self.value,
            ease: self.ease,
        }
        .write()
    }
}
