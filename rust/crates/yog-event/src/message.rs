//! What crosses the bus: a payload in Yog Data's binary form, at an
//! address — an event's or a service's — or at its branch for a key.

use yog_data::Schema;
use yog_ids as ids;

/// A message the bus carries; its payload is `Self` as a frame
/// (`yog_data::frame`: packed by position — both sides have the schema).
pub trait Message: Schema + Sized {
    /// The payload's bytes.
    fn to_bytes(&self) -> Vec<u8> {
        yog_data::frame::to_frame(self)
    }

    /// The message read from a payload; `None` if it isn't one.
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        yog_data::frame::read_frame(bytes)
    }

    /// `f` on the payload, packed in this thread's reused buffers: nothing
    /// allocated once they've grown to the thread's messages.
    fn with_bytes<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        yog_data::frame::with_frame(self, f)
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


/// An action's method on the handle its first field binds — `uuid` an
/// [`crate::Entity`], `player` a [`crate::Player`], `dim` a [`crate::World`]:
/// the action's function with the handle's key put in first.
macro_rules! handle_method {
    ($name:ident, $reply:ty, [$($doc:tt)*]; uuid : $t:ty $(, $f:ident : $ft:ty)*) => {
        handle_method!(@on $crate::Entity, uuid, $name, $reply, [$($doc)*]; $($f : $ft),*);
    };
    ($name:ident, $reply:ty, [$($doc:tt)*]; player : $t:ty $(, $f:ident : $ft:ty)*) => {
        handle_method!(@on $crate::Player, name, $name, $reply, [$($doc)*]; $($f : $ft),*);
    };
    ($name:ident, $reply:ty, [$($doc:tt)*]; dim : $t:ty $(, $f:ident : $ft:ty)*) => {
        handle_method!(@on $crate::World, dim, $name, $reply, [$($doc)*]; $($f : $ft),*);
    };
    (@on $handle:path, $key:ident, $name:ident, $reply:ty, [$($doc:tt)*]; $($f:ident : $ft:ty),*) => {
        pastey::paste! {
            impl $handle {
                $($doc)*
                pub fn [<$name:snake>](&self $(, $f: impl Into<$ft>)*) -> $reply {
                    [<$name:snake>](&*self.$key $(, $f)*)
                }
            }
        }
    };
    ($($other:tt)*) => {};
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

            pastey::paste! {
                $(#[$doc])*
                #[doc = ""]
                #[doc = concat!("Calls [`", stringify!($name), "`]; what no one answers reads as the reply's default.")]
                pub fn [<$name:snake>]($( $field: impl Into<$fty> ),*) -> $reply {
                    $crate::call(&$name { $( $field: $field.into() ),* }).unwrap_or_default()
                }
            }

            handle_method! { $name, $reply, [$(#[$doc])*]; $( $field : $fty ),* }
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
