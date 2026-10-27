//! What a read met, by what Yog makes of it: content, tags of content,
//! `.yres` entries. Every one is a reference ([`substrate_data::Read::refs`]);
//! these split them.

use substrate_data::schema::Found;
use substrate_data::{Kind, Ref};

use crate::{Res, kinds};

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
    of_kind::<kinds::Tag>(refs)
        .filter_map(|f| {
            let (kind, id) = f.value.id.as_str().split_once(yog_ids::sep::NAMESPACE)?;
            let value = TagRef {
                kind: kind.to_owned(),
                id: id.to_owned(),
            };
            Some(Found { value, path: f.path.clone() })
        })
        .collect()
}

/// The `.yres` entries in `refs`.
pub fn resources(refs: &[Found<Ref>]) -> Vec<Found<Res>> {
    of_kind::<kinds::Res>(refs)
        .filter_map(|f| {
            let value = Res::parse(f.value.id.as_str())?;
            Some(Found { value, path: f.path.clone() })
        })
        .collect()
}

/// The references of kind `K` in `refs`.
fn of_kind<K: Kind>(refs: &[Found<Ref>]) -> impl Iterator<Item = &Found<Ref>> {
    refs.iter().filter(|f| f.value.kind.as_str() == K::NAME)
}
