//! A map of Yog Data: entries in file order, keys unique. A small map is
//! searched by a scan, the fastest there is for few keys. Past
//! [`INDEXED_FROM`] keys, a lookup builds an index of them once, and every
//! later change keeps it current. Decoding never builds one, so a document
//! read and never searched pays nothing for it.

use std::collections::HashMap;
use std::sync::OnceLock;

use yog_sync::Arc;

use super::value::Value;

/// How many keys a map has before a lookup indexes them: below it a scan
/// beats hashing (`benches/data.rs`'s maps, `yog-storage`'s records).
const INDEXED_FROM: usize = 32;

/// Keys are shared: a decoded document allocates each distinct key once,
/// however many maps have it.
#[derive(Clone, Default)]
pub struct Map {
    entries: Vec<(Arc<str>, Value)>,
    /// Each key's position, once a lookup in a big map asked for it;
    /// boxed, so a map (and so every [`Value`]) is no bigger for it.
    index: OnceLock<Box<HashMap<Arc<str>, usize>>>,
}

impl std::fmt::Debug for Map {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Map").field(&self.entries).finish()
    }
}

impl PartialEq for Map {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Map {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            entries: Vec::with_capacity(n),
            index: OnceLock::new(),
        }
    }

    /// Where `key` is: by the index in a big map (made now if it isn't
    /// yet), by a scan in a small one.
    fn position(&self, key: &str) -> Option<usize> {
        if self.entries.len() < INDEXED_FROM {
            return self.entries.iter().position(|(k, _)| &**k == key);
        }
        let index = self.index.get_or_init(|| {
            Box::new(
                self.entries
                    .iter()
                    .enumerate()
                    .map(|(i, (k, _))| (k.clone(), i))
                    .collect(),
            )
        });
        index.get(key).copied()
    }

    /// The index, if one's made: what a change keeps current.
    fn index_mut(&mut self) -> Option<&mut HashMap<Arc<str>, usize>> {
        self.index.get_mut().map(|b| &mut **b)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.position(key).map(|i| &self.entries[i].1)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        let i = self.position(key)?;
        Some(&mut self.entries[i].1)
    }

    /// The value of the `i`-th entry.
    pub fn value_at(&self, i: usize) -> Option<&Value> {
        self.entries.get(i).map(|(_, v)| v)
    }

    /// The position of `key` among the entries.
    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.position(key)
    }

    /// Sets `key`, in place if it's there, else at the end; returns the old value.
    pub fn insert(&mut self, key: impl Into<String>, value: Value) -> Option<Value> {
        let key = key.into();
        if let Some(slot) = self.get_mut(&key) {
            return Some(std::mem::replace(slot, value));
        }
        self.push_unchecked(key.into(), value);
        None
    }

    /// Appends without checking for the key — for decoders that already did.
    pub(crate) fn push_unchecked(&mut self, key: Arc<str>, value: Value) {
        let at = self.entries.len();
        if let Some(index) = self.index_mut() {
            index.insert(key.clone(), at);
        }
        self.entries.push((key, value));
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self.position(key)?;
        let (_, value) = self.entries.remove(i);
        if let Some(index) = self.index_mut() {
            index.remove(key);
            for at in index.values_mut() {
                if *at > i {
                    *at -= 1;
                }
            }
        }
        Some(value)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (&**k, v))
    }

    pub fn keys(&self) -> impl ExactSizeIterator<Item = &str> {
        self.entries.iter().map(|(k, _)| &**k)
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = &Value> {
        self.entries.iter().map(|(_, v)| v)
    }

    /// Keeps the entries `keep` says to, in one pass; the index made again
    /// when it's next needed.
    pub fn retain(&mut self, mut keep: impl FnMut(&str, &Value) -> bool) {
        self.entries.retain(|(k, v)| keep(k, v));
        self.index = OnceLock::new();
    }
}

impl IntoIterator for Map {
    type Item = (String, Value);
    type IntoIter = std::iter::Map<
        std::vec::IntoIter<(Arc<str>, Value)>,
        fn((Arc<str>, Value)) -> (String, Value),
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter().map(|(k, v)| (k.to_string(), v))
    }
}

impl<'a> IntoIterator for &'a Map {
    type Item = (&'a str, &'a Value);
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (Arc<str>, Value)>,
        fn(&'a (Arc<str>, Value)) -> (&'a str, &'a Value),
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(k, v)| (&**k, v))
    }
}

impl<K: Into<String>> FromIterator<(K, Value)> for Map {
    fn from_iter<I: IntoIterator<Item = (K, Value)>>(iter: I) -> Self {
        let mut map = Map::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

#[cfg(test)]
mod tests;
