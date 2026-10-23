//! The videos a client plays (`docs/design/video.md`; the runtime answers,
//! on a client): a player opened on a mod's video, played, paused, sought,
//! closed — and, each frame it's drawn, its picture.

/// Opens a player on video `id` (`mymod:intro`), paused at its start; its
/// handle (none: no such video, or no client).
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Open {
    pub id: String,
}

/// Plays (`playing`) or pauses player `handle`.
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Play {
    pub handle: u32,
    pub playing: bool,
}

/// Sends player `handle` to `seconds` from its video's start.
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Seek {
    pub handle: u32,
    pub seconds: f64,
}

/// Lets player `handle` go: its picture, its sound.
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Close {
    pub handle: u32,
}

/// Player `handle`'s picture now (asked for each frame it's drawn).
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Frame {
    pub handle: u32,
}

/// Plays video `id` full screen over the game till its end; `skippable`:
/// Esc ends it early. Whether it started.
#[derive(Debug, Clone, yog_data::Schema)]
pub struct Cutscene {
    pub id: String,
    pub skippable: bool,
}

/// A player's picture and where it is.
#[derive(Debug, Clone, Default, PartialEq, yog_data::Schema)]
pub struct Picture {
    /// The texture it's in: an image source, drawn like any other.
    pub texture: String,
    /// Its size, pixels.
    pub width: u32,
    pub height: u32,
    /// Seconds from its start, and its length.
    pub seconds: f64,
    pub length: f64,
    pub playing: bool,
}
