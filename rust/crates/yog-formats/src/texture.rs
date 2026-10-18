//! `.ytexture` — a texture: how one of the mod's pictures works in the game.
//! The picture itself is a `.yres` image (`image: @Images.block.ruby`) the
//! IDE shows as it is; this says how the build turns it into what the game
//! draws — which atlas it's packed into, how it animates and filters. Its
//! id is its path under `resources/` without the extension
//! (`resources/block/ruby.ytexture` is `<mod id>:block/ruby`), named
//! elsewhere as `@texture:<mod id>:block/ruby`. Yog Data text, like every
//! source format; the build compiles it.

use yog_data::{Res, Schema};

/// A `.ytexture`.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct TextureFile {
    /// The picture: a `.yres` image.
    pub image: Res,
    /// The atlas it's packed into (`minecraft:blocks`, `minecraft:particles`,
    /// … — see `yog_registry::Atlas`); none: bound on its own (an entity's,
    /// armor's).
    pub atlas: Option<String>,
    /// Frames stacked top to bottom, played in turn.
    pub animation: Option<Animation>,
    /// Smoothed when scaled, not kept pixelated.
    #[data(default)]
    pub blur: bool,
    /// Its edges clamped, not repeated.
    #[data(default)]
    pub clamp: bool,
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
    fn a_texture_round_trips_through_its_file_and_names_itself() {
        let t = TextureFile {
            image: Res { set: "Images".into(), key: "block.ruby".into() },
            atlas: Some("minecraft:blocks".into()),
            animation: Some(Animation { frame_time: Some(2), frames: vec![Frame { index: 1, time: Some(5) }], ..Animation::default() }),
            blur: false,
            clamp: true,
        };
        assert_eq!(yog_data::from_text::<TextureFile>(&yog_data::text::to_string_pretty(&t.write())).unwrap().value, t);
        assert_eq!(id_of("mymod", "block/ruby.ytexture").as_deref(), Some("mymod:block/ruby"));
    }
}
