//! `.yres` — a mod's texts, images, sounds, files, colors, numbers, flags.

use yog_data::Schema;

/// An entry's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
pub enum Kind {
    String,
    Image,
    File,
    Sound,
    Color,
    Number,
    Bool,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::String => "string",
            Kind::Image => "image",
            Kind::File => "file",
            Kind::Sound => "sound",
            Kind::Color => "color",
            Kind::Number => "number",
            Kind::Bool => "bool",
        }
    }
}

/// An entry's object form: `{ "type": ..., "value": ..., "comment": ..., ... }`.
#[derive(Schema)]
pub struct EntryObject {
    /// What the entry is; by default what its value is (string, number, bool).
    #[data(rename = "type")]
    pub kind: Option<Kind>,
    /// The text, path, color, number or flag.
    pub value: yog_data::Value,
    /// A note for whoever edits the file — never shipped.
    pub comment: Option<String>,
    /// Texts: the translation key instead of the default one.
    pub key: Option<String>,
    /// Sounds: the sound event name.
    pub event: Option<String>,
    /// Sounds: the subtitle's translation key.
    pub subtitle: Option<String>,
}

/// `resources/.../Strings.ru_ru.yres` → `("Strings", Some("ru_ru"))`.
pub fn split_file_name(file_name: &str) -> Option<(&str, Option<&str>)> {
    let stem = file_name.strip_suffix(yog_ids::Format::Res.ext())?.strip_suffix('.')?;
    Some(match stem.split_once('.') {
        Some((set, locale)) => (set, Some(locale)),
        None => (stem, None),
    })
}

/// Where an image entry's picture goes, under `assets/<mod id>/textures/`:
/// its file's path under `resources/` (`rel`) without `.png`. `None` for a
/// file that isn't a `.png`. (Textures the game's atlases are made of are
/// `.ytexture` files, not `.yres` images.)
#[must_use]
pub fn image_texture(rel: &str) -> Option<String> {
    rel.strip_suffix(yog_ids::res::PNG).map(str::to_string)
}
