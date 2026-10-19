//! Poisoned locks still open.

use super::*;
use std::sync::Arc;

#[test]
fn a_poisoned_mutex_still_opens() {
    let m = Arc::new(Mutex::new(1));
    let held = Arc::clone(&m);
    let _ = std::thread::spawn(move || {
        let _guard = held.lock().unwrap();
        panic!("poison");
    })
    .join();
    assert!(m.lock().is_err());
    assert_eq!(*m.locked(), 1);
}

#[test]
fn a_poisoned_rwlock_still_opens() {
    let l = Arc::new(RwLock::new(2));
    let held = Arc::clone(&l);
    let _ = std::thread::spawn(move || {
        let _guard = held.write().unwrap();
        panic!("poison");
    })
    .join();
    assert_eq!(*l.read_locked(), 2);
    *l.write_locked() = 3;
    assert_eq!(*l.read_locked(), 3);
}
