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


/// Actions the Java host provides (`docs/design/bus.md`, stage 3): each a
/// request struct, its reply, its address and the host method answering
/// it; and the domain's `actions()` descriptors.
macro_rules! actions {
    ($( $(#[$doc:meta])* $name:ident { $( $(#[$fattr:meta])* $field:ident : $fty:ty ),* $(,)? } -> $reply:ty = $address:expr, $host:literal; )*) => {
        $(
            $(#[$doc])*
            #[derive(Debug, Clone, yog_data::Schema)]
            pub struct $name { $( $(#[$fattr])* pub $field: $fty ),* }

            impl $crate::Service for $name {
                const ADDRESS: &'static str = $address;
                const KEY: Option<&'static str> = None;
                type Reply = $reply;

                fn key(&self) -> Option<&str> {
                    None
                }
            }
        )*

        /// This domain's actions.
        pub(crate) fn actions() -> Vec<$crate::ServiceInfo> {
            vec![$($crate::ServiceInfo {
                domain: $crate::message::branch($address, yog_ids::service::PREFIX),
                name: stringify!($name),
                address: $address,
                key: None,
                shape: <$name as yog_data::Schema>::shape(),
                reply: <$reply as yog_data::Schema>::shape(),
                host: Some($host),
            }),*]
        }
    };
}
