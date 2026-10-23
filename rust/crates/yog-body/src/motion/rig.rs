//! A [`Rig`]: a model with clips playing on it, layered, and poses a mod
//! sets by hand on top — evaluated into quads to draw, vanilla parts to
//! pose, and locators to follow.

use yog_sync::Arc;

use crate::math::{Mat4, Vec3};
use crate::motion::anim::{BonePose, Clip, Pose};
use crate::shape::model::{Model, Quad};
use crate::wire::{Anchor, BoneOp, Look, Ops};

/// A clip playing.
#[derive(Debug, Clone)]
pub struct Layer {
    pub clip: Arc<Clip>,
    /// When it started, in the rig's seconds.
    pub start: f32,
    pub speed: f32,
    /// How much of it shows (0–1); fades lay clips over each other.
    pub weight: f32,
}

/// A model and what it does.
#[derive(Debug, Clone)]
pub struct Rig {
    pub model: Arc<Model>,
    /// Where the model's root bones hang: on a vanilla part, the model, the
    /// entity, or a block.
    pub anchor: Anchor,
    pub layers: Vec<Layer>,
    /// Set by hand each frame, laid over the clips.
    pub manual: Pose,
    pub look: Look,
}

impl Rig {
    pub fn new(model: Arc<Model>, anchor: Anchor) -> Rig {
        Rig {
            model,
            anchor,
            layers: Vec::new(),
            manual: Pose::default(),
            look: Look::default(),
        }
    }

    /// Plays `clip` from `now` (replacing the same clip if it plays).
    pub fn play(&mut self, clip: Arc<Clip>, now: f32) {
        self.layers.retain(|l| l.clip.name != clip.name);
        self.layers.push(Layer {
            clip,
            start: now,
            speed: 1.0,
            weight: 1.0,
        });
    }

    /// Stops `clip`.
    pub fn stop(&mut self, clip: &str) {
        self.layers.retain(|l| l.clip.name != clip);
    }

    pub fn playing(&self, clip: &str) -> bool {
        self.layers.iter().any(|l| l.clip.name == clip)
    }

    /// The pose at `now`: the clips by weight, then the manual pose. Clips
    /// that ended (`once`) are dropped.
    pub fn pose(&mut self, now: f32) -> Pose {
        self.layers
            .retain(|l| !l.clip.finished((now - l.start) * l.speed));
        let mut pose = Pose::default();
        for l in &self.layers {
            pose.add(&l.clip.sample((now - l.start) * l.speed), l.weight);
        }
        pose.add(&self.manual, 1.0);
        pose
    }

    /// Each bone's matrix in the anchor's space.
    pub fn matrices(&self, pose: &Pose) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.model.bones.len());
        for b in &self.model.bones {
            let p = pose.get(&b.name);
            let local = crate::math::part(b.pivot + p.position, b.rotation + p.rotation, p.scale);
            out.push(match b.parent {
                Some(i) => out[i] * local,
                None => local,
            });
        }
        out
    }

    /// The model's faces, posed, in the anchor's space.
    pub fn quads(&self, pose: &Pose) -> Vec<Quad> {
        let m = self.matrices(pose);
        self.model
            .bones
            .iter()
            .zip(&m)
            .flat_map(|(b, m)| b.quads.iter().map(move |q| q.transformed(m)))
            .collect()
    }

    /// Where locator `name` is now, in the anchor's space.
    pub fn locator(&self, name: &str, pose: &Pose) -> Option<Vec3> {
        let m = self.matrices(pose);
        self.model.bones.iter().zip(&m).find_map(|(b, m)| {
            b.locators
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, p)| m.transform_point(*p))
        })
    }

    /// Writes the rig at `now` into `ops`: its quads, and the pose of the
    /// vanilla parts its clips move (bones the model lacks).
    pub fn write(&mut self, ops: &mut Ops, now: f32) {
        let pose = self.pose(now);
        for (name, p) in &pose.0 {
            if self.model.bone(name).is_none() {
                ops.bone(name, &vanilla_op(p));
            }
        }
        let quads = self.quads(&pose);
        ops.mesh(&self.anchor, &self.model.texture, self.look, &quads);
    }
}

/// A clip's pose of a vanilla part, added to vanilla's.
pub fn vanilla_op(p: &BonePose) -> BoneOp {
    BoneOp {
        rotation: p.rotation,
        position: p.position,
        scale: (p.scale != Vec3::ONE).then_some(p.scale),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::format::{BoneDef, CubeDef, ModelFile};
    use yog_data::Ordered;

    fn model() -> Arc<Model> {
        let file = ModelFile {
            texture: "m:x".into(),
            video: None,
            texture_size: [64, 32],
            bones: vec![
                BoneDef {
                    name: "a".into(),
                    parent: None,
                    pivot: [0.0, 0.0, 0.0],
                    rotation: [0.0; 3],
                    cubes: vec![],
                    locators: Ordered::default(),
                },
                BoneDef {
                    name: "b".into(),
                    parent: Some("a".into()),
                    pivot: [0.0, 10.0, 0.0],
                    rotation: [0.0; 3],
                    cubes: vec![CubeDef {
                        origin: [0.0; 3],
                        size: [1.0; 3],
                        uv: [0.0; 2],
                        inflate: 0.0,
                        mirror: false,
                    }],
                    locators: Ordered(vec![("tip".into(), [0.0, 5.0, 0.0])]),
                },
            ],
        };
        Arc::new(Model::new(&file).unwrap())
    }

    #[test]
    fn children_follow_parents() {
        let mut rig = Rig::new(model(), Anchor::Model);
        rig.manual.bone("a").rotation = Vec3::new(0.0, 0.0, std::f32::consts::FRAC_PI_2);
        let pose = rig.pose(0.0);
        let tip = rig.locator("tip", &pose).unwrap();
        assert!(
            (tip - Vec3::new(-15.0, 0.0, 0.0)).length() < 1e-4,
            "{tip:?}"
        );
        assert_eq!(rig.quads(&pose).len(), 6);
    }
}
