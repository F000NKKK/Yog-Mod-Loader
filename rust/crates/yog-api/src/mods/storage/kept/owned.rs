//! Values kept for each owner — the world, a player, a block —
//! in the owner's store: the Rust side of a Yog Script object's `saved`
//! and `synced` members (`docs/design/storage.md`), at the same keys.

use std::collections::HashMap;
use std::hash::Hash;

use yog_core::BlockPos;
use yog_data::Schema;
use yog_ids::storage::SEP;
use yog_ids::units::{CHUNK_BITS, CHUNK_MASK};
use yog_sync::Mutex;

use super::synced::{self, Audience, Outgoing};
use super::{Gone, Kept, register};
use crate::storage::Store;

/// What a kind of owner is to its values: where an owner's value is (its
/// store, its key there), the owner a [`Gone`] names, the owner as a
/// synced row names it.
struct Owners<K> {
    place: fn(&K, &str) -> Option<(Store, String)>,
    whose: fn(&Gone<'_>) -> Option<K>,
    text: fn(&K) -> String,
}

/// A value, whether it changed since it was put in its store, and since it
/// was sent.
struct Slot<T> {
    value: T,
    changed: bool,
    unsent: bool,
}

/// What an old save called a value (Yog Script's `was`): its key then,
/// and what reads it from a store as the value is now.
pub struct Was<T> {
    pub(super) key: &'static str,
    read: fn(&Store, &str) -> Option<T>,
}

impl<T: Schema> Was<T> {
    /// The old value at `key` of `store`, as the value is now.
    pub(super) fn read(&self, store: &Store, key: &str) -> Option<T> {
        (self.read)(store, key)
    }

    /// The value under `key` before, of the same type.
    pub const fn renamed(key: &'static str) -> Self {
        Self::from::<T>(key)
    }

    /// The value under `key` before, an `Old` the value is made from.
    pub const fn from<Old: Schema + Into<T>>(key: &'static str) -> Self {
        Was {
            key,
            read: read_as::<Old, T>,
        }
    }
}

/// `store`'s value at `key` as an `Old`, made a `T`.
fn read_as<Old: Schema + Into<T>, T>(store: &Store, key: &str) -> Option<T> {
    store.get::<Old>(key).map(Into::into)
}

/// A value kept for each owner `K`: loaded from the owner's store as it's
/// first used, written back as the world is saved, removed as the owner
/// goes; sent to who sees it when `synced`.
struct Owned<K: 'static, T> {
    key: &'static str,
    slots: Mutex<Option<HashMap<K, Slot<T>>>>,
    owners: &'static Owners<K>,
    synced: Option<Audience>,
    was: Option<Was<T>>,
}

impl<K: Hash + Eq + Clone + Send + Sync + 'static, T: Schema + Default + Send + 'static>
    Owned<K, T>
{
    const fn new(key: &'static str, owners: &'static Owners<K>, synced: Option<Audience>) -> Self {
        Owned {
            key,
            slots: Mutex::new(None),
            owners,
            synced,
            was: None,
        }
    }

    fn with<R>(&'static self, owner: K, read: impl FnOnce(&T) -> R) -> R {
        self.at(owner, |slot| read(&slot.value))
    }

    fn with_mut<R>(&'static self, owner: K, change: impl FnOnce(&mut T) -> R) -> R {
        self.at(owner, |slot| {
            slot.changed = true;
            slot.unsent = true;
            change(&mut slot.value)
        })
    }

    fn at<R>(&'static self, owner: K, f: impl FnOnce(&mut Slot<T>) -> R) -> R {
        register(self);
        let mut slots = self.slots.lock();
        let slot = slots
            .get_or_insert_with(HashMap::new)
            .entry(owner)
            .or_insert_with_key(|owner| {
                let (value, changed) = self.first(owner);
                Slot {
                    value,
                    changed,
                    unsent: false,
                }
            });
        f(slot)
    }

    /// `owner`'s value as it's first used: what arrived for it (a client),
    /// or what its store has — under its key, else under what an old save
    /// called it (`was`: read as it is now, the old record gone, the value
    /// to be written under its key: `changed`).
    fn first(&self, owner: &K) -> (T, bool) {
        let arrived = self
            .synced
            .and_then(|_| synced::pending(self.key, &(self.owners.text)(owner)))
            .and_then(|bytes| yog_data::read_bytes(&bytes).ok());
        if let Some(value) = arrived {
            return (value, false);
        }
        let Some((store, key)) = (self.owners.place)(owner, self.key) else {
            return (T::default(), false);
        };
        if let Some(value) = store.get(&key) {
            return (value, false);
        }
        let old = self.was.as_ref().and_then(|was| {
            let (store, old_key) = (self.owners.place)(owner, was.key)?;
            let value = was.read(&store, &old_key)?;
            store.remove(&old_key);
            Some(value)
        });
        match old {
            Some(value) => (value, true),
            None => (T::default(), false),
        }
    }
}

