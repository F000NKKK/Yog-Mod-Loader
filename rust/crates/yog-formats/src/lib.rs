//! The schemas of Yog's source formats (Yog Data, see `yog-data`): what
//! `yog build` reads and checks, and what Yog-IDLE's editors are generated
//! from (`shape`). Books and recipe layouts are `yog-book`'s.

pub mod content;
pub mod def;
pub mod manifest;
pub mod recipe;
pub mod yres;

use yog_data::{Schema, Shape};

/// The formats `shape` describes.
pub const FORMATS: [&str; 8] = ["yblock", "yitem", "yrecipe", "ydef", "yres", "ymodel", "yanim", "yog.toml"];

/// What the format `name` (a file extension without the dot, or `yog.toml`) looks like.
pub fn shape(name: &str) -> Option<Shape> {
    Some(match name {
        "yblock" => content::BlockFile::shape(),
        "yitem" => content::ItemFile::shape(),
        "yrecipe" => recipe::RecipeFile::shape(),
        "ydef" => def::Def::shape(),
        "yres" => Shape::Map(yres::EntryObject::shape),
        "ymodel" => yog_body::format::ModelFile::shape(),
        "yanim" => yog_body::format::AnimFile::shape(),
        "yog.toml" => manifest::ManifestFile::shape(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_format_describes_itself() {
        for f in super::FORMATS {
            let d = super::shape(f).unwrap().describe();
            assert!(d.get("root").is_some(), "{f}");
        }
    }
}
