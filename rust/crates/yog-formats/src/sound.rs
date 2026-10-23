//! `.ysound` — a sound event: what the game plays when it's played. The
//! sounds themselves are `.yres` sounds (`sound: @Sounds.abacus1`) as given
//! — any format, normalised by the build — or the game's sound files
//! (`file: "minecraft:dig/sand1"`); this says which of them play,
//! how loud and how high, and the subtitle shown. Its id is its path under
//! `resources/sound/` with `/` as `.` (`resources/sound/casting/cast.ysound`
//! is `<mod id>:casting.cast`), named elsewhere as `@sound:<id>`. Yog Data,
//! like every source format; the build writes the game's `sounds.json`.

use yog_data::{Res, Schema};

/// Where `.ysound` files live, under `resources/`.
pub const DIR: &str = yog_ids::kind::SOUND;

/// A `.ysound`.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct SoundFile {
    /// What plays: one of them, picked by weight.
    pub sounds: Vec<Variant>,
    /// The subtitle shown when it plays: a `.yres` text.
    pub subtitle: Option<Res>,
    /// Replaces the game's own sounds of the same event (another mod's, a resource pack's).
    #[data(default)]
    pub replace: bool,
}

/// One sound an event may play: the mod's own (`sound`) or one of the
/// game's (or another mod's) sound files (`file`) — one of them.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Variant {
    /// The mod's sound: a `.yres` sound.
    pub sound: Option<Res>,
    /// A sound file the game has (`minecraft:dig/sand1`, under its
    /// `sounds/`; no namespace: the game's).
    pub file: Option<String>,
    /// 1 if not given.
    pub volume: Option<f32>,
    /// 1 if not given.
    pub pitch: Option<f32>,
    /// How often it's picked against the others (1 if not given).
    pub weight: Option<u32>,
    /// Streamed, not loaded whole: long sounds (music, ambience).
    #[data(default)]
    pub stream: bool,
    /// Loaded with the resources, not when first played.
    #[data(default)]
    pub preload: bool,
    /// Blocks it carries (16 if not given).
    pub attenuation_distance: Option<u32>,
}

/// An event's id from its file's path under `resources/` (`sound/casting/cast.ysound`)
/// and the mod's id; none for a file outside `resources/sound/`.
#[must_use]
pub fn id_of(mod_id: &str, rel: &str) -> Option<String> {
    let path = rel
        .strip_suffix(yog_ids::Format::Sound.ext())?
        .strip_suffix(yog_ids::sep::DOT)?
        .strip_prefix(DIR)?
        .strip_prefix(yog_ids::sep::PATH)?;
    Some(yog_ids::join(
        mod_id,
        &path.replace(yog_ids::sep::PATH, yog_ids::sep::DOT),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_is_named_by_its_path_under_sound() {
        assert_eq!(
            id_of("hex", "sound/casting/cast.ysound").as_deref(),
            Some("hex:casting.cast")
        );
        assert_eq!(
            id_of("hex", "sound/abacus.ysound").as_deref(),
            Some("hex:abacus")
        );
        assert_eq!(id_of("hex", "block/abacus.ysound"), None);
        assert_eq!(id_of("hex", "sound/abacus.ytexture"), None);
    }
}
