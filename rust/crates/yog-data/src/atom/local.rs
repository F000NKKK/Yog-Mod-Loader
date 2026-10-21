//! The table a process keeps itself: the runtime's, or a tool's.
use super::{HashMap, OnceLock, RwLock};

#[derive(Default)]
struct Names {
    numbers: HashMap<&'static str, u32>,
    names: Vec<&'static str>,
}

static NAMES: OnceLock<RwLock<Names>> = OnceLock::new();

fn names() -> &'static RwLock<Names> {
    NAMES.get_or_init(Default::default)
}

/// `name`'s number, made the first time (its text kept for good).
pub fn intern(name: &str) -> u32 {
    if let Some(&n) = names().read().numbers.get(name) {
        return n;
    }
    let mut t = names().write();
    if let Some(&n) = t.numbers.get(name) {
        return n;
    }
    let kept: &'static str = Box::leak(name.into());
    let n = u32::try_from(t.names.len()).unwrap_or(u32::MAX);
    t.names.push(kept);
    t.numbers.insert(kept, n);
    n
}

pub fn name(n: u32) -> Option<&'static str> {
    names().read().names.get(n as usize).copied()
}
