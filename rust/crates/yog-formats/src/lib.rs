//! The schemas of Yog's source formats (Yog Data, see `yog-data`): what
//! `yog build` reads and checks, and what Yog-IDLE's editors are generated
//! from (`shape`). Books too (`book`); `yog-book` draws them.

pub mod binding;
pub mod book;
pub mod content;
pub mod def;
pub mod manifest;
pub mod material;
pub mod particle;
pub mod recipe;
pub mod sound;
pub mod tags;
pub mod texture;
pub mod yres;
pub mod yui;

use yog_data::Diagnostic;
use yog_data::{Schema, Shape, Value};

use yog_ids::{Format, file};

/// The id of a resource of `format` living in `dir` under `resources/`,
/// from its file's path there (`sound/casting/cast.ysound`) and the mod's
/// id: its path under `dir` with `/` as `.` (`<mod id>:casting.cast`);
/// none for a file elsewhere or of another format.
#[must_use]
pub fn resource_id(mod_id: &str, rel: &str, dir: &str, format: Format) -> Option<String> {
    let path = rel
        .strip_suffix(format.ext())?
        .strip_suffix(yog_ids::sep::DOT)?
        .strip_prefix(dir)?
        .strip_prefix(yog_ids::sep::PATH)?;
    Some(yog_ids::join(
        mod_id,
        &path.replace(yog_ids::sep::PATH, yog_ids::sep::DOT),
    ))
}

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
            Format::Sound => {
                type $t = sound::SoundFile;
                $body
            }
            Format::Particle => {
                type $t = yog_registry::Particle;
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
            Format::Ui | Format::Data | Format::Mod | Format::Script | Format::Nanites => $none,
        }
    };
}

/// What files of `format` look like; `None` for one that isn't Yog Data
/// written by hand (screens, scripts, compiled data, built mods).
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

/// `value` as a file of `format` is read: what it says, every field it
/// leaves out at its default (what the game and the designers' previews
/// take); its problems when it won't read. A format without a shape: as it is.
pub fn complete(format: Format, value: &Value) -> Result<Value, Vec<Diagnostic>> {
    with_type!(format, T => yog_data::read_value::<T>(value).map(|v| v.write()), else Ok(value.clone()))
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

    #[test]
    fn a_file_completes_with_its_defaults() {
        let text = r#"{ textures: ["@texture:m:particle/cloud"] }"#;
        let value = yog_data::text::parse(text).unwrap().value;
        let full = super::complete(yog_ids::Format::Particle, &value).unwrap();
        assert!(full.get("friction").is_some() && full.get("render").is_some());
    }
}