impl<K: Hash + Eq + Clone + Send + Sync + 'static, T: Schema + Default + Send + 'static> Kept
    for Owned<K, T>
{
    fn keep(&self) {
        let mut slots = self.slots.lock();
        for (owner, slot) in slots.iter_mut().flatten().filter(|(_, s)| s.changed) {
            if let Some((store, key)) = (self.owners.place)(owner, self.key) {
                store.set(&key, &slot.value);
                slot.changed = false;
            }
        }
    }

    fn forget(&self) {
        *self.slots.lock() = None;
    }

    fn gone(&self, owner: &Gone<'_>) {
        let Some(owner) = (self.owners.whose)(owner) else {
            return;
        };
        if let Some(slots) = self.slots.lock().as_mut() {
            slots.remove(&owner);
        }
        if let Some((store, key)) = (self.owners.place)(&owner, self.key) {
            store.remove(&key);
        }
    }

    fn sync(&self, every: bool, out: &mut dyn FnMut(Outgoing)) {
        let Some(audience) = self.synced else {
            return;
        };
        let mut slots = self.slots.lock();
        for (owner, slot) in slots
            .iter_mut()
            .flatten()
            .filter(|(_, s)| every || s.unsent)
        {
            if !every {
                slot.unsent = false;
            }
            let owner = (self.owners.text)(owner);
            out(Outgoing {
                to: matches!(audience, Audience::Owner).then(|| owner.clone()),
                owner,
                key: self.key,
                bytes: yog_data::to_bytes(&slot.value),
            });
        }
    }

    fn take(&self, key: &str, owner: &str, bytes: &[u8]) -> bool {
        if self.synced.is_none() || key != self.key {
            return false;
        }
        let Ok(value) = yog_data::read_bytes::<T>(bytes) else {
            return true;
        };
        let mut slots = self.slots.lock();
        let slot = slots
            .iter_mut()
            .flatten()
            .find(|(k, _)| (self.owners.text)(k) == owner);
        match slot {
            Some((_, slot)) => slot.value = value,
            None => return false,
        }
        true
    }
}

/// The public face of an [`Owned`] whose owners are `$owners`: taken as
/// `$owner` (`$take` makes the key), sent to `$audience` when synced.
macro_rules! owned {
    ($(#[$doc:meta])* $name:ident, $key:ty, $owner:ty, $take:expr, $owners:expr, $audience:expr) => {
        $(#[$doc])*
        pub struct $name<T>(Owned<$key, T>);

        impl<T: Schema + Default + Send + 'static> $name<T> {
            pub const fn new(key: &'static str) -> Self {
                $name(Owned::new(key, $owners, None))
            }

            /// Kept, and sent to the clients that see its owner as it
            /// changes (their reads the server's value; their writes their
            /// own until it sends the next).
            pub const fn synced(key: &'static str) -> Self {
                $name(Owned::new(key, $owners, Some($audience)))
            }

            /// What an old save called the value: read from there when
            /// its key has nothing, then kept under its key.
            pub const fn was(mut self, was: Was<T>) -> Self {
                self.0.was = Some(was);
                self
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
    /// (`<world>/yog-data/<mod>/player/<uuid>.ydata`, at its key); synced,
    /// to that player alone.
    PlayerData, String, &str, str::to_owned,
    &Owners {
        place: |uuid, key| Some((crate::storage::player(uuid)?, key.to_owned())),
        whose: |_| None,
        text: String::clone,
    },
    Audience::Owner
);

owned!(
    /// A value kept for each block, by dimension and place (its chunk's
    /// store, as `<x>_<y>_<z>.<key>` within the chunk): removed as it's
    /// broken or another is put there.
    BlockData, (String, [i32; 3]), (&str, BlockPos),
    |(dim, pos): (&str, BlockPos)| (dim.to_owned(), [pos.x, pos.y, pos.z]),
    &Owners {
        place: block_place,
        whose: |Gone::Block(dim, pos)| Some(((*dim).to_owned(), [pos.x, pos.y, pos.z])),
        text: |(dim, [x, y, z])| format!("{dim}{SEP}{x}{SEP}{y}{SEP}{z}"),
    },
    Audience::All
);

/// A block's value's place: its chunk's store, and its key there (its
/// place in the chunk, then the value's).
fn block_place((dim, [x, y, z]): &(String, [i32; 3]), key: &str) -> Option<(Store, String)> {
    let store = crate::storage::chunk(dim, x >> CHUNK_BITS, z >> CHUNK_BITS)?;
    let owner = format!("{}{SEP}{y}{SEP}{}", x & CHUNK_MASK, z & CHUNK_MASK);
    Some((store, yog_event::storage::record_key(&owner, key)))
}

/// The world's owners: the one world.
static WORLD: Owners<()> = Owners {
    place: |(), key| Some((crate::storage::world()?, key.to_owned())),
    whose: |_| None,
    text: |()| String::new(),
};

/// A value kept with the world (`<world>/yog-data/<mod>/world.ydata`, at
/// its key): its default where there's none yet; with no world open (a
/// client of another's server) it's kept in memory only. Synced, to everyone.
pub struct WorldData<T>(Owned<(), T>);

impl<T: Schema + Default + Send + 'static> WorldData<T> {
    pub const fn new(key: &'static str) -> Self {
        WorldData(Owned::new(key, &WORLD, None))
    }

    /// Kept, and sent to every client as it changes.
    pub const fn synced(key: &'static str) -> Self {
        WorldData(Owned::new(key, &WORLD, Some(Audience::All)))
    }

    /// What an old save called the value: read from there when its key
    /// has nothing, then kept under its key.
    pub const fn was(mut self, was: Was<T>) -> Self {
        self.0.was = Some(was);
        self
    }

    /// What `read` makes of the value.
    pub fn with<R>(&'static self, read: impl FnOnce(&T) -> R) -> R {
        self.0.with((), read)
    }

    /// The value changed by `change` (saved with the world); what `change` returns.
    pub fn with_mut<R>(&'static self, change: impl FnOnce(&mut T) -> R) -> R {
        self.0.with_mut((), change)
    }
}
