//! Poisoned locks still open.

use super::{Arc, Mutex, RwLock};

#[test]
fn a_poisoned_mutex_still_opens() {
    let m = Arc::new(Mutex::new(1));
    let held = Arc::clone(&m);
    let _ = std::thread::spawn(move || {
        let _guard = held.lock();
        panic!("poison");
    })
    .join();
    assert_eq!(*m.lock(), 1);
    assert!(m.try_lock().is_some());
}

#[test]
fn a_held_mutex_does_not_try_lock() {
    let m = Mutex::new(1);
    let _guard = m.lock();
    assert!(m.try_lock().is_none());
}

#[test]
fn a_poisoned_rwlock_still_opens() {
    let l = Arc::new(RwLock::new(2));
    let held = Arc::clone(&l);
    let _ = std::thread::spawn(move || {
        let _guard = held.write();
        panic!("poison");
    })
    .join();
    assert_eq!(*l.read(), 2);
    *l.write() = 3;
    assert_eq!(*l.read(), 3);
}
