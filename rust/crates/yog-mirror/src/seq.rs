//! The seqlock every shared block is guarded by: a writer never waits, a
//! reader retries while a write is under way or happened meanwhile.
//!
//! Values themselves are plain bytes read while the writer may be writing
//! them: a torn read is thrown away by the check, never used.

use yog_sync::atomic::{AtomicU32, Ordering, fence};

/// Starts a write: `seq` goes odd. Pass what it returns to [`end`].
#[inline]
pub fn begin(seq: &AtomicU32) -> u32 {
    let s = seq.load(Ordering::Relaxed);
    seq.store(s.wrapping_add(1), Ordering::Relaxed);
    fence(Ordering::Release);
    s
}

/// Ends the write [`begin`] started: `seq` is even again, past where it was.
#[inline]
pub fn end(seq: &AtomicU32, began: u32) {
    seq.store(began.wrapping_add(2), Ordering::Release);
}

/// `read` run until no write overlapped it.
#[inline]
pub fn read<T>(seq: &AtomicU32, mut read: impl FnMut() -> T) -> T {
    loop {
        let before = seq.load(Ordering::Acquire);
        if before & 1 == 0 {
            let value = read();
            fence(Ordering::Acquire);
            if seq.load(Ordering::Relaxed) == before {
                return value;
            }
        }
        std::hint::spin_loop();
    }
}
