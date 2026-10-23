//! A video a mod adds (`.yvideo`), as the build compiled it: its picture's
//! AV1 packets and how to play them (`docs/design/video.md`). The runtime
//! decodes the packets; the sound plays as the event named here.

use yog_data::with::Raw;

mod clock;

pub use clock::{Clock, Plan};

/// A compiled video (`mymod:intro`).
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct VideoDef {
    pub id: String,
    /// The size it's shown at, pixels; its decoded frames may be bigger
    /// (padded up to the encoder's least), shown from their top-left.
    pub width: u32,
    pub height: u32,
    /// Frames a second, as a fraction (`num / den`).
    pub fps_num: u32,
    pub fps_den: u32,
    /// Each frame's AV1 packet's size, in order; the packets are `packets`
    /// one after another.
    pub sizes: Vec<u32>,
    #[data(with = "Raw")]
    pub packets: Vec<u8>,
    /// The frames a decoder may start at (ascending; the first is 0).
    pub keyframes: Vec<u32>,
    /// The sound event played with it (none: it's silent).
    pub sound: Option<String>,
    /// Its sound's loudness.
    pub volume: f32,
    /// It starts again at its end.
    pub looping: bool,
}

impl VideoDef {
    /// How many frames it has.
    #[must_use]
    pub fn frames(&self) -> u32 {
        self.sizes.len() as u32
    }

    /// Seconds a frame shows.
    #[must_use]
    pub fn frame_seconds(&self) -> f64 {
        f64::from(self.fps_den) / f64::from(self.fps_num.max(1))
    }

    /// How long it lasts, seconds.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        f64::from(self.frames()) * self.frame_seconds()
    }

    /// Each frame's packet, in order.
    pub fn packets(&self) -> impl Iterator<Item = &[u8]> {
        let mut at = 0usize;
        self.sizes.iter().map(move |&n| {
            let end = (at + n as usize).min(self.packets.len());
            let p = &self.packets[at.min(end)..end];
            at = end;
            p
        })
    }

    /// The keyframe at or before `frame`.
    #[must_use]
    pub fn keyframe_before(&self, frame: u32) -> u32 {
        let i = self.keyframes.partition_point(|&k| k <= frame);
        i.checked_sub(1).map_or(0, |i| self.keyframes[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> VideoDef {
        VideoDef {
            id: "m:intro".into(),
            width: 4,
            height: 2,
            fps_num: 20,
            fps_den: 1,
            sizes: vec![3, 1, 2],
            packets: vec![1, 2, 3, 4, 5, 6],
            keyframes: vec![0, 2],
            sound: Some("m:video.intro".into()),
            volume: 1.0,
            looping: false,
        }
    }

    #[test]
    fn packets_are_cut_by_their_sizes() {
        let v = def();
        let p: Vec<&[u8]> = v.packets().collect();
        assert_eq!(p, vec![&[1, 2, 3][..], &[4][..], &[5, 6][..]]);
        assert_eq!(v.frames(), 3);
        assert!((v.seconds() - 0.15).abs() < 1e-9);
    }

    #[test]
    fn a_seek_starts_at_the_keyframe_before() {
        let v = def();
        assert_eq!(
            (
                v.keyframe_before(0),
                v.keyframe_before(1),
                v.keyframe_before(2)
            ),
            (0, 0, 2)
        );
    }

    #[test]
    fn it_crosses_as_yog_data() {
        let v = def();
        let bytes = yog_data::binary::encode(&yog_data::Schema::write(&v));
        let back: VideoDef = yog_data::read_bytes(&bytes).unwrap();
        assert_eq!(back, v);
    }
}
