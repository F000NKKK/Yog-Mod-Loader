//! The game: the host's actions.

use yog_ids as ids;

actions! {
    /// The game's directory.
    Dir {  } -> Option<String> = ids::service_game::DIR, "gameDir";
    /// The current world's directory.
    WorldDir {  } -> Option<String> = ids::service_game::WORLD_DIR, "worldDir";
}
