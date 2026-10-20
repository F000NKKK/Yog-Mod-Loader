//! Clips (`.yanim`) sampled at a time, and the pose they make: per bone a
//! turn, a move and a scale on top of the bone's rest.

use std::collections::BTreeMap;

use crate::format::{AnimFile, ClipDef, Ease, KeyDef, Loop, TracksDef};
use crate::math::{Vec3, rad};

/// What a bone does on top of its rest: radians, pixels, a factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BonePose {
    pub rotation: Vec3,
    pub position: Vec3,
    pub scale: Vec3,
}

impl Default for BonePose {
    fn default() -> Self {
        Self { rotation: Vec3::ZERO, position: Vec3::ZERO, scale: Vec3::ONE }
    }
}

impl BonePose {
    /// `other` laid over this one at `weight` (turns and moves add, scales
    /// multiply).
    pub fn add(&mut self, other: &BonePose, weight: f32) {
        self.rotation = self.rotation + other.rotation * weight;
        self.position = self.position + other.position * weight;
        self.scale = self.scale.scale(Vec3::ONE.lerp(other.scale, weight));
    }

    /// Between this pose and `to`.
    pub fn lerp(&self, to: &BonePose, t: f32) -> BonePose {
        BonePose {
            rotation: self.rotation.lerp(to.rotation, t),
            position: self.position.lerp(to.position, t),
            scale: self.scale.lerp(to.scale, t),
        }
    }
}

/// Bones by name → what they do. Names a model lacks are the vanilla
/// model's parts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pose(pub BTreeMap<String, BonePose>);

impl Pose {
    pub fn get(&self, bone: &str) -> BonePose {
        self.0.get(bone).copied().unwrap_or_default()
    }

    /// The bone's pose, to change.
    pub fn bone(&mut self, bone: &str) -> &mut BonePose {
        self.0.entry(bone.to_owned()).or_default()
    }

    /// `other` laid over this pose at `weight`.
    pub fn add(&mut self, other: &Pose, weight: f32) {
        for (name, p) in &other.0 {
            self.bone(name).add(p, weight);
        }
    }
}

/// One track's keyframes, by time.
#[derive(Debug, Clone, Default)]
struct Track(Vec<(f32, Vec3, Ease)>);

impl Track {
    fn new(keys: &[(String, KeyDef)], f: impl Fn(Vec3) -> Vec3) -> Result<Track, String> {
        let mut out = Vec::with_capacity(keys.len());
        for (t, k) in keys {
            let time: f32 = t.trim().parse().map_err(|_| format!("`{t}` isn't a time in seconds"))?;
            out.push((time, f(Vec3::from_array(k.value)), k.ease));
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(Track(out))
    }

    fn sample(&self, t: f32) -> Option<Vec3> {
        let k = &self.0;
        let first = k.first()?;
        if k.len() == 1 || t <= first.0 {
            return Some(first.1);
        }
        let last = k.last()?;
        if t >= last.0 {
            return Some(last.1);
        }
        let i = k.iter().rposition(|key| key.0 <= t)?;
        let (a, b) = (&k[i], &k[i + 1]);
        let span = (b.0 - a.0).max(1e-6);
        let s = (t - a.0) / span;
        Some(match a.2 {
            Ease::Step => a.1,
            Ease::Linear => a.1.lerp(b.1, s),
            Ease::Smooth => {
                let p0 = if i > 0 { k[i - 1].1 } else { a.1 };
                let p3 = k.get(i + 2).map_or(b.1, |n| n.1);
                catmull_rom(p0, a.1, b.1, p3, s)
            }
        })
    }
}

/// The Catmull-Rom spline's basis, doubled: row `i` weighs the four
/// points for `tⁱ`.
const CATMULL_ROM: [[f32; 4]; 4] = [[0.0, 2.0, 0.0, 0.0], [-1.0, 0.0, 1.0, 0.0], [2.0, -5.0, 4.0, -1.0], [-1.0, 3.0, -3.0, 1.0]];

fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let (points, powers) = ([p0, p1, p2, p3], [1.0, t, t * t, t * t * t]);
    let doubled = CATMULL_ROM.iter().zip(powers).fold(Vec3::ZERO, |sum, (row, tp)| {
        sum + row.iter().zip(points).fold(Vec3::ZERO, |s, (&w, p)| s + p * w) * tp
    });
    doubled * 0.5
}

