//! Yog's sync primitives — the only ones Yog code uses. Its locks outlive a
//! panic: a thread that panicked while holding one leaves the data as it
//! last was, and the game (or a tool) goes on with it instead of failing
//! every later call — across JNI a panic would take the game down. The rest
//! is the standard library's, re-exported so every sync type comes from here.

mod mutex;
mod rwlock;

pub use mutex::Mutex;
pub use rwlock::RwLock;
pub use std::sync::{
    Arc, LazyLock, MutexGuard, Once, OnceLock, RwLockReadGuard, RwLockWriteGuard, Weak, atomic,
    mpsc,
};

#[cfg(test)]
mod tests;
