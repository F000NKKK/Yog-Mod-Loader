//! A column's memory, owned by the store: what a [`YogColumn`] points at,
//! grown in place of readers (the old memory kept until the column goes).
//!
//! The memory is shared: readers anywhere and a writer in another mod reach
//! it through the header's pointer, so it's held as a raw allocation, never
//! as a `Box` claiming it alone.

use yog_abi::YogColumn;
use yog_sync::atomic::{AtomicPtr, AtomicU32, Ordering};

/// Rows a buffer starts with.
pub const INITIAL_ROWS: u32 = 64;
/// Bytes a word of the backing store: buffers are 8-byte aligned.
const WORD: usize = std::mem::size_of::<u64>();

/// Zeroed words, freed on drop.
struct Words {
    ptr: *mut u64,
    len: usize,
}

impl Words {
    fn zeroed(len: usize) -> Words {
        let ptr = Box::into_raw(vec![0u64; len].into_boxed_slice()).cast::<u64>();
        Words { ptr, len }
    }

    fn bytes(&self) -> usize {
        self.len * WORD
    }
}

impl Drop for Words {
    fn drop(&mut self) {
        // SAFETY: made by `zeroed` from a boxed slice of `len` words.
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(self.ptr, self.len)) });
    }
}

/// Owns a column's header and memory.
pub struct Buffer {
    raw: Box<YogColumn>,
    data: Words,
    /// Memory it grew out of: a reader may still be looking at it.
    kept: Vec<Words>,
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

fn words(width: u32, rows: u32) -> usize {
    (width as usize * rows as usize).div_ceil(WORD)
}

impl Buffer {
    /// A zeroed buffer for `rows` rows of `width` bytes.
    pub fn new(width: u32, rows: u32) -> Buffer {
        let data = Words::zeroed(words(width, rows));
        let raw = Box::new(YogColumn {
            seq: AtomicU32::new(0),
            width,
            capacity: AtomicU32::new(rows),
            generation: AtomicU32::new(0),
            data: AtomicPtr::new(data.ptr.cast()),
        });
        Buffer {
            raw,
            data,
            kept: Vec::new(),
        }
    }

    /// The header readers and writers are handed (its address never moves).
    pub fn raw(&self) -> &YogColumn {
        &self.raw
    }

    pub fn capacity(&self) -> u32 {
        self.raw.capacity.load(Ordering::Relaxed)
    }

    /// Runs `write` inside this buffer's own seqlock.
    pub fn locked<R>(&mut self, write: impl FnOnce(&mut Buffer) -> R) -> R {
        let began = crate::shared::seq::begin(&self.raw.seq);
        let r = write(self);
        crate::shared::seq::end(&self.raw.seq, began);
        r
    }

    /// Makes room for `rows` rows (doubling), the values kept. The caller
    /// holds the seqlock guarding this buffer.
    pub fn reserve(&mut self, rows: u32) {
        if rows <= self.capacity() {
            return;
        }
        let capacity = rows.next_power_of_two().max(INITIAL_ROWS);
        let data = Words::zeroed(words(self.raw.width, capacity));
        // SAFETY: both hold at least the old one's bytes; distinct allocations.
        unsafe {
            std::ptr::copy_nonoverlapping(self.data.ptr, data.ptr, self.data.len);
        }
        self.raw.data.store(data.ptr.cast(), Ordering::Release);
        self.raw.capacity.store(capacity, Ordering::Release);
        self.raw.generation.fetch_add(1, Ordering::Release);
        self.kept.push(std::mem::replace(&mut self.data, data));
    }

    /// Its bytes, `capacity * width` of them (at least).
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: `data` holds `bytes()` bytes; u8 has no alignment.
        unsafe { std::slice::from_raw_parts(self.data.ptr.cast(), self.data.bytes()) }
    }

    /// Its bytes to write; the store's writer is the only one while it
    /// holds `&mut self` (a mod writes through the header, never meanwhile).
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: as in `bytes`.
        unsafe { std::slice::from_raw_parts_mut(self.data.ptr.cast(), self.data.bytes()) }
    }

    /// Row `row`'s bytes.
    pub fn row_mut(&mut self, row: u32) -> &mut [u8] {
        let w = self.raw.width as usize;
        let at = row as usize * w;
        &mut self.bytes_mut()[at..at + w]
    }

    /// Every byte zero again (a key space rebuilding its index).
    pub fn clear(&mut self) {
        self.bytes_mut().fill(0);
    }
}
