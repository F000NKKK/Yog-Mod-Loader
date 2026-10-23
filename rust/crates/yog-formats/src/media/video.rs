//! `.yvideo` — a video: what it shows, at what size and frame rate, and
//! how it plays (`docs/design/video.md`). Its id is its path under
//! `resources/video/` with `/` as `.` (`resources/video/intro.yvideo` is
//! `<mod id>:intro`). The build renders it — the picture as AV1, its sound
//! as an Ogg sound event — into a definition the runtime plays.

use yog_data::{Res, Schema};

mod montage;

pub use montage::{Montage, PICTURE_SECONDS, PictureClip, PictureTrack};

/// Where `.yvideo` files live, under `resources/`.
pub const DIR: &str = yog_ids::kind::VIDEO;

/// A montage's frame rate when the file doesn't say.
pub const MONTAGE_FPS: f32 = 30.0;

/// The best quality asked for and the worst: AV1's constant rate factor
/// (lower is better and bigger).
pub const BEST_QUALITY: u32 = 0;
pub const WORST_QUALITY: u32 = 63;
/// A good look at a modest size.
const QUALITY: u32 = 35;

/// A video (a `.yvideo` file).
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Video {
    /// The `.yres` video it shows…
    pub source: Option<Res>,
    /// …or a montage of videos, pictures and sounds.
    pub montage: Option<Montage>,
    /// Its size, pixels: a source's when not given (one given, the other
    /// keeps the source's proportions); a montage needs both.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Frames a second: a source's when not given, a montage's
    /// [`MONTAGE_FPS`].
    pub fps: Option<f32>,
    /// AV1's rate factor, [`BEST_QUALITY`]..=[`WORST_QUALITY`].
    #[data(default = "quality")]
    pub quality: u32,
    /// It starts again at its end.
    #[data(default)]
    pub looping: bool,
    /// Its sound kept (played with it).
    #[data(default = "yes")]
    pub sound: bool,
    /// Its sound's loudness.
    #[data(default = "one")]
    pub volume: f32,
}

fn quality() -> u32 {
    QUALITY
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

/// A video's id from its file's path under `resources/` and the mod's id;
/// none for a file outside `resources/video/`.
#[must_use]
pub fn id_of(mod_id: &str, rel: &str) -> Option<String> {
    crate::resource_id(mod_id, rel, DIR, yog_ids::Format::Video)
}

/// The sound event of video `name` (its path in the mod): `video.<name>`.
#[must_use]
pub fn sound_event(name: &str) -> String {
    format!("{DIR}{}{name}", yog_ids::sep::DOT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_video_is_named_by_its_path_under_video() {
        assert_eq!(id_of("m", "video/intro.yvideo").as_deref(), Some("m:intro"));
        assert_eq!(id_of("m", "sound/intro.yvideo"), None);
        assert_eq!(sound_event("intro"), "video.intro");
    }

    #[test]
    fn a_video_needs_only_its_source() {
        let v = yog_data::from_text::<Video>(r#"{ source: "@Videos.intro" }"#)
            .unwrap()
            .value;
        assert_eq!(
            (v.quality, v.sound, v.looping, v.volume),
            (QUALITY, true, false, 1.0)
        );
    }
}
