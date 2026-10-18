//! The schemas of Yog's source formats (Yog Data, see `yog-data`): what
//! `yog build` reads and checks, and what Yog-IDLE's editors are generated
//! from (`shape`). Books too (`book`); `yog-book` draws them.

pub mod book;
pub mod content;
pub mod def;
pub mod manifest;
pub mod recipe;
pub mod tags;
pub mod yres;
pub mod yui;

use yog_data::{Schema, Shape};

use yog_ids::{Format, file};

/// What files of `format` look like; `None` for one that isn't Yog Data
/// written by hand (screens, compiled data, built mods).
#[must_use]
pub fn shape(format: Format) -> Option<Shape> {
    Some(match format {
        Format::Block => content::BlockFile::shape(),
        Format::Item => content::ItemFile::shape(),
        Format::Entity => yog_registry::EntityDef::shape(),
        Format::Recipe => recipe::RecipeFile::shape(),
        Format::Book => book::Book::shape(),
        Format::Def => def::Def::shape(),
        Format::Res => Shape::Map(yres::EntryObject::shape),
        Format::Model => yog_body::format::ModelFile::shape(),
        Format::Anim => yog_body::format::AnimFile::shape(),
        Format::Font => yog_font::Font::shape(),
        Format::Ui | Format::Data | Format::Mod => return None,
    })
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
    Format::ALL.iter().filter(|f| shape(**f).is_some()).map(|f| f.ext()).chain([file::MANIFEST])
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
