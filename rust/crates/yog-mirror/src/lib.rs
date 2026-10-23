//! Mirrors (`docs/design/bus.md`, "Mirrors"): data read every tick kept in
//! shared memory, written by its provider at the rate asked, read by
//! anyone in place — no call, no copy.
//!
//! * [`Store`] — the runtime's: key spaces, columns, mirrors, demand.
//! * [`view`] — what everyone else holds: columns and key spaces read (and
//!   written, by a column's one writer) through their headers.
//! * [`Cell`] — what a row of a column is.

mod columns;
mod shared;

pub use shared::{host, view};

mod spec;

pub use columns::cell::{Cell, Kind, NO_ATOM};
pub use columns::keys::KeySpace;
pub use columns::store::{MAX_FIELDS, SpaceId, Store};
pub use shared::view::{Bits, ColumnRef, ColumnWriter, KeySpaceRef};
pub use spec::{Clock, Declared, Due, DueList, FieldSpec, Linked, MirrorSpec, Rate, SCOPE_PACES};

#[cfg(test)]
mod tests;
