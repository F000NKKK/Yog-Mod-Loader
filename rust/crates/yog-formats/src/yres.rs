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
    /// Images: the texture path under `assets/<mod id>/textures/`.
    pub texture: Option<String>,
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
