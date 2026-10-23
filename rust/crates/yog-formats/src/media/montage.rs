//! A montage: a sound made of others — clips of the mod's (or the game's)
//! sounds laid on a timeline, each cut, moved, faded, sped up or reversed
//! and given effects, then effects over the whole. Its sources stay as
//! they are; the build renders it (ffmpeg) into the one sound the game
//! plays, and the designers play the same description. A video's
//! montage (`crate::video::Montage`) has one for its sound.

use yog_data::{Res, Schema};

/// A sound made of clips (`.ysound` variants' `mix`), as a music
/// program lays one out: clips on tracks, each track a mixer channel, all
/// into the master.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Mix {
    /// Beats a minute and beats a bar: the grid the designer snaps to (the sound doesn't change).
    #[data(default = "tempo")]
    pub tempo: f32,
    #[data(default = "beats_per_bar")]
    pub beats_per_bar: u32,
    /// The tracks, by number: a clip plays through its track's channel
    /// (a track not listed: as it is).
    #[data(default)]
    pub tracks: Vec<Track>,
    /// The clips, each where it starts; they overlap freely.
    pub clips: Vec<SoundClip>,
    /// The master's loudness, and its effects in order.
    #[data(default = "one")]
    pub gain: f32,
    #[data(default)]
    pub effects: Vec<SoundEffect>,
}

/// A track and its mixer channel: what its clips go through together.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Track {
    /// What the designer calls it.
    pub name: Option<String>,
    /// Its colour in the designer, `0xRRGGBB`.
    pub color: Option<u32>,
    /// Its loudness, times its clips'.
    #[data(default = "one")]
    pub gain: f32,
    /// Left out of the mix.
    #[data(default)]
    pub mute: bool,
    /// Only the soloed tracks are in the mix, when any is.
    #[data(default)]
    pub solo: bool,
    /// Its effects, in order.
    #[data(default)]
    pub effects: Vec<SoundEffect>,
}

/// A part of a sound, placed in a mix.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct SoundClip {
    /// The track it's on (its mixer channel).
    #[data(default)]
    pub track: u32,
    /// The mod's sound (a `.yres` sound)…
    pub sound: Option<Res>,
    /// …or a sound file the game has (`minecraft:dig/sand1`).
    pub file: Option<String>,
    /// Seconds into the mix it starts.
    #[data(default)]
    pub at: f32,
    /// The part of its sound it plays: from `from` seconds to `to` (its end if not given).
    #[data(default)]
    pub from: f32,
    pub to: Option<f32>,
    /// Its loudness, times its own.
    #[data(default = "one")]
    pub gain: f32,
    /// Seconds it fades in at its start, out at its end.
    #[data(default)]
    pub fade_in: f32,
    #[data(default)]
    pub fade_out: f32,
    /// How fast it plays — its pitch with it, as the game's pitch does (2: an octave up, twice as short).
    #[data(default = "one")]
    pub speed: f32,
    /// It plays backwards.
    #[data(default)]
    pub reverse: bool,
    /// Effects on it alone, in order.
    #[data(default)]
    pub effects: Vec<SoundEffect>,
}

/// What a sound goes through.
#[derive(Schema, Debug, Clone, PartialEq)]
#[data(tag = "fx")]
pub enum SoundEffect {
    /// Heard once more `delay` seconds later, at `decay` of its loudness.
    Echo { delay: f32, decay: f32 },
    /// Only what's lower than `hz`: muffled, as through a wall.
    LowPass { hz: f32 },
    /// Only what's higher than `hz`: thin, as through a phone.
    HighPass { hz: f32 },
}

/// The designer's grid when a mix doesn't say: 120 beats a minute, four a bar.
const TEMPO: f32 = 120.0;
const BEATS_PER_BAR: u32 = 4;

fn one() -> f32 {
    1.0
}

fn tempo() -> f32 {
    TEMPO
}

fn beats_per_bar() -> u32 {
    BEATS_PER_BAR
}

impl Default for Mix {
    /// An empty mix on the designer's default grid.
    fn default() -> Self {
        Mix {
            tempo: TEMPO,
            beats_per_bar: BEATS_PER_BAR,
            tracks: Vec::new(),
            clips: Vec::new(),
            gain: one(),
            effects: Vec::new(),
        }
    }
}

impl Mix {
    /// Whether track `n` is heard: not muted, and soloed when any track is.
    #[must_use]
    pub fn heard(&self, n: u32) -> bool {
        let soloed = self.tracks.iter().any(|t| t.solo);
        self.tracks
            .get(n as usize)
            .map_or(!soloed, |t| !t.mute && (t.solo || !soloed))
    }
}

impl SoundClip {
    /// How long it plays in the mix, seconds, when its sound lasts `length`.
    #[must_use]
    pub fn duration(&self, length: f32) -> f32 {
        let to = self.to.unwrap_or(length).min(length);
        (to - self.from).max(0.0) / self.speed.max(f32::EPSILON)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mix_reads_its_clips_and_effects() {
        let text = r#"{ clips: [{ sound: "@Sounds.abacus1", at: 0.5, to: 1, speed: 2,
            effects: [{ fx: "low_pass", hz: 800 }] }], effects: [{ fx: "echo", delay: 0.2, decay: 0.4 }] }"#;
        let mix = yog_data::from_text::<Mix>(text).unwrap().value;
        let clip = &mix.clips[0];
        assert_eq!((clip.at, clip.gain, clip.duration(3.0)), (0.5, 1.0, 0.5));
        assert_eq!(clip.effects, vec![SoundEffect::LowPass { hz: 800.0 }]);
        assert_eq!(
            mix.effects,
            vec![SoundEffect::Echo {
                delay: 0.2,
                decay: 0.4
            }]
        );
        assert_eq!(
            (mix.tempo, mix.beats_per_bar, mix.gain),
            (TEMPO, BEATS_PER_BAR, 1.0)
        );
    }

    #[test]
    fn a_soloed_track_leaves_the_others_out() {
        let text = r#"{ tracks: [{ solo: true }, {}, { mute: true }], clips: [] }"#;
        let mix = yog_data::from_text::<Mix>(text).unwrap().value;
        assert_eq!(
            (mix.heard(0), mix.heard(1), mix.heard(2), mix.heard(3)),
            (true, false, false, false)
        );
    }
}
