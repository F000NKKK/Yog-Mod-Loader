//! Sound groups: what a block sounds like.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::equipment::built_in;

/// What a block sounds like: the sound events it plays when broken,
/// stepped on, placed, hit while mining and fallen on. The game's groups
/// are built in ([`SoundGroup::built_in`]); a mod registers its own
/// (`Registry::register_sound_group`, or a `.ydef` of kind `sound_group`)
/// and names it in `BlockDef::sound`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct SoundGroup {
    pub id: String,
    /// Sound event ids (`minecraft:block.stone.break`, `mymod:crystal.break`).
    #[data(rename = "break")]
    pub break_sound: String,
    pub step: String,
    pub place: String,
    pub hit: String,
    pub fall: String,
    #[data(default = "one")]
    #[data(range = "0..=10")]
    pub volume: f32,
    #[data(default = "one")]
    #[data(range = "0..=10")]
    pub pitch: f32,
}

fn one() -> f32 {
    1.0
}

static BUILT_IN_SOUND_GROUPS: std::sync::LazyLock<Vec<SoundGroup>> =
    std::sync::LazyLock::new(|| {
        yog_data::built_in::<Vec<SoundGroup>>(
            "yog-registry's built-in sound groups",
            include_str!("../assets/sound_groups.ydata"),
        )
    });

impl SoundGroup {
    /// `mymod:crystal`, its sounds `mymod:crystal.break`, `.step`, `.place`,
    /// `.hit`, `.fall`; volume and pitch 1.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        let ev = |s: &str| format!("{id}.{s}");
        Self {
            break_sound: ev("break"),
            step: ev("step"),
            place: ev("place"),
            hit: ev("hit"),
            fall: ev("fall"),
            volume: 1.0,
            pitch: 1.0,
            id,
        }
    }

    /// The five sound events: break, step, place, hit, fall.
    pub fn sounds(
        mut self,
        break_sound: &str,
        step: &str,
        place: &str,
        hit: &str,
        fall: &str,
    ) -> Self {
        [self.break_sound, self.step, self.place, self.hit, self.fall] =
            [break_sound, step, place, hit, fall].map(String::from);
        self
    }

    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    pub fn pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch;
        self
    }

    /// The game's groups (`minecraft:stone`, `minecraft:amethyst`…, from
    /// Minecraft's `SoundType`), as data.
    pub fn built_ins() -> &'static [SoundGroup] {
        &BUILT_IN_SOUND_GROUPS
    }

    /// One of the game's groups, by short name or id (`stone` = `minecraft:stone`).
    pub fn built_in(name: &str) -> Option<&'static SoundGroup> {
        built_in(Self::built_ins(), |g| &g.id, name)
    }
}
