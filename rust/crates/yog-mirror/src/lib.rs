//! Mirrors (`docs/design/bus.md`, "Mirrors"): data read every tick kept in
//! shared memory, written by its provider at the rate asked, read by
//! anyone in place — no call, no copy.
//!
//! * [`Store`] — the runtime's: key spaces, columns, mirrors, demand.
//! * [`view`] — what everyone else holds: columns and key spaces read (and
//!   written, by a column's one writer) through their headers.
//! * [`Cell`] — what a row of a column is.

mod buffer;
mod cell;
mod keys;
mod seq;
mod spec;
mod store;
pub mod view;

pub use cell::{Cell, Kind};
pub use keys::KeySpace;
pub use spec::{Declared, Due, DueList, FieldSpec, Linked, MirrorSpec, Rate};
pub use store::{MAX_FIELDS, SpaceId, Store};
pub use view::{Bits, ColumnRef, ColumnWriter, KeySpaceRef};

#[cfg(test)]
mod tests;
