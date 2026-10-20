//! Addresses as numbers. An address is its name's atom (`yog_data::Atom`):
//! the same number in every binary of the process — the runtime, each mod,
//! the host's Java. A message type keeps its own once it's resolved, so a
//! call after the first passes a number, no string looked up.

use std::sync::atomic::{AtomicU32, Ordering};

/// An address: its name's atom number.
pub type Address = u32;

/// No address: what an empty name resolves to (`yog_abi::NO_ADDRESS`).
pub const NO_ADDRESS: Address = u32::MAX;

/// `name`'s address.
pub fn resolve(name: &str) -> Address {
    if name.is_empty() { NO_ADDRESS } else { yog_data::Atom::new(name).number() }
}

/// A message type's address, resolved the first time it's asked for.
pub struct Cached(AtomicU32);

impl Cached {
    /// Not resolved yet ([`NO_ADDRESS`] is never cached: it costs nothing to resolve).
    const UNSET: u32 = NO_ADDRESS;

    pub const fn new() -> Self {
        Cached(AtomicU32::new(Self::UNSET))
    }

    /// `name`'s address (always the same name for this cache).
    pub fn get(&self, name: &str) -> Address {
        let n = self.0.load(Ordering::Relaxed);
        if n != Self::UNSET {
            return n;
        }
        let n = resolve(name);
        self.0.store(n, Ordering::Relaxed);
        n
    }
}

impl Default for Cached {
    fn default() -> Self {
        Self::new()
    }
}

/// Where a message goes: its own address (cached), or for a key its
/// branch's, resolved each time.
pub(crate) fn of(cached: &Cached, address: &'static str, key: Option<&str>) -> Address {
    match key {
        None => cached.get(address),
        Some(key) => resolve(&crate::keyed(address, key)),
    }
}
