//! Morphing one body into another, as the old Morph mod did: the faces of
//! the first slide, turn and stretch into the faces of the second, while the
//! first's skin fades into the second's.
//!
//! Faces pair up in the order the models give them (cube by cube, as their
//! parts come); faces one side has more of grow from — or shrink into — the
//! middle of the other body.

use std::collections::BTreeMap;

use crate::math::Vec3;
use crate::model::Quad;
use crate::wire::{Anchor, Mesh, Mode, Ops};

/// One face of a body, with its texture.
#[derive(Clone, Copy)]
struct Face<'a> {
    texture: &'a str,
    quad: Quad,
}

fn faces(meshes: &[Mesh]) -> Vec<Face<'_>> {
    meshes.iter().flat_map(|m| m.quads.iter().map(move |q| Face { texture: &m.texture, quad: *q })).collect()
}

fn middle(faces: &[Face<'_>]) -> Vec3 {
    if faces.is_empty() {
        return Vec3::ZERO;
    }
    faces.iter().map(|f| f.quad.center()).fold(Vec3::ZERO, |a, b| a + b) * (1.0 / faces.len() as f32)
}

/// A face shrunk to a point.
fn point(at: Vec3, like: &Quad) -> Quad {
    Quad { pos: [at; 4], uv: like.uv, normal: like.normal }
}

fn lerp(a: &Quad, b: &Quad, t: f32) -> [Vec3; 4] {
    [0, 1, 2, 3].map(|i| a.pos[i].lerp(b.pos[i], t))
}

/// Writes `from` turning into `to` at `t` (0: all `from`, 1: all `to`)
/// into `ops`, in entity space — both as [`crate::wire::Capture`]s give
/// them. Smooth `t` (an ease) is the caller's choice.
pub fn blend(ops: &mut Ops, from: &[Mesh], to: &[Mesh], t: f32) {
    let t = t.clamp(0.0, 1.0);
    let (a, b) = (faces(from), faces(to));
    let (ma, mb) = (middle(&a), middle(&b));
    // Texture → (quads in the old skin, quads in the new).
    let mut old: BTreeMap<&str, Vec<Quad>> = BTreeMap::new();
    let mut new: BTreeMap<&str, Vec<Quad>> = BTreeMap::new();
    for i in 0..a.len().max(b.len()) {
        let fa = a.get(i).map(|f| (f.texture, f.quad));
        let fb = b.get(i).map(|f| (f.texture, f.quad));
        let (qa, qb) = match (fa, fb) {
            (Some((_, qa)), Some((_, qb))) => (qa, qb),
            (Some((_, qa)), None) => (qa, point(mb, &qa)),
            (None, Some((_, qb))) => (point(ma, &qb), qb),
            (None, None) => continue,
        };
        let pos = lerp(&qa, &qb, t);
        let normal = qa.normal.lerp(qb.normal, t).normalize();
        if let Some((tex, q)) = fa {
            old.entry(tex).or_default().push(Quad { pos, uv: q.uv, normal });
        }
        if let Some((tex, q)) = fb {
            new.entry(tex).or_default().push(Quad { pos, uv: q.uv, normal });
        }
    }
    let alpha = |k: f32| ((k.clamp(0.0, 1.0) * 255.0).round() as u32) << 24 | 0x00FF_FFFF;
    for (tex, quads) in &old {
        if t < 1.0 {
            ops.mesh(&Anchor::Entity, tex, Mode::Translucent, false, alpha(1.0 - t), quads);
        }
    }
    for (tex, quads) in &new {
        if t > 0.0 {
            ops.mesh(&Anchor::Entity, tex, Mode::Translucent, false, alpha(t), quads);
        }
    }
}

/// `t` eased in and out (smoothstep) — how a morph looks natural.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(tex: &str, at: f32, n: usize) -> Mesh {
        let q = Quad { pos: [Vec3::new(at, 0.0, 0.0); 4], uv: [[0.0; 2]; 4], normal: Vec3::new(0.0, 1.0, 0.0) };
        Mesh { texture: tex.into(), quads: vec![q; n] }
    }

    #[test]
    fn half_way_draws_both_skins() {
        let mut ops = Ops::default();
        blend(&mut ops, &[mesh("a", 0.0, 2)], &[mesh("b", 2.0, 3)], 0.5);
        assert!(ops.0.iter().filter(|b| **b == 3).count() >= 2);
        let mut none = Ops::default();
        blend(&mut none, &[mesh("a", 0.0, 2)], &[mesh("b", 2.0, 3)], 0.0);
        assert!(!none.is_empty());
    }
}
