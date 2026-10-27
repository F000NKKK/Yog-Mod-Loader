//! A mod's declared state in its stores (`docs/design/storage.md`): its
//! objects' records — a type's saved members, as Yog Data — read when their
//! owner loads, written as the world saves. Keyed by the mod: each mod's
//! native answers its own, over `yog-storage`.

use yog_data::Schema;
use yog_data::with::Raw;

/// Which of the mod's stores (the mod: the request's key): a scope's
/// (`yog_ids::storage`'s words: `world`, `player`, `chunk`, …) and its
/// instance (a uuid, a dimension, a chunk's `<dim>_<cx>_<cz>`; empty for
/// `game` and `world`).
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
pub struct Where {
    pub scope: String,
    pub instance: String,
}

/// The record of type `ty` (its full Yog Script name) of `owner` (a block's
/// position in its chunk; empty when the store is the owner's own).
#[derive(Debug, Clone, Schema)]
pub struct Read {
    pub mod_id: String,
    pub at: Where,
    pub owner: String,
    pub ty: String,
}

/// A record's bytes.
#[derive(Debug, Clone, Default, Schema)]
pub struct Record {
    #[data(with = "Raw")]
    pub bytes: Vec<u8>,
}

/// Writes a record: kept as its bytes, written as the world saves.
#[derive(Debug, Clone, Schema)]
pub struct Write {
    pub mod_id: String,
    pub at: Where,
    pub owner: String,
    pub ty: String,
    #[data(with = "Raw")]
    pub bytes: Vec<u8>,
}

/// Removes `owner`'s records: every type's, or `ty`'s alone.
#[derive(Debug, Clone, Schema)]
pub struct Remove {
    pub mod_id: String,
    pub at: Where,
    pub owner: String,
    pub ty: Option<String>,
}

/// The owners with records in a store (a chunk's positions).
#[derive(Debug, Clone, Schema)]
pub struct Owners {
    pub mod_id: String,
    pub at: Where,
}

/// A `synced` value as it goes to a client (on `yog_ids::yog::SYNCED`):
/// what it's of (`kind`: a Yog Script class, `mod:Class`; a Rust mod's id),
/// its owner (a player, an entity, a block; empty for the world's), its
/// key and its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
pub struct SyncedRow {
    pub kind: String,
    pub owner: String,
    pub key: String,
    #[data(with = "Raw")]
    pub bytes: Vec<u8>,
}

/// A record's key in its store: its owner's, then its type's.
#[must_use]
pub fn record_key(owner: &str, ty: &str) -> String {
    if owner.is_empty() {
        ty.to_owned()
    } else {
        format!("{owner}{}{ty}", yog_ids::sep::PATH)
    }
}

/// The prefix of every record of `owner`.
#[must_use]
pub fn owner_prefix(owner: &str) -> String {
    format!("{owner}{}", yog_ids::sep::PATH)
}
