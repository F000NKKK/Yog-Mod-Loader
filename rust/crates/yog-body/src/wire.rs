//! What the host and a mod say each frame about one entity (see
//! `docs/design/yog-body.md`): the [`Frame`] the host sends, the [`Ops`]
//! the mod answers with. Both are Yog Data, carried by the bus's
//! `client/render/body` event; the hosts read and write them through the
//! records generated from them.

use yog_data::Schema;
use yog_data::with::Raw;
use yog_graphics::Color;
use yog_ids::time::TICKS_PER_SECOND as TICKS;

use crate::math::{Mat4, Vec3};
use crate::model::Quad;

/// Floats a vertex takes on the wire: x y z, u v, nx ny nz.
const VERTEX_FLOATS: usize = 8;
/// Vertices a quad has.
const QUAD_VERTICES: usize = 4;

/// One entity, this frame.
#[derive(Schema, Debug, Clone, Default, PartialEq)]
#[data(rename = "BodyFrame")]
pub struct Frame {
    pub uuid: String,
    /// `minecraft:player`, `minecraft:zombie`…
    pub entity_type: String,
    /// Between the last tick and the next (0–1).
    pub partial: f32,
    /// Ticks it has lived, with `partial`.
    pub age: f32,
    /// How far its legs have walked, and how fast (vanilla's limb swing).
    pub limb_swing: f32,
    pub limb_amount: f32,
    /// The head's turn from the body, and its pitch (degrees).
    pub head_yaw: f32,
    pub head_pitch: f32,
    /// Degrees.
    pub body_yaw: f32,
    pub pitch: f32,
    pub pos: [f64; 3],
    /// Blocks per tick.
    pub velocity: Vec3,
    /// `yog_core::query::entity_flags` bits.
    pub flags: u32,
    /// The world's time in ticks, with `partial` — the same on every
    /// client, so clips timed by it play alike for everyone.
    pub game_time: f64,
    /// What was captured last frame (see [`Ops::capture`]).
    #[data(default)]
    pub captures: Vec<Capture>,
    /// What the entity keeps by key (`entity::SetData`), as this client was told.
    #[data(default)]
    pub data: Vec<Kept>,
}

/// A value the entity keeps, by key.
#[derive(Schema, Debug, Clone, Default, PartialEq)]
#[data(rename = "BodyKept")]
pub struct Kept {
    pub key: String,
    #[data(with = "Raw")]
    pub value: Vec<u8>,
}

/// The posed geometry of an entity (entity space, see the design doc).
#[derive(Schema, Debug, Clone, Default, PartialEq)]
#[data(rename = "BodyCapture")]
pub struct Capture {
    /// `self`, or the entity type captured.
    pub source: String,
    pub meshes: Vec<Mesh>,
}

/// Quads with one texture: [`VERTEX_FLOATS`] floats a vertex, four vertices a quad.
#[derive(Schema, Debug, Clone, Default, PartialEq)]
#[data(rename = "BodyMesh")]
pub struct Mesh {
    pub texture: String,
    pub verts: Vec<f32>,
}

impl Mesh {
    pub fn new(texture: &str, quads: &[Quad]) -> Mesh {
        Mesh {
            texture: texture.to_string(),
            verts: verts(quads),
        }
    }

    /// Its quads (the whole ones).
    pub fn quads(&self) -> Vec<Quad> {
        self.verts
            .as_chunks::<{ VERTEX_FLOATS * QUAD_VERTICES }>()
            .0
            .iter()
            .map(|q| {
                let vertex = |i: usize| &q[i * VERTEX_FLOATS..(i + 1) * VERTEX_FLOATS];
                Quad {
                    pos: std::array::from_fn(|i| {
                        Vec3::new(vertex(i)[0], vertex(i)[1], vertex(i)[2])
                    }),
                    uv: std::array::from_fn(|i| [vertex(i)[3], vertex(i)[4]]),
                    normal: Vec3::new(vertex(0)[5], vertex(0)[6], vertex(0)[7]),
                }
            })
            .collect()
    }
}

/// `quads` as the wire's vertices.
fn verts(quads: &[Quad]) -> Vec<f32> {
    let mut out = Vec::with_capacity(quads.len() * QUAD_VERTICES * VERTEX_FLOATS);
    for q in quads {
        for i in 0..QUAD_VERTICES {
            out.extend([q.pos[i].x, q.pos[i].y, q.pos[i].z, q.uv[i][0], q.uv[i][1]]);
            out.extend([q.normal.x, q.normal.y, q.normal.z]);
        }
    }
    out
}

/// Where a mesh's vertices are.
#[derive(Debug, Clone, PartialEq)]
pub enum Anchor {
    /// Blocks, y up, feet at the origin, after the body's yaw.
    Entity,
    /// The vanilla model's space: pixels, y down.
    Model,
    /// A vanilla part's space (pixels), wherever the part is posed.
    Bone(String),
    /// A block's space: pixels 0–16, y up, as block models (block bodies).
    Block,
}

/// [`Anchor`] as the wire names it; a bone's name goes beside it.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[data(rename = "BodyAnchor")]
pub enum AnchorKind {
    #[default]
    Entity,
    Model,
    Bone,
    Block,
}

/// How a mesh looks: how it's drawn, whether it shines, its tint (alpha
/// counts in [`Mode::Translucent`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    pub mode: Mode,
    /// The enchantment shine.
    pub glint: bool,
    pub tint: Color,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            mode: Mode::Cutout,
            glint: false,
            tint: Color::WHITE,
        }
    }
}

