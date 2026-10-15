//! What the host and a mod say each frame about one entity (see
//! `docs/design/yog-body.md`): the [`Frame`] the host sends, the [`Ops`]
//! the mod answers with. Little-endian; strings are a u16 length and UTF-8.

use crate::math::{Mat4, Vec3};
use crate::model::Quad;

/// One entity, this frame.
#[derive(Debug, Clone, Default, PartialEq)]
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
    pub captures: Vec<Capture>,
    /// What the entity keeps by key (`Server::set_entity_data`), as this
    /// client was told.
    pub data: Vec<(String, Vec<u8>)>,
}

/// The posed geometry of an entity (entity space, see the design doc).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Capture {
    /// `self`, or the entity type captured.
    pub source: String,
    pub meshes: Vec<Mesh>,
}

/// Quads with one texture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub texture: String,
    pub quads: Vec<Quad>,
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

/// How a mesh is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
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

/// A mod's answer: operations, written in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ops(pub Vec<u8>);

const ROOT: u8 = 1;
const BONE: u8 = 2;
const MESH: u8 = 3;
const DISGUISE: u8 = 4;
const HIDE: u8 = 5;
const CAPTURE: u8 = 6;

impl Ops {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Turns the whole body by `m` in entity space (after the body's yaw).
    pub fn root(&mut self, m: &Mat4) {
        self.0.push(ROOT);
        for f in m.0 {
            put_f32(&mut self.0, f);
        }
    }

    /// Poses the vanilla part `bone` (after vanilla animated it).
    pub fn bone(&mut self, bone: &str, op: &BoneOp) {
        let b = &mut self.0;
        b.push(BONE);
        put_str(b, bone);
        let mut flags = 0u8;
        if op.set_rotation {
            flags |= 1;
        }
        if op.set_position {
            flags |= 2;
        }
        if op.scale.is_some() {
            flags |= 4;
        }
        match op.visible {
            Some(false) => flags |= 8,
            Some(true) => flags |= 16,
            None => {}
        }
        b.push(flags);
        for v in [op.rotation, op.position, op.scale.unwrap_or(Vec3::ONE)] {
            put_vec(b, v);
        }
    }

    /// Draws `quads` with `texture` at `anchor`. `argb` tints it (alpha
    /// counts in [`Mode::Translucent`]); `glint` adds the enchantment shine.
    pub fn mesh(&mut self, anchor: &Anchor, texture: &str, mode: Mode, glint: bool, argb: u32, quads: &[Quad]) {
        if quads.is_empty() {
            return;
        }
        let b = &mut self.0;
        b.push(MESH);
        let (a, bone) = match anchor {
            Anchor::Entity => (0u8, ""),
            Anchor::Model => (1, ""),
            Anchor::Bone(n) => (2, n.as_str()),
            Anchor::Block => (3, ""),
        };
        b.push(a);
        put_str(b, bone);
        put_str(b, texture);
        b.push(match mode {
            Mode::Cutout => 0,
            Mode::Translucent => 1,
            Mode::Emissive => 2,
        });
        b.push(glint as u8);
        b.extend_from_slice(&argb.to_le_bytes());
        put_quads(b, quads);
    }

    /// Draws the entity as one of `entity_type`, animated like it.
    pub fn disguise(&mut self, entity_type: &str) {
        self.0.push(DISGUISE);
        put_str(&mut self.0, entity_type);
    }

    /// Doesn't draw the entity itself (its model, layers and name).
    pub fn hide(&mut self) {
        self.0.push(HIDE);
    }

    /// Asks for the posed geometry of the entity (`"self"`) or of a type
    /// posed like it; it comes in the next frame's [`Frame::captures`].
    pub fn capture(&mut self, source: &str) {
        self.0.push(CAPTURE);
        put_str(&mut self.0, source);
    }
}

// ── Encoding ────────────────────────────────────────────────────────────────

fn put_f32(b: &mut Vec<u8>, f: f32) {
    b.extend_from_slice(&f.to_le_bytes());
}

fn put_vec(b: &mut Vec<u8>, v: Vec3) {
    put_f32(b, v.x);
    put_f32(b, v.y);
    put_f32(b, v.z);
}

fn put_str(b: &mut Vec<u8>, s: &str) {
    let s = &s.as_bytes()[..s.len().min(u16::MAX as usize)];
    b.extend_from_slice(&(s.len() as u16).to_le_bytes());
    b.extend_from_slice(s);
}

/// Vertex count, then x y z u v nx ny nz per vertex.
fn put_quads(b: &mut Vec<u8>, quads: &[Quad]) {
    b.extend_from_slice(&((quads.len() * 4) as u32).to_le_bytes());
    for q in quads {
        for i in 0..4 {
            put_vec(b, q.pos[i]);
            put_f32(b, q.uv[i][0]);
            put_f32(b, q.uv[i][1]);
            put_vec(b, q.normal);
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.b.len() < n {
            return None;
        }
        let (h, t) = self.b.split_at(n);
        self.b = t;
        Some(h)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    }
    fn f32(&mut self) -> Option<f32> {
        self.take(4).map(|b| f32::from_le_bytes(b.try_into().unwrap()))
    }
    fn f64(&mut self) -> Option<f64> {
        self.take(8).map(|b| f64::from_le_bytes(b.try_into().unwrap()))
    }
    fn vec(&mut self) -> Option<Vec3> {
        Some(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u16()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
    fn quads(&mut self) -> Option<Vec<Quad>> {
        let n = self.u32()? as usize / 4;
        let mut out = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            let mut q = Quad { pos: [Vec3::ZERO; 4], uv: [[0.0; 2]; 4], normal: Vec3::ZERO };
            for i in 0..4 {
                q.pos[i] = self.vec()?;
                q.uv[i] = [self.f32()?, self.f32()?];
                q.normal = self.vec()?;
            }
            out.push(q);
        }
        Some(out)
    }
}

impl Frame {
    /// Reads what the host sent.
    pub fn decode(bytes: &[u8]) -> Option<Frame> {
        let mut r = Reader { b: bytes };
        if r.u8()? != 2 {
            return None;
        }
        let mut f = Frame {
            uuid: r.str()?,
            entity_type: r.str()?,
            partial: r.f32()?,
            age: r.f32()?,
            limb_swing: r.f32()?,
            limb_amount: r.f32()?,
            head_yaw: r.f32()?,
            head_pitch: r.f32()?,
            body_yaw: r.f32()?,
            pitch: r.f32()?,
            pos: [r.f64()?, r.f64()?, r.f64()?],
            velocity: r.vec()?,
            flags: r.u32()?,
            game_time: r.f64()?,
            captures: Vec::new(),
            data: Vec::new(),
        };
        for _ in 0..r.u16()? {
            let source = r.str()?;
            let mut meshes = Vec::new();
            for _ in 0..r.u16()? {
                meshes.push(Mesh { texture: r.str()?, quads: r.quads()? });
            }
            f.captures.push(Capture { source, meshes });
        }
        for _ in 0..r.u16()? {
            let key = r.str()?;
            let n = r.u32()? as usize;
            f.data.push((key, r.take(n)?.to_vec()));
        }
        Some(f)
    }

    /// As the host writes it (for tests and tools).
    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![2u8];
        put_str(&mut b, &self.uuid);
        put_str(&mut b, &self.entity_type);
        for f in [self.partial, self.age, self.limb_swing, self.limb_amount, self.head_yaw, self.head_pitch, self.body_yaw, self.pitch] {
            put_f32(&mut b, f);
        }
        for p in self.pos {
            b.extend_from_slice(&p.to_le_bytes());
        }
        put_vec(&mut b, self.velocity);
        b.extend_from_slice(&self.flags.to_le_bytes());
        b.extend_from_slice(&self.game_time.to_le_bytes());
        b.extend_from_slice(&(self.captures.len() as u16).to_le_bytes());
        for c in &self.captures {
            put_str(&mut b, &c.source);
            b.extend_from_slice(&(c.meshes.len() as u16).to_le_bytes());
            for m in &c.meshes {
                put_str(&mut b, &m.texture);
                put_quads(&mut b, &m.quads);
            }
        }
        b.extend_from_slice(&(self.data.len() as u16).to_le_bytes());
        for (k, v) in &self.data {
            put_str(&mut b, k);
            b.extend_from_slice(&(v.len() as u32).to_le_bytes());
            b.extend_from_slice(v);
        }
        b
    }

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
        self.age / 20.0
    }

    /// The world's time in seconds — for clips everyone sees alike.
    pub fn world_seconds(&self) -> f32 {
        (self.game_time / 20.0) as f32
    }

    /// The value the entity keeps under `key`.
    pub fn data(&self, key: &str) -> Option<&[u8]> {
        self.data.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_reads_back() {
        let q = Quad { pos: [Vec3::new(1.0, 2.0, 3.0); 4], uv: [[0.5, 0.25]; 4], normal: Vec3::new(0.0, 1.0, 0.0) };
        let f = Frame {
            uuid: "u".into(),
            entity_type: "minecraft:player".into(),
            age: 12.5,
            pos: [1.0, 2.0, 3.0],
            flags: 5,
            game_time: 1234.5,
            data: vec![("m:k".into(), vec![1, 2])],
            captures: vec![Capture { source: "self".into(), meshes: vec![Mesh { texture: "t".into(), quads: vec![q] }] }],
            ..Default::default()
        };
        assert_eq!(Frame::decode(&f.encode()), Some(f));
    }

    #[test]
    fn ops_are_records() {
        let mut o = Ops::default();
        o.hide();
        o.disguise("minecraft:zombie");
        assert_eq!(o.0[0], HIDE);
        assert_eq!(o.0[1], DISGUISE);
        assert_eq!(&o.0[2..4], &16u16.to_le_bytes());
    }
}
