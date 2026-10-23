//! The column store: key spaces, columns, the memory they're in, a row's fixed-width cell.

pub(crate) mod buffer;
pub(crate) mod cell;
pub(crate) mod keys;
pub(crate) mod store;
