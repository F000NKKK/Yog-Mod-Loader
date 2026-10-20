//! yog-ids — the names every part of Yog shares, defined once: namespaces,
//! kinds of content, typed-value words, image-source schemes, separators and
//! Yog's own ids. The lowest crate: everything above takes these from here
//! (composing its own constants with [`concatcp!`] / [`id!`]); the IDE
//! (TypeScript) and the hosts (Java) get them generated ([`export`]).

pub use const_format::concatcp;

/// `id!(ns::YOG, "state")` → `"yog:state"`, at compile time.
#[macro_export]
macro_rules! id {
    ($ns:expr, $path:expr) => {
        $crate::concatcp!($ns, $crate::sep::NAMESPACE, $path)
    };
}

macro_rules! names {
    ($( $(#[$gdoc:meta])* $group:ident { $( $(#[$doc:meta])* $name:ident = $value:expr ),* $(,)? } )*) => {
        $(
            $(#[$gdoc])*
            pub mod $group {
                #[allow(unused_imports)] // a group composes names from the ones above
                use crate::*;
                $( $(#[$doc])* pub const $name: &str = $value; )*
            }
        )*
        /// Every name of this file, by group — what [`crate::export`] writes.
        pub const ALL: &[$crate::Group] = &[ $( (stringify!($group), &[ $( (stringify!($name), $group::$name) ),* ]) ),* ];
    };
}

mod names {
    pub mod base;
    pub mod bus;
    pub mod services;
    pub mod content;
}
mod formats;
pub mod export;
#[cfg(test)]
mod tests;

pub use formats::*;
pub use names::base::*;
pub use names::bus::*;
pub use names::services::*;
pub use names::content::*;

/// A group of names: its name, and each name with its value.
pub type Group = (&'static str, &'static [(&'static str, &'static str)]);

/// Every name, file by file — what [`export`] writes.
pub const ALL: &[&[Group]] = &[names::base::ALL, names::bus::ALL, names::services::ALL, names::content::ALL];

/// Every group of names, in order.
pub fn groups() -> impl Iterator<Item = &'static Group> {
    ALL.iter().flat_map(|file| file.iter())
}
