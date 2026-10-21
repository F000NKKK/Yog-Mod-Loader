//! A `.ymodel` made ready to draw: its bones in order, each cube baked into
//! the quads vanilla's `ModelPart.Cube` makes (same corners, same box-UV,
//! same mirroring) — so a model drawn here looks as one drawn by the game.

use crate::format::{CubeDef, ModelFile};
use crate::math::{Mat4, Vec3, rad};

/// One four-cornered face: corners, their UVs (0–1), its normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub pos: [Vec3; 4],
    pub uv: [[f32; 2]; 4],
    pub normal: Vec3,
}

impl Quad {
    /// This quad moved by `m`.
    pub fn transformed(&self, m: &Mat4) -> Quad {
        Quad {
            pos: self.pos.map(|p| m.transform_point(p)),
            uv: self.uv,
            normal: m.transform_normal(self.normal),
        }
    }

    /// The middle of its corners.
    pub fn center(&self) -> Vec3 {
        self.pos.iter().fold(Vec3::ZERO, |sum, &p| sum + p) * (1.0 / self.pos.len() as f32)
    }
}

/// A bone, ready.
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    /// Index of its parent in [`Model::bones`] (always before it).
    pub parent: Option<usize>,
    /// Pixels.
    pub pivot: Vec3,
    /// Radians.
    pub rotation: Vec3,
    /// Its cubes' faces, in its own space (pixels).
    pub quads: Vec<Quad>,
    pub locators: Vec<(String, Vec3)>,
}

/// A model, ready.
#[derive(Debug, Clone)]
pub struct Model {
    /// The texture as the host loads it (`ns:textures/….png`).
    pub texture: String,
    pub bones: Vec<Bone>,
}

impl Model {
    /// Builds `file`. Errors name what's wrong (an unknown or later
    /// parent, a repeated name).
    pub fn new(file: &ModelFile) -> Result<Model, String> {
        let texture = texture_resource(&file.texture);
        let [tw, th] = file.texture_size.map(|n| n.max(1) as f32);
        let mut bones: Vec<Bone> = Vec::with_capacity(file.bones.len());
        for b in &file.bones {
            if bones.iter().any(|o| o.name == b.name) {
                return Err(format!("bone `{}` comes twice", b.name));
            }
            let parent = match &b.parent {
                None => None,
                Some(p) => Some(bones.iter().position(|o| &o.name == p).ok_or_else(|| {
                    format!("bone `{}`: its parent `{p}` must come before it", b.name)
                })?),
            };
            bones.push(Bone {
                name: b.name.clone(),
                parent,
                pivot: Vec3::from_array(b.pivot),
                rotation: rad(Vec3::from_array(b.rotation)),
                quads: b.cubes.iter().flat_map(|c| cube(c, tw, th)).collect(),
                locators: b
                    .locators
                    .0
                    .iter()
                    .map(|(n, p)| (n.clone(), Vec3::from_array(*p)))
                    .collect(),
            });
        }
        Ok(Model { texture, bones })
    }

    pub fn bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }
}

/// The texture as the host loads it: `<ns>:textures/<path>.png`.
pub fn texture_resource(id: &str) -> String {
    let (ns, path) = yog_ids::split(id);
    yog_ids::join(ns, &yog_ids::texture_file(path))
}

/// `ModelPart.Cube`'s six faces.
pub fn cube(c: &CubeDef, tex_w: f32, tex_h: f32) -> [Quad; 6] {
    let [x, y, z] = c.origin;
    let [w, h, d] = c.size;
    let [u, v] = c.uv;
    let g = c.inflate;
    let (mut x0, y0, z0) = (x - g, y - g, z - g);
    let (mut x1, y1, z1) = (x + w + g, y + h + g, z + d + g);
    if c.mirror {
        std::mem::swap(&mut x0, &mut x1);
    }
    let p = |x, y, z| Vec3::new(x, y, z);
    let (v7, v0, v1, v2) = (p(x0, y0, z0), p(x1, y0, z0), p(x1, y1, z0), p(x0, y1, z0));
    let (v3, v4, v5, v6) = (p(x0, y0, z1), p(x1, y0, z1), p(x1, y1, z1), p(x0, y1, z1));
    let (u0, u1, u2, u3, u4, u5) = (
        u,
        u + d,
        u + d + w,
        u + d + w + w,
        u + d + w + d,
        u + d + w + d + w,
    );
    let (t0, t1, t2) = (v, v + d, v + d + h);
    let face = |corners: [Vec3; 4], ua: f32, va: f32, ub: f32, vb: f32, normal: Vec3| {
        let mut pos = corners;
        let mut uv = [
            [ub / tex_w, va / tex_h],
            [ua / tex_w, va / tex_h],
            [ua / tex_w, vb / tex_h],
            [ub / tex_w, vb / tex_h],
        ];
        let mut n = normal;
        if c.mirror {
            pos.reverse();
            uv.reverse();
            n.x = -n.x;
        }
        Quad { pos, uv, normal: n }
    };
    [
        face([v4, v3, v7, v0], u1, t0, u2, t1, Vec3::new(0.0, -1.0, 0.0)),
        face([v1, v2, v6, v5], u2, t1, u3, t0, Vec3::new(0.0, 1.0, 0.0)),
        face([v7, v3, v6, v2], u0, t1, u1, t2, Vec3::new(-1.0, 0.0, 0.0)),
        face([v0, v7, v2, v1], u1, t1, u2, t2, Vec3::new(0.0, 0.0, -1.0)),
        face([v4, v0, v1, v5], u2, t1, u4, t2, Vec3::new(1.0, 0.0, 0.0)),
        face([v3, v4, v5, v6], u4, t1, u5, t2, Vec3::new(0.0, 0.0, 1.0)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wing() -> CubeDef {
        CubeDef {
            origin: [-10.0, 0.0, 0.0],
            size: [10.0, 20.0, 2.0],
            uv: [22.0, 0.0],
            inflate: 1.0,
            mirror: false,
        }
    }

    #[test]
    fn a_cube_is_vanillas() {
        let q = cube(&wing(), 64.0, 32.0);
        // DOWN: v4, v3, v7, v0 with UVs from (d, 0) to (d + w, d).
        assert_eq!(q[0].pos[0], Vec3::new(1.0, -1.0, 3.0));
        assert_eq!(q[0].uv[0], [34.0 / 64.0, 0.0]);
        assert_eq!(q[0].uv[1], [24.0 / 64.0, 0.0]);
        assert_eq!(q[3].normal, Vec3::new(0.0, 0.0, -1.0));
    }

    #[test]
    fn a_mirrored_cube_turns_its_faces() {
        let mut c = wing();
        c.mirror = true;
        let q = cube(&c, 64.0, 32.0);
        assert_eq!(q[2].normal, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(q[0].pos[3], Vec3::new(-11.0, -1.0, 3.0));
    }

    #[test]
    fn a_texture_is_loaded_from_its_resource() {
        assert_eq!(
            texture_resource("minecraft:entity/elytra"),
            "minecraft:textures/entity/elytra.png"
        );
    }
}
