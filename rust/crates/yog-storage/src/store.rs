//! A handle to a store: typed values by key.

use std::io;
use std::path::Path;

use yog_data::Schema;
use yog_sync::{Arc, Mutex};

use crate::Scope;
use crate::shelf::{self, Shelf};

/// A mod's store of a scope: values of any [`Schema`] type by key, kept in
/// memory and written when it's flushed. Handles to one store share it.
#[derive(Clone)]
pub struct Store {
    shelf: Arc<Mutex<Shelf>>,
}

impl Store {
    /// `mod_id`'s store of `scope` under `root` (the game's folder or the world's).
    pub fn open(root: &Path, mod_id: &str, scope: Scope<'_>) -> Self {
        Store {
            shelf: shelf::file(scope.path(root, mod_id)),
        }
    }

    /// `mod_id`'s session store: kept while the game runs, never written.
    pub fn session(mod_id: &str) -> Self {
        Store {
            shelf: shelf::session(mod_id),
        }
    }

    /// The value at `key`, if there's one of type `T`.
    pub fn get<T: Schema>(&self, key: &str) -> Option<T> {
        yog_data::read_value(self.shelf.lock().values.get(key)?).ok()
    }

    /// The value at `key`, or `T`'s default.
    pub fn get_or_default<T: Schema + Default>(&self, key: &str) -> T {
        self.get(key).unwrap_or_default()
    }

    /// `value` at `key`.
    pub fn set<T: Schema>(&self, key: &str, value: &T) {
        let mut shelf = self.shelf.lock();
        shelf.values.insert(key, value.write());
        shelf.dirty = true;
    }

    /// The value at `key` changed by `change` (its default when there's none); what `change` returns.
    pub fn update<T: Schema + Default, R>(&self, key: &str, change: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.get_or_default(key);
        let out = change(&mut value);
        self.set(key, &value);
        out
    }

    /// Takes the value at `key` out; whether there was one.
    pub fn remove(&self, key: &str) -> bool {
        let mut shelf = self.shelf.lock();
        let removed = shelf.values.remove(key).is_some();
        shelf.dirty |= removed;
        removed
    }

    pub fn contains(&self, key: &str) -> bool {
        self.shelf.lock().values.get(key).is_some()
    }

    /// Every key, in the order they were first set.
    pub fn keys(&self) -> Vec<String> {
        self.shelf.lock().values.keys().map(str::to_owned).collect()
    }

    /// Writes it now if it changed (else it's written with every store, see [`crate::flush_all`]).
    pub fn flush(&self) -> io::Result<()> {
        self.shelf.lock().flush()
    }
}
