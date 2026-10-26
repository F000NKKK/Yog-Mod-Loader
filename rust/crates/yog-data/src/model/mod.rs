//! The model: a tree of typed values (owned and borrowed), atoms, the
//! typed values' string forms.

pub mod atom;
pub(crate) mod borrowed;
pub(crate) mod map;
pub mod typed;
pub(crate) mod value;
