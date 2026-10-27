//! Values the mod keeps: loaded from their store when first used, written
//! back as the world is saved — no loading and saving by hand.

mod owned;
pub(super) mod synced;

use yog_core::BlockPos;
use yog_data::Schema;
use yog_sync::Mutex;

pub use owned::{BlockData, EntityData, PlayerData, WorldData};

/// What's kept, as the saving sees it.
trait Kept: Sync {
    /// Puts what changed into its store.
    fn keep(&self);
    /// The world closed: what belongs to it is forgotten (the next world loads its own).
    fn forget(&self);
    /// `owner` is gone for good: what it kept removed.
    fn gone(&self, _owner: &Gone<'_>) {}
    /// Its `synced` values to send: the changed ones, or `every` one (a
    /// player joined).
    fn sync(&self, _every: bool, _out: &mut dyn FnMut(synced::Outgoing)) {}
    /// A synced value arrived (a client): taken when this keeps `key`.
    fn take(&self, _key: &str, _owner: &str, _bytes: &[u8]) -> bool {
        false
    }
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

pub(super) fn sync(every: bool, out: &mut dyn FnMut(synced::Outgoing)) {
    for kept in KEPT.lock().iter() {
        kept.sync(every, out);
    }
}

pub(super) fn take(key: &str, owner: &str, bytes: &[u8]) -> bool {
    KEPT.lock().iter().any(|kept| kept.take(key, owner, bytes))
}

pub(super) fn forget_world() {
    for kept in KEPT.lock().iter() {
        kept.forget();
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
