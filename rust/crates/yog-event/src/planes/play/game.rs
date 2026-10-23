//! The game: the host's actions.

use yog_ids as ids;

actions! {
    /// The game's directory.
    Dir {  } -> Option<String> = ids::service_game::DIR, "YogGameActions.gameDir";
    /// The current world's directory.
    WorldDir {  } -> Option<String> = ids::service_game::WORLD_DIR, "YogGameActions.worldDir";
    /// The game's text for translation `key` in the current language.
    Translate { key: String } -> Option<String> = ids::service_game::TRANSLATE, "YogGameActions.translate";
    /// The loader's own (Java) mods.
    PlatformMods {  } -> Vec<ModInfo> = ids::service_game::PLATFORM_MODS, "YogMods.platformMods";
}

/// Every installed mod: Yog's and the loader's (the runtime answers).
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Mods {}

/// An installed mod.
#[derive(Debug, Clone, PartialEq, yog_data::Schema)]
pub struct ModInfo {
    /// Which list it's on (`yog_ids::mod_source`).
    pub source: String,
    pub id: String,
    pub name: String,
    pub version: String,
    /// Comma-separated.
    pub authors: String,
    pub description: String,
}

/// Whether mod `id` is loaded — a Yog mod or a Java one (the runtime answers).
#[derive(Debug, Clone, yog_data::Schema)]
pub struct ModLoaded {
    pub id: String,
}
