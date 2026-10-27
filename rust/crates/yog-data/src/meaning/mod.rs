//! [`Meaning`]: what a reference is to Yog.

use substrate_data::{Kind, Ref};

use crate::found::TagRef;

/// What opens a stack's data after an id (a block's state: `yog_ids::sep::STATE_OPEN`).
const DATA_OPEN: &str = "{";
use crate::{Res, kinds};

/// What a reference is to Yog: content, a tag of content, a `.yres`
/// entry, an image source or a translation key.
#[derive(Debug, Clone, PartialEq)]
pub enum Meaning {
    Content(Ref),
    Tag(TagRef),
    Res(Res),
    Image(String),
    Lang(String),
}

impl Meaning {
    /// What `r` is, if it's well formed for its kind (a tag names its
    /// content's kind, a `.yres` entry its set).
    pub fn of(r: Ref) -> Option<Meaning> {
        let (kind, id) = (r.kind.as_str(), r.id.as_str());
        Some(if kind == kinds::Tag::NAME {
            let (kind, id) = id.split_once(yog_ids::sep::NAMESPACE)?;
            Meaning::Tag(TagRef {
                kind: kind.to_owned(),
                id: id.to_owned(),
            })
        } else if kind == kinds::Res::NAME {
            Meaning::Res(Res::parse(id)?)
        } else if kind == kinds::Image::NAME {
            Meaning::Image(id.to_owned())
        } else if kind == kinds::Lang::NAME {
            Meaning::Lang(id.to_owned())
        } else {
            // A content id is the game's way (`ns:path`, lower case, a block
            // state or data after it), the game's when it names no namespace.
            let base = id.split(yog_ids::sep::STATE_OPEN).next().unwrap_or(id);
            let base = base.split(DATA_OPEN).next().unwrap_or(base);
            if !yog_ids::valid_id(base) {
                return None;
            }
            Meaning::Content(Ref::parse(kind, &yog_ids::qualify(id)).ok()?)
        })
    }
}
