//! What crosses the bus: a payload in Yog Data's binary form, at an
//! address — an event's or a service's — or at its branch for a key.

use yog_data::Schema;
use yog_ids as ids;

/// A message the bus carries; its payload is `Self` in Yog Data's binary form.
pub trait Message: Schema + Sized {
    /// The payload's bytes.
    fn to_bytes(&self) -> Vec<u8> {
        yog_data::binary::encode(&self.write())
    }

    /// The message read from a payload; `None` if it isn't one.
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        yog_data::read_bytes(bytes).ok()
    }
}

impl<T: Schema> Message for T {}

/// `address`'s branch for `key`: where a keyed message with that key goes.
pub fn keyed(address: &str, key: &str) -> String {
    format!("{address}{}{key}", ids::sep::PATH)
}

/// Where a message with `key` (if keyed) goes: `address`, or its branch.
pub(crate) fn target(address: &str, key: Option<&str>) -> String {
    match key {
        Some(key) => keyed(address, key),
        None => address.to_owned(),
    }
}

/// Where `address` sits under `prefix`: `yog:event/player/session/join`
/// under `yog:event/` → `player/session`.
pub(crate) fn branch(address: &'static str, prefix: &str) -> &'static str {
    let path = address.strip_prefix(prefix).unwrap_or(address);
    path.rsplit_once(ids::sep::PATH).map_or("", |(branch, _)| branch)
}

/// A type path's last segment: `client::ui::Open` → `Open`.
pub(crate) fn last_segment(path: &'static str) -> &'static str {
    path.rfind(':').map_or(path, |i| path.split_at(i + 1).1.trim())
}

/// The key field's name, if an entry gives one.
macro_rules! key {
    () => {
        None
    };
    ($key:ident) => {
        Some(stringify!($key))
    };
}

/// A message's key field's value, if an entry gives one.
macro_rules! key_value {
    ($self:ident,) => {
        None
    };
    ($self:ident, $key:ident) => {
        Some(&$self.$key)
    };
}

