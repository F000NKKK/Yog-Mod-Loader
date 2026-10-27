//! Values the mod keeps: loaded from their store when first used, written
//! back as the world is saved — no loading and saving by hand.

mod owned;

use yog_core::BlockPos;
use yog_data::Schema;
use yog_sync::Mutex;

pub use owned::{BlockData, EntityData, PlayerData};

/// What's kept, as the saving sees it.
trait Kept: Sync {
    /// Puts what changed into its store.
    fn keep(&self);
    /// The world closed: what belongs to it is forgotten (the next world loads its own).
    fn forget(&self);
    /// `owner` is gone for good: what it kept removed.
    fn gone(&self, _owner: &Gone<'_>) {}
}

/// An owner gone for good: what it kept goes with it.
pub(super) enum Gone<'a> {
    /// An entity dead (by its uuid).
    Entity(&'a str),
    /// The block at a place broken, or another put there: its dimension, its place.
    Block(&'a str, BlockPos),
}

/// Every kept value used so far.
static KEPT: Mutex<Vec<&'static dyn Kept>> = Mutex::new(Vec::new());

fn register(kept: &'static dyn Kept) {
    let mut all = KEPT.lock();
    if !all.iter().any(|k| std::ptr::addr_eq(*k, kept)) {
        all.push(kept);
    }
}

pub(super) fn keep_all() {
    for kept in KEPT.lock().iter() {
        kept.keep();
    }
}

pub(super) fn gone(owner: &Gone<'_>) {
    for kept in KEPT.lock().iter() {
        kept.gone(owner);
    }
}

pub(super) fn forget_world() {
    for kept in KEPT.lock().iter() {
        kept.forget();
    }
}

/// A value and whether it changed since it was put in its store.
struct Slot<T> {
    value: T,
    changed: bool,
}

/// A value kept with the world (`<world>/yog-data/<mod>/world.ydata`, at
/// its key): its default where there's none yet; with no world open (a
/// client of another's server) it's kept in memory only.
pub struct WorldData<T> {
    key: &'static str,
    slot: Mutex<Option<Slot<T>>>,
}

impl<T: Schema + Default + Send + 'static> WorldData<T> {
    pub const fn new(key: &'static str) -> Self {
        WorldData {
            key,
            slot: Mutex::new(None),
        }
    }

    /// What `read` makes of the value.
    pub fn with<R>(&'static self, read: impl FnOnce(&T) -> R) -> R {
        self.at(|slot| read(&slot.value))
    }

    /// The value changed by `change` (saved with the world); what `change` returns.
    pub fn with_mut<R>(&'static self, change: impl FnOnce(&mut T) -> R) -> R {
        self.at(|slot| {
            slot.changed = true;
            change(&mut slot.value)
        })
    }

    fn at<R>(&'static self, f: impl FnOnce(&mut Slot<T>) -> R) -> R {
        register(self);
        let mut slot = self.slot.lock();
        f(slot.get_or_insert_with(|| Slot {
            value: super::world()
                .and_then(|store| store.get(self.key))
                .unwrap_or_default(),
            changed: false,
        }))
    }
}

impl<T: Schema + Default + Send + 'static> Kept for WorldData<T> {
    fn keep(&self) {
        let mut slot = self.slot.lock();
        if let Some(slot) = slot.as_mut().filter(|s| s.changed)
            && let Some(store) = super::world()
        {
            store.set(self.key, &slot.value);
            slot.changed = false;
        }
    }

    fn forget(&self) {
        *self.slot.lock() = None;
    }
}

/// A value kept for the install, whatever world (`<game>/yog-data/<mod>/game.ydata`):
/// written as it changes.
pub struct GameData<T> {
    key: &'static str,
    slot: Mutex<Option<T>>,
}

impl<T: Schema + Default + Send + 'static> GameData<T> {
    pub const fn new(key: &'static str) -> Self {
        GameData {
            key,
            slot: Mutex::new(None),
        }
    }

    /// What `read` makes of the value.
    pub fn with<R>(&'static self, read: impl FnOnce(&T) -> R) -> R {
        let mut slot = self.slot.lock();
        read(slot.get_or_insert_with(|| super::game().get_or_default(self.key)))
    }

    /// The value changed by `change`, and written.
    pub fn with_mut<R>(&'static self, change: impl FnOnce(&mut T) -> R) -> R {
        let mut slot = self.slot.lock();
        let value = slot.get_or_insert_with(|| super::game().get_or_default(self.key));
        let out = change(value);
        let store = super::game();
        store.set(self.key, value);
        if let Err(e) = store.flush() {
            crate::error!("[{}] couldn't save its data: {e}", crate::mod_id());
        }
        out
    }
}
