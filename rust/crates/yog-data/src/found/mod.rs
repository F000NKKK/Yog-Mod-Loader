//! What a read met, by what Yog makes of it: content, tags of content,
//! `.yres` entries. Every one is a reference ([`substrate_data::Read::refs`]);
//! these split them.

use substrate_data::schema::Found;
use substrate_data::{Kind, Path, Ref};

use crate::{Meaning, Res, kinds};

/// A tag met: its content's kind (`item`) and its id (`minecraft:planks`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRef {
    pub kind: String,
    pub id: String,
}

/// The references to content in `refs`: neither tags nor `.yres` entries
/// nor translation keys nor images.
pub fn content(refs: &[Found<Ref>]) -> impl Iterator<Item = &Found<Ref>> {
    let domain = [kinds::Tag::NAME, kinds::Res::NAME, kinds::Lang::NAME, kinds::Image::NAME];
    refs.iter().filter(move |f| !domain.contains(&f.value.kind.as_str()))
}

/// The tags in `refs`.
pub fn tags(refs: &[Found<Ref>]) -> Vec<Found<TagRef>> {
    meanings(refs)
        .filter_map(|(m, path)| match m {
            Meaning::Tag(value) => Some(Found { value, path }),
            _ => None,
        })
        .collect()
}

/// The `.yres` entries in `refs`.
pub fn resources(refs: &[Found<Ref>]) -> Vec<Found<Res>> {
    meanings(refs)
        .filter_map(|(m, path)| match m {
            Meaning::Res(value) => Some(Found { value, path }),
            _ => None,
        })
        .collect()
}

/// What each of `refs` is to Yog, and where.
fn meanings(refs: &[Found<Ref>]) -> impl Iterator<Item = (Meaning, Path)> + '_ {
    refs.iter()
        .filter_map(|f| Some((Meaning::of(f.value.clone())?, f.path.clone())))
}
