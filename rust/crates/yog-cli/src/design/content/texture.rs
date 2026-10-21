//! A texture as the editors show and paint it: its picture, animation and
//! whether it's the mod's own to change.

use base64::Engine as _;
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::{texture_bytes, texture_file, texture_id};
use crate::design::project::mod_root;
use crate::serve::ServiceError;

#[derive(yog_data::Schema)]
pub struct Texture {
    /// `data:image/png;base64,...`
    pub url: String,
    /// Its `.png.mcmeta` animation, if it has one.
    pub animation: Option<Animation>,
    /// The texture is the mod's own file — the editor can paint it in place
    /// (otherwise painting copies it into the mod first).
    pub writable: bool,
}

/// A texture's animation (`.png.mcmeta`), as the game plays it: frames
/// `frame_width`×`frame_height` laid row by row over the image, shown in
/// `frames` order, each for its `time` in ticks.
#[derive(Clone, Debug, PartialEq, yog_data::Schema)]
#[data(rename_all = "camelCase")]
pub struct Animation {
    pub frame_width: u32,
    pub frame_height: u32,
    pub frames: Vec<AnimationFrame>,
    /// Blends each frame into the next (the game's `interpolate`).
    pub interpolate: bool,
}

#[derive(Clone, Debug, PartialEq, yog_data::Schema)]
pub struct AnimationFrame {
    pub index: u32,
    /// Ticks (1/20 s).
    pub time: u32,
}

impl Animation {
    /// The animation of a `size` image from its `.mcmeta` text; `None` when it has none.
    pub fn read(meta: &str, size: (u32, u32)) -> Option<Animation> {
        let doc: Value = serde_json::from_str(meta).ok()?;
        Self::of(&yog_formats::texture::Animation::from_mcmeta(&doc)?, size)
    }

    /// `a` of a `size` image, every frame resolved: its size (the game's
    /// default a square of the image's smaller side) and each frame's time.
    pub fn of(a: &yog_formats::texture::Animation, (w, h): (u32, u32)) -> Option<Animation> {
        let side = w.min(h);
        let fw = a.width.unwrap_or(side).max(1);
        let fh = a.height.unwrap_or(side).max(1);
        let count = (w / fw) * (h / fh);
        if count == 0 {
            return None;
        }
        let frame_time = a.frame_time().max(1);
        let frames = if a.frames.is_empty() {
            (0..count)
                .map(|index| AnimationFrame {
                    index,
                    time: frame_time,
                })
                .collect()
        } else {
            a.frames
                .iter()
                .filter(|f| f.index < count)
                .map(|f| AnimationFrame {
                    index: f.index,
                    time: f.time.map_or(frame_time, |t| t.max(1)),
                })
                .collect()
        };
        Some(Animation {
            frame_width: fw,
            frame_height: fh,
            frames,
            interpolate: a.interpolate,
        })
    }

    /// It as a `.ytexture` keeps it, for a `size` image: every frame listed with its time.
    pub fn to_format(&self, (w, h): (u32, u32)) -> yog_formats::texture::Animation {
        let side = w.min(h);
        yog_formats::texture::Animation {
            frame_time: None,
            interpolate: self.interpolate,
            frames: self
                .frames
                .iter()
                .map(|f| yog_formats::texture::Frame {
                    index: f.index,
                    time: Some(f.time),
                })
                .collect(),
            width: (self.frame_width != side).then_some(self.frame_width),
            height: (self.frame_height != side).then_some(self.frame_height),
        }
    }
}

/// Sets the animation of texture `texture` of the mod (`None`: still) in
/// its `.ytexture`; the file written.
pub fn write_animation(
    file: &Path,
    texture: &str,
    animation: Option<&Animation>,
) -> Result<PathBuf, String> {
    let root = mod_root(file).ok_or("not in a mod")?;
    let id = texture_id(texture);
    let animation = match animation {
        Some(a) => {
            let png = texture_bytes(file, texture).map_err(|e| e.message)?;
            let size = image_size(&png).ok_or_else(|| format!("{id} isn't a picture"))?;
            Some(a.to_format(size))
        }
        None => None,
    };
    crate::media::save_texture_animation(&root, &id, animation)
}

/// A body model's texture (`.ymodel`): `@texture:<ns>:<path>`, as a content file's.
pub fn body_texture(file: &Path, texture: &str) -> Result<Texture, ServiceError> {
    self::texture(file, texture)
}

pub fn texture(file: &Path, texture: &str) -> Result<Texture, ServiceError> {
    let bytes = texture_bytes(file, texture)?;
    let animation = image_size(&bytes).and_then(|size| {
        let meta = texture_file(file, texture, "png.mcmeta").ok()?;
        Animation::read(&String::from_utf8_lossy(&meta), size)
    });
    Ok(Texture {
        url: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        ),
        animation,
        writable: own_texture(file, texture).is_some(),
    })
}

/// The mod and the id of texture `texture` when it's the mod's own (it
/// can be painted in place, or added); `None` for the game's, another
/// mod's, a `#slot`.
pub fn own_texture(file: &Path, texture: &str) -> Option<(PathBuf, String)> {
    if texture.starts_with(yog_formats::content::SLOT_MARK) {
        return None;
    }
    let root = mod_root(file)?;
    let id = texture_id(texture);
    crate::media::is_own_texture(&root, &id).then_some((root, id))
}

/// Writes `png` as the picture of the mod's texture `texture`; the `.yres` written.
pub fn write_texture(file: &Path, texture: &str, png: Vec<u8>) -> Result<PathBuf, String> {
    if image_size(&png).is_none() {
        return Err("not a PNG image".into());
    }
    let (root, id) = own_texture(file, texture)
        .ok_or_else(|| format!("{texture} isn't a texture of this mod"))?;
    crate::media::save_texture_picture(&root, &id, png)
}

/// Width and height from a PNG header.
pub(super) fn image_size(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || &png[1..4] != b"PNG" {
        return None;
    }
    let w = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(png[20..24].try_into().ok()?);
    Some((w, h))
}
