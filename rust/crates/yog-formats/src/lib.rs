//! The schemas of Yog's source formats (Yog Data, see `yog-data`): what
//! `yog build` reads and checks, and what Yog-IDLE's editors are generated
//! from (`shape`). Books too (`book`); `yog-book` draws them.

pub mod binding;
pub mod book;
pub mod content;
pub mod def;
pub mod manifest;
pub mod material;
pub mod recipe;
pub mod tags;
pub mod texture;
pub mod yres;
pub mod yui;

use yog_data::Diagnostic;
use yog_data::{Schema, Shape, Value};

use yog_ids::{Format, file};

/// `$body` with `$t` the type files of `$format` read as; `$none` for a
/// format that isn't Yog Data written by hand (screens, compiled data,
/// built mods). The one list of which type each format is.
macro_rules! with_type {
    ($format:expr, $t:ident => $body:expr, else $none:expr) => {
        match $format {
            Format::Block => {
                type $t = content::BlockFile;
                $body
            }
            Format::Item => {
                type $t = content::ItemFile;
                $body
            }
            Format::Entity => {
                type $t = yog_registry::EntityDef;
                $body
            }
            Format::Recipe => {
                type $t = recipe::RecipeFile;
                $body
            }
            Format::Book => {
                type $t = book::Book;
                $body
            }
            Format::Def => {
                type $t = def::Def;
                $body
            }
            Format::Res => {
                type $t = yres::YresFile;
                $body
            }
            Format::Model => {
                type $t = yog_body::format::ModelFile;
                $body
            }
            Format::Anim => {
                type $t = yog_body::format::AnimFile;
                $body
            }
            Format::Font => {
                type $t = yog_font::Font;
                $body
            }
            Format::Texture => {
                type $t = texture::TextureFile;
                $body
            }
            Format::Material => {
                type $t = material::MaterialFile;
                $body
            }
            Format::Ui | Format::Data | Format::Mod => $none,
        }
    };
}

/// What files of `format` look like; `None` for one that isn't Yog Data
/// written by hand (screens, compiled data, built mods).
#[must_use]
pub fn shape(format: Format) -> Option<Shape> {
    with_type!(format, T => Some(T::shape()), else None)
}

/// What's wrong with `value` as a file of `format`: every problem, at its
/// path (none for a format without a shape).
#[must_use]
pub fn problems(format: Format, value: &Value) -> Vec<Diagnostic> {
    with_type!(format, T => yog_data::from_value::<T>(value).err().unwrap_or_default(), else Vec::new())
}

/// What the file `name` describes looks like: a format's extension (with or
/// without the dot) or the manifest's name.
#[must_use]
pub fn shape_of(name: &str) -> Option<Shape> {
    if name == file::MANIFEST {
        return Some(manifest::ManifestFile::shape());
    }
    shape(Format::of(name)?)
}

/// The names [`shape_of`] knows: every format with a shape, and the manifest.
pub fn names() -> impl Iterator<Item = &'static str> {
    Format::ALL
        .iter()
        .filter(|f| shape(**f).is_some())
        .map(|f| f.ext())
        .chain([file::MANIFEST])
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_format_describes_itself() {
        for name in super::names() {
            let d = super::shape_of(name).unwrap().describe();
            assert!(d.get("root").is_some(), "{name}");
        }
    }
}
