//! A video made of clips (`.yvideo`'s `montage`), laid out as the sound
//! designer's DAW lays sounds: videos and pictures on picture tracks — a
//! higher track drawn over a lower — and the sound a [`Mix`], the videos'
//! own sound laid in with it. The build renders it (ffmpeg); the designer
//! plays the same description.

use yog_data::{Res, Schema};

use crate::montage::Mix;

/// How long a picture shows when its clip doesn't say, seconds.
pub const PICTURE_SECONDS: f32 = 3.0;

/// A video's montage.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Montage {
    /// The picture tracks, by number (a track not listed: shown as it is).
    #[data(default)]
    pub tracks: Vec<PictureTrack>,
    /// The picture clips, each where it starts; they overlap freely.
    #[data(default)]
    pub clips: Vec<PictureClip>,
    /// Its sound: a mix as a `.ysound`'s.
    pub sound: Option<Mix>,
}

/// A picture track: what the designer calls it, and whether it's shown.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct PictureTrack {
    pub name: Option<String>,
    /// Its colour in the designer, `0xRRGGBB`.
    pub color: Option<u32>,
    /// Left out of the picture.
    #[data(default)]
    pub hidden: bool,
}

/// A video or a picture, placed in a montage — fitted in its frame, its
/// proportions kept.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct PictureClip {
    /// The track it's on: a higher one is drawn over a lower.
    #[data(default)]
    pub track: u32,
    /// The mod's video (a `.yres` video)…
    pub video: Option<Res>,
    /// …or picture (a `.yres` image).
    pub image: Option<Res>,
    /// Seconds into the montage it starts.
    #[data(default)]
    pub at: f32,
    /// The part shown: a video's from `from` seconds to `to` (its end if
    /// not given); a picture shows `to - from` seconds
    /// ([`PICTURE_SECONDS`] if not given).
    #[data(default)]
    pub from: f32,
    pub to: Option<f32>,
    /// How fast a video plays (2: twice as fast, half as long).
    #[data(default = "one")]
    pub speed: f32,
    /// Seconds it fades in at its start, out at its end.
    #[data(default)]
    pub fade_in: f32,
    #[data(default)]
    pub fade_out: f32,
    /// How much it covers what's under it, 0..1.
    #[data(default = "one")]
    pub opacity: f32,
    /// A video's own sound laid into the montage's, at this loudness (0: none).
    #[data(default = "one")]
    pub gain: f32,
}

fn one() -> f32 {
    1.0
}

impl PictureClip {
    /// How long it shows, seconds, when its video lasts `length` (none: a picture).
    #[must_use]
    pub fn duration(&self, length: Option<f32>) -> f32 {
        let to = match length {
            Some(l) => self.to.unwrap_or(l).min(l),
            None => self.to.unwrap_or(self.from + PICTURE_SECONDS),
        };
        let shown = (to - self.from).max(0.0);
        if length.is_some() {
            shown / self.speed.max(f32::EPSILON)
        } else {
            shown
        }
    }
}

impl Montage {
    /// Whether picture track `n` is shown.
    #[must_use]
    pub fn shown(&self, n: u32) -> bool {
        self.tracks.get(n as usize).is_none_or(|t| !t.hidden)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_last_as_cut_and_sped() {
        let m = yog_data::from_text::<Montage>(
            r#"{ tracks: [{}, { hidden: true }], clips: [
                { video: "@res:Videos.a", from: 1, to: 5, speed: 2 },
                { image: "@res:Images.logo", at: 2, track: 1 } ] }"#,
        )
        .unwrap()
        .value;
        assert_eq!(m.clips[0].duration(Some(10.0)), 2.0);
        assert_eq!(
            m.clips[0].duration(Some(3.0)),
            1.0,
            "cut at the video's end"
        );
        assert_eq!(m.clips[1].duration(None), PICTURE_SECONDS);
        assert!(m.shown(0) && !m.shown(1) && m.shown(2));
    }
}
