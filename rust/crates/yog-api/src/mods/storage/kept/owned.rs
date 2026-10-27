//! Values kept for each owner — a player, an entity, a block — in the
//! owner's store: the Rust side of a Yog Script object's `saved` members
//! (`docs/design/storage.md`), at the same keys.

use std::collections::HashMap;
use std::hash::Hash;

use yog_core::BlockPos;
use yog_data::Schema;
use yog_ids::storage::SEP;
use yog_ids::units::{CHUNK_BITS, CHUNK_MASK};
use yog_sync::Mutex;

use super::{Gone, Kept, Slot, register};
use crate::storage::Store;

/// Where an owner's value is: its store, and its key in it.
type Place<K> = fn(&K, &str) -> Option<(Store, String)>;

/// A value kept for each owner `K`: loaded from the owner's store as it's
/// first used, written back as the world is saved, removed as the owner goes.
struct Owned<K, T> {
    key: &'static str,
    slots: Mutex<Option<HashMap<K, Slot<T>>>>,
    place: Place<K>,
    /// The owner a [`Gone`] names, if it's one of this kind.
    whose: fn(&Gone<'_>) -> Option<K>,
}

impl<K: Hash + Eq + Clone + Send + Sync + 'static, T: Schema + Default + Send + 'static>
    Owned<K, T>
{
    const fn new(key: &'static str, place: Place<K>, whose: fn(&Gone<'_>) -> Option<K>) -> Self {
        Owned {
            key,
            slots: Mutex::new(None),
            place,
            whose,
        }
    }

    fn with<R>(&'static self, owner: K, read: impl FnOnce(&T) -> R) -> R {
        self.at(owner, |slot| read(&slot.value))
    }

    fn with_mut<R>(&'static self, owner: K, change: impl FnOnce(&mut T) -> R) -> R {
        self.at(owner, |slot| {
            slot.changed = true;
            change(&mut slot.value)
        })
    }

    fn at<R>(&'static self, owner: K, f: impl FnOnce(&mut Slot<T>) -> R) -> R {
        register(self);
        let mut slots = self.slots.lock();
        let slot = slots
            .get_or_insert_with(HashMap::new)
            .entry(owner)
            .or_insert_with_key(|owner| Slot {
                value: (self.place)(owner, self.key)
                    .and_then(|(store, key)| store.get(&key))
                    .unwrap_or_default(),
                changed: false,
            });
        f(slot)
    }
}

impl<K: Hash + Eq + Clone + Send + Sync + 'static, T: Schema + Default + Send + 'static> Kept
    for Owned<K, T>
{
    fn keep(&self) {
        let mut slots = self.slots.lock();
        for (owner, slot) in slots.iter_mut().flatten().filter(|(_, s)| s.changed) {
            if let Some((store, key)) = (self.place)(owner, self.key) {
                store.set(&key, &slot.value);
                slot.changed = false;
            }
        }
    }

    fn forget(&self) {
        *self.slots.lock() = None;
    }

    fn gone(&self, owner: &Gone<'_>) {
        let Some(owner) = (self.whose)(owner) else {
            return;
        };
        if let Some(slots) = self.slots.lock().as_mut() {
            slots.remove(&owner);
        }
        if let Some((store, key)) = (self.place)(&owner, self.key) {
            store.remove(&key);
        }
    }
}

/// The public face of an [`Owned`]: its owners taken as `$owner`, kept as `$key`.
macro_rules! owned {
    ($(#[$doc:meta])* $name:ident, $key:ty, $owner:ty, $take:expr, $place:expr, $whose:expr) => {
        $(#[$doc])*
        pub struct $name<T>(Owned<$key, T>);

        impl<T: Schema + Default + Send + 'static> $name<T> {
            pub const fn new(key: &'static str) -> Self {
                $name(Owned::new(key, $place, $whose))
            }

            /// What `read` makes of the owner's value.
            pub fn with<R>(&'static self, owner: $owner, read: impl FnOnce(&T) -> R) -> R {
                self.0.with($take(owner), read)
            }

            /// The owner's value changed by `change` (saved with the world).
            pub fn with_mut<R>(&'static self, owner: $owner, change: impl FnOnce(&mut T) -> R) -> R {
                self.0.with_mut($take(owner), change)
            }
        }
    };
}

owned!(
    /// A value kept for each player of the world, by uuid
    /// (`<world>/yog-data/<mod>/player/<uuid>.ydata`, at its key).
    PlayerData, String, &str, str::to_owned,
    |uuid, key| Some((crate::storage::player(uuid)?, key.to_owned())),
    |_| None
);

owned!(
    /// A value kept for each entity, by uuid (its own store, at its key):
    /// removed as it dies.
    EntityData, String, &str, str::to_owned,
    |uuid, key| Some((crate::storage::entity(uuid)?, key.to_owned())),
    |gone| match gone {
        Gone::Entity(uuid) => Some((*uuid).to_owned()),
        Gone::Block(..) => None,
    }
);

owned!(
    /// A value kept for each block, by dimension and place (its chunk's
    /// store, as `<x>_<y>_<z>.<key>` within the chunk): removed as it's
    /// broken or another is put there.
    BlockData, (String, [i32; 3]), (&str, BlockPos), |(dim, pos): (&str, BlockPos)| (dim.to_owned(), [pos.x, pos.y, pos.z]),
    block_place,
    |gone| match gone {
        Gone::Block(dim, pos) => Some(((*dim).to_owned(), [pos.x, pos.y, pos.z])),
        Gone::Entity(_) => None,
    }
);

/// A block's value's place: its chunk's store, and its key there (its
/// place in the chunk, then the value's).
fn block_place((dim, [x, y, z]): &(String, [i32; 3]), key: &str) -> Option<(Store, String)> {
    let store = crate::storage::chunk(dim, x >> CHUNK_BITS, z >> CHUNK_BITS)?;
    let owner = format!("{}{SEP}{y}{SEP}{}", x & CHUNK_MASK, z & CHUNK_MASK);
    Some((store, yog_event::storage::record_key(&owner, key)))
}
