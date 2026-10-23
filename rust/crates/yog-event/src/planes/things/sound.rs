//! Sounds: the host's actions.

use crate::Point;
use yog_ids as ids;

actions! {
    /// Plays `sound` at `pos`.
    Play { dim: String, pos: Point, sound: String, volume: f32, pitch: f32 } -> bool = ids::service_sound::PLAY, "actions.YogSoundActions.playSound";
    /// Plays `sound` to `player` only.
    PlayToPlayer { player: String, sound: String, volume: f32, pitch: f32 } -> bool = ids::service_sound::PLAY_TO_PLAYER, "actions.YogSoundActions.playSoundToPlayer";
}
