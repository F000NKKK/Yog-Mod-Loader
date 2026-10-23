//! Videos (`docs/design/video.md`): a client plays a mod's `.yvideo`
//! (`resources/video/intro.yvideo` is `mymod:intro`) on anything drawn with
//! a picture — a screen, a quad in the world, an entity's or a model's
//! texture. Its frame is decoded when it's asked for, each frame it's drawn.
//! Nothing plays on a dedicated server.
//!
//! ```ignore
//! use yog_api::video::Player;
//! let intro = Player::open("mymod:intro").expect("the mod's video");
//! intro.play();
//! // each frame it's drawn:
//! if let Some(p) = intro.frame() {
//!     gfx.draw2d().image(&p.texture, rect);
//! }
//! ```

pub use yog_event::video::Picture;
use yog_event::video::{Close, Cutscene, Frame, Open, Play, Seek};

use crate::call;

/// Plays video `id` full screen over the game till its end (a cutscene);
/// `skippable`: Esc ends it early. Whether it started.
pub fn cutscene(id: &str, skippable: bool) -> bool {
    call(&Cutscene {
        id: id.to_owned(),
        skippable,
    }) == Some(true)
}

/// A player of one video, paused at its start when opened; closed when dropped.
#[derive(Debug, PartialEq, Eq)]
pub struct Player(u32);

impl Player {
    /// A player of video `id` (none: no such video, or no client).
    pub fn open(id: &str) -> Option<Player> {
        call(&Open { id: id.to_owned() }).flatten().map(Player)
    }

    /// Plays it (from its start again once it's ended).
    pub fn play(&self) {
        call(&Play {
            handle: self.0,
            playing: true,
        });
    }

    pub fn pause(&self) {
        call(&Play {
            handle: self.0,
            playing: false,
        });
    }

    /// Goes to `seconds` from its start.
    pub fn seek(&self, seconds: f64) {
        call(&Seek {
            handle: self.0,
            seconds,
        });
    }

    /// Its picture now, to draw (ask each frame it's drawn).
    pub fn frame(&self) -> Option<Picture> {
        call(&Frame { handle: self.0 }).flatten()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        call(&Close { handle: self.0 });
    }
}
