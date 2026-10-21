//! `.ytexture` — a texture: how one of the mod's pictures works in the game.
//! The picture itself is a `.yres` image (`image: @Images.block.ruby`) the
//! IDE shows as it is; this says how the build turns it into what the game
//! draws — which atlas it's packed into, how it animates and filters. Its
//! id is its path under `resources/` without the extension
//! (`resources/block/ruby.ytexture` is `<mod id>:block/ruby`), named
//! elsewhere as `@texture:<mod id>:block/ruby`. Yog Data text, like every
//! source format; the build compiles it.

mod consts;
use consts as mcmeta;

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
    let path = rel
        .strip_suffix(yog_ids::Format::Texture.ext())?
        .strip_suffix('.')?;
    Some(yog_ids::join(mod_id, path))
}

use serde_json::{Value, json};

impl TextureFile {
    /// The game's `.mcmeta` of the texture, when it animates or filters.
    #[must_use]
    pub fn game_mcmeta(&self) -> Option<Value> {
        let t = self;
        let mut meta = serde_json::Map::new();
        if let Some(a) = &t.animation {
            meta.insert(mcmeta::ANIMATION.into(), animation(a));
        }
        if t.blur || t.clamp {
            meta.insert(
                mcmeta::TEXTURE.into(),
                json!({ "blur": t.blur, "clamp": t.clamp }),
            );
        }
        (!meta.is_empty()).then_some(Value::Object(meta))
    }
}

/// An animation the game's way.
fn animation(a: &Animation) -> Value {
    let mut anim = serde_json::Map::new();
    anim.insert("frametime".into(), json!(a.frame_time()));
    if a.interpolate {
        anim.insert("interpolate".into(), json!(true));
    }
    if !a.frames.is_empty() {
        let frames = a.frames.iter().map(|f| match f.time {
            Some(time) => json!({ "index": f.index, "time": time }),
            None => json!(f.index),
        });
        anim.insert("frames".into(), Value::Array(frames.collect()));
    }
    if let Some(w) = a.width {
        anim.insert("width".into(), json!(w));
    }
    if let Some(h) = a.height {
        anim.insert("height".into(), json!(h));
    }
    Value::Object(anim)
}

impl Animation {
    /// The animation a game `.mcmeta` (its JSON) describes.
    #[must_use]
    pub fn from_mcmeta(mcmeta: &Value) -> Option<Animation> {
        let a = mcmeta.get(mcmeta::ANIMATION)?;
        let u32_of = |v: &Value, k: &str| {
            v.get(k)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        };
        Some(Animation {
            frame_time: u32_of(a, "frametime"),
            interpolate: a
                .get("interpolate")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            frames: a
                .get("frames")
                .and_then(Value::as_array)
                .map(|fs| {
                    fs.iter()
                        .filter_map(|f| match f.as_u64() {
                            Some(i) => Some(Frame {
                                index: u32::try_from(i).ok()?,
                                time: None,
                            }),
                            None => Some(Frame {
                                index: u32_of(f, "index")?,
                                time: u32_of(f, "time"),
                            }),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            width: u32_of(a, "width"),
            height: u32_of(a, "height"),
        })
    }
}

/// The `.yres` set a picture at `path` (`block/ruby`) goes into when the
/// IDE or an import adds it: by its first folder (`BlockImages`), the GUI's
/// in `Images`.
#[must_use]
pub fn picture_set(path: &str) -> String {
    match path.split(yog_ids::sep::PATH).next().unwrap_or_default() {
        GUI_DIR => GUI_SET.to_string(),
        top => {
            top.split('_')
                .map(|w| w[..1].to_ascii_uppercase() + &w[1..])
                .collect::<String>()
                + PICTURE_SET_SUFFIX
        }
    }
}

/// The GUI's pictures' folder and set; what the other sets' names end with.
const GUI_DIR: &str = "gui";
const GUI_SET: &str = "Images";
const PICTURE_SET_SUFFIX: &str = "Images";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_round_trips_through_its_file_and_names_itself() {
        let t = TextureFile {
            image: Res {
                set: "Images".into(),
                key: "block.ruby".into(),
            },
            atlas: Some("minecraft:blocks".into()),
            animation: Some(Animation {
                frame_time: Some(2),
                frames: vec![Frame {
                    index: 1,
                    time: Some(5),
                }],
                ..Animation::default()
            }),
            blur: false,
            clamp: true,
        };
        assert_eq!(
            yog_data::from_text::<TextureFile>(&yog_data::text::to_string_pretty(&t.write()))
                .unwrap()
                .value,
            t
        );
        assert_eq!(
            id_of("mymod", "block/ruby.ytexture").as_deref(),
            Some("mymod:block/ruby")
        );
        assert_eq!(
            (
                picture_set("block/ruby"),
                picture_set("mob_effect/x"),
                picture_set("gui/logo")
            ),
            (
                "BlockImages".into(),
                "MobEffectImages".into(),
                "Images".into()
            )
        );
        let meta = json!({ "animation": { "frametime": 2, "interpolate": true, "frames": [1, { "index": 0, "time": 5 }] } });
        let a = Animation::from_mcmeta(&meta).unwrap();
        assert_eq!(
            TextureFile {
                animation: Some(a),
                clamp: false,
                ..t
            }
            .game_mcmeta(),
            Some(meta)
        );
    }
}
