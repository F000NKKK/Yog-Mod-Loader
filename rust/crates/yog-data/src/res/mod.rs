//! [`Res`]: an entry of a mod's `.yres` sets, `@res:Strings.title`.

use std::fmt;

use substrate_data::{Id, Reader, Schema, Shape, Value, ValueRef};

use crate::kinds;

/// An entry of a `.yres` set: the set (upper case, `Strings`) and its key
/// (`screen.title`), written `@res:Strings.screen.title`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Res {
    pub set: String,
    pub key: String,
}

impl Res {
    /// `Set.key`: what the resources list it by.
    pub fn name(&self) -> String {
        self.to_string()
    }

    /// `spelled` (`Strings.screen.title`) as an entry, if it's one.
    pub fn parse(spelled: &str) -> Option<Res> {
        let (set, key) = spelled.split_once(yog_ids::sep::DOT)?;
        let set_ok = set.starts_with(|c: char| c.is_ascii_uppercase())
            && set.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        (set_ok && is_key(key)).then(|| Res {
            set: set.to_owned(),
            key: key.to_owned(),
        })
    }
}

/// A key as `.yres` files have them: letters, digits, `_`, dotted.
fn is_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .split(yog_ids::sep::DOT)
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}

impl fmt::Display for Res {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.set, yog_ids::sep::DOT, self.key)
    }
}

impl Schema for Res {
    fn shape() -> Shape {
        substrate_data::schema::ref_shape::<kinds::Res>()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader<'_>) -> Option<Self> {
        let id = Id::<kinds::Res>::read(v, cx)?;
        Res::parse(&id.id).or_else(|| {
            cx.problem(format!(
                "`{}`: an entry is `@res:Set.key` with an upper-case set (`@res:Strings.title`)",
                id.id
            ));
            None
        })
    }
    fn write(&self) -> Value {
        Id::<kinds::Res>::new(self.to_string()).write()
    }
}