#[derive(Debug, Clone, Default)]
struct Tracks {
    rotation: Track,
    position: Track,
    scale: Track,
}

impl Tracks {
    fn new(d: &TracksDef) -> Result<Tracks, String> {
        Ok(Tracks {
            rotation: Track::new(&d.rotation.0, rad)?,
            position: Track::new(&d.position.0, |v| v)?,
            scale: Track::new(&d.scale.0, |v| v)?,
        })
    }
}

/// An animation, ready.
#[derive(Debug, Clone)]
pub struct Clip {
    pub name: String,
    /// Seconds.
    pub length: f32,
    pub repeat: Loop,
    bones: Vec<(String, Tracks)>,
}

impl Clip {
    pub fn new(name: &str, d: &ClipDef) -> Result<Clip, String> {
        let bones = d
            .bones
            .0
            .iter()
            .map(|(b, t)| Tracks::new(t).map(|t| (b.clone(), t)).map_err(|e| format!("clip `{name}`, bone `{b}`: {e}")))
            .collect::<Result<_, _>>()?;
        Ok(Clip { name: name.to_owned(), length: d.length.max(0.0), repeat: d.repeat, bones })
    }

    /// The clip's time `t` seconds after it started, by its loop; `None`
    /// once a `once` clip is over.
    pub fn local_time(&self, t: f32) -> Option<f32> {
        if self.length <= 0.0 {
            return Some(0.0);
        }
        match self.repeat {
            Loop::Loop => Some(t.rem_euclid(self.length)),
            Loop::Hold => Some(t.clamp(0.0, self.length)),
            Loop::Once => (t <= self.length).then_some(t.max(0.0)),
        }
    }

    /// Whether a `once` clip has ended by `t`.
    pub fn finished(&self, t: f32) -> bool {
        self.local_time(t).is_none()
    }

    /// The pose `t` seconds after the clip started.
    pub fn sample(&self, t: f32) -> Pose {
        let mut pose = Pose::default();
        let Some(t) = self.local_time(t) else { return pose };
        for (bone, tracks) in &self.bones {
            let p = pose.bone(bone);
            if let Some(r) = tracks.rotation.sample(t) {
                p.rotation = r;
            }
            if let Some(m) = tracks.position.sample(t) {
                p.position = m;
            }
            if let Some(s) = tracks.scale.sample(t) {
                p.scale = s;
            }
        }
        pose
    }
}

/// A `.yanim`'s clips.
#[derive(Debug, Clone, Default)]
pub struct Clips(pub Vec<Clip>);

impl Clips {
    pub fn new(file: &AnimFile) -> Result<Clips, String> {
        file.0.0.iter().map(|(n, c)| Clip::new(n, c)).collect::<Result<_, _>>().map(Clips)
    }

    pub fn get(&self, name: &str) -> Option<&Clip> {
        self.0.iter().find(|c| c.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_data::Ordered;

    fn clip(repeat: Loop, ease: Ease) -> Clip {
        let rot = Ordered(vec![
            ("0".into(), KeyDef { value: [0.0, 0.0, 0.0], ease }),
            ("1".into(), KeyDef { value: [90.0, 0.0, 0.0], ease }),
        ]);
        let def = ClipDef {
            length: 1.0,
            repeat,
            bones: Ordered(vec![("arm".into(), TracksDef { rotation: rot, ..Default::default() })]),
        };
        Clip::new("swing", &def).unwrap()
    }

    #[test]
    fn keys_are_blended() {
        let r = clip(Loop::Once, Ease::Linear).sample(0.5).get("arm").rotation;
        assert!((r.x - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        let s = clip(Loop::Once, Ease::Step).sample(0.5).get("arm").rotation;
        assert_eq!(s.x, 0.0);
    }

    #[test]
    fn loops() {
        let c = clip(Loop::Loop, Ease::Linear);
        assert!((c.sample(1.5).get("arm").rotation.x - c.sample(0.5).get("arm").rotation.x).abs() < 1e-5);
        assert!(clip(Loop::Once, Ease::Linear).finished(1.5));
        assert!(!clip(Loop::Hold, Ease::Linear).finished(1.5));
    }

    #[test]
    fn smooth_keys_pass_through_their_values() {
        let c = clip(Loop::Once, Ease::Smooth);
        assert!(c.sample(1.0).get("arm").rotation.x > 1.5);
        assert_eq!(c.sample(0.0).get("arm").rotation.x, 0.0);
    }
}
