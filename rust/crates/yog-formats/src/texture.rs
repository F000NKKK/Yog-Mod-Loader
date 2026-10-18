//! `.ytexture` — a texture: its picture as given (any image the build
//! reads, beside the file) and how it animates. Its id is its path under
//! `resources/` without the extension (`resources/block/ruby.ytexture` is
//! `<mod id>:block/ruby`), named elsewhere as `@texture:<mod id>:block/ruby`.
//! The build turns every one into the game's texture (a PNG, its animation
//! the game's way) — the atlases are made of them.

use yog_data::Schema;

/// A `.ytexture`.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct TextureFile {
    /// The picture, a path from this file's folder (`ruby.png`, `ruby.aseprite`).
    pub source: String,
    /// Frames stacked top to bottom, played in turn.
    pub animation: Option<Animation>,
}

/// How an animated texture plays.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Animation {
    /// Ticks each frame shows (1 if not given).
    pub frame_time: Option<u32>,
    /// Blends each frame into the next.
    #[data(default)]
    pub interpolate: bool,
    /// The frames' order and times, when not each once, top to bottom.
    #[data(default)]
    pub frames: Vec<Frame>,
    /// A frame's size, when frames aren't square (the picture's width wide).
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// One step of an animation: a frame (from 0, top to bottom), for its own time.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Frame {
    pub index: u32,
    pub time: Option<u32>,
}

impl Animation {
    /// Ticks a frame shows when nothing says otherwise.
    pub const DEFAULT_FRAME_TIME: u32 = 1;

    /// Ticks each frame shows.
    #[must_use]
    pub fn frame_time(&self) -> u32 {
        self.frame_time.unwrap_or(Self::DEFAULT_FRAME_TIME)
    }
}

/// A texture's id from its file's path under `resources/` (`block/ruby.ytexture`) and the mod's id.
#[must_use]
pub fn id_of(mod_id: &str, rel: &str) -> Option<String> {
    let path = rel.strip_suffix(yog_ids::Format::Texture.ext())?.strip_suffix('.')?;
    Some(yog_ids::join(mod_id, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_reads_and_names_itself() {
        let t = yog_data::from_text::<TextureFile>(r#"{ source: "ruby.png", animation: { frame_time: 2, frames: [{ index: 1, time: 5 }, { index: 0 }] } }"#).unwrap().value;
        let a = t.animation.unwrap();
        assert_eq!((a.frame_time(), a.frames.len(), a.frames[0].time), (2, 2, Some(5)));
        assert_eq!(id_of("mymod", "block/ruby.ytexture").as_deref(), Some("mymod:block/ruby"));
        assert_eq!(yog_data::from_text::<TextureFile>(r#"{ source: "x.png" }"#).unwrap().value.animation, None);
    }
}