/// How a mesh is drawn.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[data(rename = "BodyMode")]
pub enum Mode {
    /// Opaque, transparent pixels cut out, both sides.
    #[default]
    Cutout,
    /// Blended by its alpha.
    Translucent,
    /// Glows: full bright, added.
    Emissive,
}

/// What to change about a vanilla part.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Default)]
#[data(rename = "BodyBoneOp")]
pub struct BoneOp {
    /// Radians; replaces vanilla's turn if `set_rotation`, else adds.
    pub rotation: Vec3,
    pub set_rotation: bool,
    /// Pixels; replaces vanilla's offset if `set_position`, else adds.
    pub position: Vec3,
    pub set_position: bool,
    pub scale: Option<Vec3>,
    /// `Some(false)` hides it, `Some(true)` shows it.
    pub visible: Option<bool>,
}

/// A vanilla part to change, and how.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
#[data(rename = "BodyBone")]
pub struct Bone {
    pub name: String,
    pub op: BoneOp,
}

/// A mesh to draw: where, with what texture, how it looks ([`Look`] as
/// numbers: a tint is ARGB).
#[derive(Schema, Debug, Clone, PartialEq, Default)]
#[data(rename = "BodyDraw")]
pub struct Draw {
    pub anchor: AnchorKind,
    /// The vanilla part, for [`AnchorKind::Bone`].
    pub bone: String,
    pub mesh: Mesh,
    pub mode: Mode,
    pub glint: bool,
    pub tint: u32,
}

/// A mod's answer — every handler's add up, in order.
#[derive(Schema, Debug, Clone, Default, PartialEq)]
#[data(rename = "BodyOps")]
pub struct Ops {
    /// The whole body's turn in entity space (after the body's yaw), column-major.
    pub root: Option<Vec<f32>>,
    #[data(default)]
    pub bones: Vec<Bone>,
    #[data(default)]
    pub draws: Vec<Draw>,
    /// Drawn as one of this entity type instead.
    pub disguise: Option<String>,
    /// The entity itself isn't drawn (its model, layers and name).
    #[data(default)]
    pub hide: bool,
    /// Sources to capture for the next frame.
    #[data(default)]
    pub captures: Vec<String>,
}

impl Ops {
    pub fn is_empty(&self) -> bool {
        *self == Ops::default()
    }

    /// Turns the whole body by `m` in entity space (after the body's yaw).
    pub fn root(&mut self, m: &Mat4) {
        let turned = match self
            .root
            .as_deref()
            .and_then(|r| <[f32; 16]>::try_from(r).ok())
        {
            Some(r) => Mat4(r) * *m,
            None => *m,
        };
        self.root = Some(turned.0.to_vec());
    }

    /// Poses the vanilla part `bone` (after vanilla animated it).
    pub fn bone(&mut self, bone: &str, op: &BoneOp) {
        self.bones.push(Bone {
            name: bone.to_string(),
            op: *op,
        });
    }

    /// Draws `quads` with `texture` at `anchor`, looking `look`.
    pub fn mesh(&mut self, anchor: &Anchor, texture: &str, look: Look, quads: &[Quad]) {
        if quads.is_empty() {
            return;
        }
        let (anchor, bone) = match anchor {
            Anchor::Entity => (AnchorKind::Entity, ""),
            Anchor::Model => (AnchorKind::Model, ""),
            Anchor::Bone(n) => (AnchorKind::Bone, n.as_str()),
            Anchor::Block => (AnchorKind::Block, ""),
        };
        let mesh = Mesh::new(texture, quads);
        self.draws.push(Draw {
            anchor,
            bone: bone.to_string(),
            mesh,
            mode: look.mode,
            glint: look.glint,
            tint: look.tint.0,
        });
    }

    /// Draws the entity as one of `entity_type`, animated like it.
    pub fn disguise(&mut self, entity_type: &str) {
        self.disguise = Some(entity_type.to_string());
    }

    /// Doesn't draw the entity itself (its model, layers and name).
    pub fn hide(&mut self) {
        self.hide = true;
    }

    /// Asks for the posed geometry of the entity (`"self"`) or of a type
    /// posed like it; it comes in the next frame's [`Frame::captures`].
    pub fn capture(&mut self, source: &str) {
        self.captures.push(source.to_string());
    }

    /// `other`'s ops after these (another handler's): turns compose, the
    /// rest adds up, a later disguise wins.
    pub fn add(&mut self, other: Ops) {
        if let Some(r) = other
            .root
            .as_deref()
            .and_then(|r| <[f32; 16]>::try_from(r).ok())
        {
            self.root(&Mat4(r));
        }
        self.bones.extend(other.bones);
        self.draws.extend(other.draws);
        self.disguise = other.disguise.or(self.disguise.take());
        self.hide |= other.hide;
        self.captures.extend(other.captures);
    }
}

impl Frame {
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// What was captured from `source` last frame.
    pub fn capture(&self, source: &str) -> Option<&Capture> {
        self.captures.iter().find(|c| c.source == source)
    }

    /// The entity's age in seconds (differs between clients: each counts
    /// from when it saw the entity).
    pub fn seconds(&self) -> f32 {
        self.age / TICKS as f32
    }

    /// The world's time in seconds — for clips everyone sees alike.
    pub fn world_seconds(&self) -> f32 {
        (self.game_time / f64::from(TICKS)) as f32
    }

    /// The value the entity keeps under `key`.
    pub fn data(&self, key: &str) -> Option<&[u8]> {
        self.data
            .iter()
            .find(|k| k.key == key)
            .map(|k| k.value.as_slice())
    }
}

#[cfg(test)]
mod tests;
