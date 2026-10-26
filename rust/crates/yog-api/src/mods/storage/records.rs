//! The mod's declared state, as the bus asks for it (`yog_event::storage`,
//! `docs/design/storage.md`): its Yog Script objects' records, kept in its
//! stores as their bytes. Each mod's native answers for its own.

use yog_event::storage::{Owners, Read, Record, Remove, Where, Write, owner_prefix, record_key};
use yog_ids::storage::{CHUNK, DIMENSION, ENTITY, GAME, PLAYER, SEP, SESSION, WORLD};

use super::Store;
use crate::Registry;

/// The store `at` names: none when it needs a world and none is open, or
/// isn't one.
fn store(at: &Where) -> Option<Store> {
    let i = at.instance.as_str();
    match at.scope.as_str() {
        SESSION => Some(super::session()),
        GAME => Some(super::game()),
        WORLD => super::world(),
        DIMENSION => super::dimension(i),
        PLAYER => super::player(i),
        ENTITY => super::entity(i),
        CHUNK => {
            // `<dim>_<cx>_<cz>`: the dimension may hold the separator too.
            let mut parts = i.rsplitn(3, SEP);
            let cz = parts.next()?.parse().ok()?;
            let cx = parts.next()?.parse().ok()?;
            super::chunk(parts.next()?, cx, cz)
        }
        _ => None,
    }
}

/// The owners of `keys`: each record's key up to its type, once each.
fn owners(keys: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = keys
        .into_iter()
        .filter_map(|k| k.rsplit_once(yog_ids::sep::PATH).map(|(o, _)| o.to_owned()))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Answers the storage services for this mod — registered by `export_mod!`.
#[doc(hidden)]
pub fn __provide_records(registry: &mut Registry) {
    let id = crate::mod_id();
    registry.provide_key::<Read>(id, |r, _| {
        let bytes = store(&r.at)?.record(&record_key(&r.owner, &r.ty))?;
        Some(Record { bytes })
    });
    registry.provide_key::<Write>(id, |w, _| {
        store(&w.at).is_some_and(|s| {
            s.set_record(&record_key(&w.owner, &w.ty), &w.bytes);
            true
        })
    });
    registry.provide_key::<Remove>(id, |r, _| {
        store(&r.at).is_some_and(|s| match &r.ty {
            Some(ty) => s.remove(&record_key(&r.owner, ty)),
            None => s.remove_under(&owner_prefix(&r.owner)) > 0,
        })
    });
    registry.provide_key::<Owners>(id, |o, _| {
        store(&o.at).map(|s| owners(s.keys())).unwrap_or_default()
    });
}

#[cfg(test)]
mod tests;
