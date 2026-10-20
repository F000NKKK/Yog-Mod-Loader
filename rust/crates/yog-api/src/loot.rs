//! Loot: change what loot tables give — a chest's, a block's drops, a
//! mob's. [`crate::Registry::change`] with [`yog_event::world::loot::Roll`] sees every table rolled; a
//! [`LootModifier`] (a `.ydef`, or built in code) adds and scales items on
//! some tables, finishing the stacks it adds with the mod's named loot
//! functions ([`crate::Registry::loot_function`]).

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

pub use yog_registry::{Loot, LootAdd, LootModifier, LootScale, LootStack};

/// A loot function: finishes a stack a [`LootModifier`] adds (sets its custom data).
pub(crate) type LootFunction = Arc<dyn Fn(&mut LootStack, &Loot) + Send + Sync>;

/// This mod's loot functions, by name.
pub(crate) fn functions() -> &'static RwLock<HashMap<String, LootFunction>> {
    static FUNCTIONS: OnceLock<RwLock<HashMap<String, LootFunction>>> = OnceLock::new();
    FUNCTIONS.get_or_init(Default::default)
}

thread_local! {
    static SEED: Cell<u64> = Cell::new({
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        t ^ 0x9e37_79b9_7f4a_7c15
    });
}

/// A number in `[0, 1)` — what loot's chances and counts are drawn with.
pub fn random() -> f64 {
    SEED.with(|s| {
        // xorshift64*
        let mut x = s.get() | 1;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        s.set(x);
        (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    })
}

/// Applies `modifier` to `loot`, running the mod's loot functions it names.
pub(crate) fn apply(modifier: &LootModifier, loot: &mut Loot) {
    let functions = functions().read().map(|f| f.clone()).unwrap_or_default();
    modifier.apply(loot, &mut random, &mut |name, stack, loot| match functions.get(name) {
        Some(f) => f(stack, loot),
        None => crate::error!("loot modifier {}: no loot function {name} (registry.loot_function)", modifier.id),
    });
}
