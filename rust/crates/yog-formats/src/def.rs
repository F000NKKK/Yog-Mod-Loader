//! `.ydef` — definitions: an entry of some kind (`ladder`, `level`,
//! `sound_group`, …), named elsewhere by reference (`@<kind>:<id>`). No
//! vocabulary is a closed list: the game's values are built-in definitions
//! of the same kinds.

use yog_data::Schema;
use yog_data::with::{Opt, RefTo};
pub use yog_registry::SoundGroup;

yog_data::kind! {
    LadderKind = "ladder",
    LevelKind = "level",
    SoundGroupKind = "sound_group",
}

/// A `.ydef`: an entry of some kind of definition, named by reference
/// (`@<kind>:<id>`) everywhere else. Kinds: `ladder` (an ordered scale),
/// `level` (a step on one) and `sound_group` (what a block sounds like).
#[derive(Schema, Debug, Clone)]
#[data(tag = "kind")]
pub enum Def {
    /// An ordered scale — tool tiers (`minecraft:tool`, built in), a mod's
    /// voltages, rarities, magic circles…
    Ladder { id: String },
    /// A step on a ladder, just above another (the bottom if none).
    Level {
        id: String,
        #[data(with = "RefTo<LadderKind>")]
        ladder: String,
        #[data(with = "Opt<RefTo<LevelKind>>")]
        above: Option<String>,
    },
    /// What blocks naming it (`sound: "@sound_group:mymod:crystal"`) sound
    /// like: five sound events and a volume and pitch. The game's groups
    /// are built-in ones (`stone`, `amethyst`…).
    SoundGroup(SoundGroup),
}

/// The game's tool tiers: `@ladder:minecraft:tool`, bottom to top (gold mines like wood).
pub const TOOL_LADDER: &str = "minecraft:tool";
pub const TOOL_LEVELS: [&str; 5] = ["wood", "stone", "iron", "diamond", "netherite"];
