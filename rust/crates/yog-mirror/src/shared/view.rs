//! Shared blocks seen through their headers: what a mod (or the Java
//! host, the same way) does with the addresses `yog:mirror/declare` hands
//! out. Reading never blocks; writing is its column's one writer's, on the
//! server thread.

use std::ptr::NonNull;

use yog_abi::{EMPTY_SLOT, GONE_SLOT, YogColumn, YogKeySlot, YogKeySpace};
use yog_sync::atomic::Ordering;

use crate::columns::cell::Cell;
use crate::shared::seq;

/// Bits a bitmap word holds.
pub(crate) const WORD_BITS: u32 = u64::BITS;
/// Bytes a key-space index slot takes.
pub(crate) const SLOT_BYTES: usize = std::mem::size_of::<YogKeySlot>();
/// Bytes of a slot's key (its row follows).
pub(crate) const KEY_BYTES: usize = std::mem::size_of::<u64>();
/// Fibonacci hashing's multiplier (2^64 / φ).
const HASH: u64 = 0x9E37_79B9_7F4A_7C15;

/// Where `key` starts its probe in an index of `mask + 1` slots (Java hashes alike).
#[inline]
pub fn slot_of(key: u64, mask: usize) -> usize {
    (key.wrapping_mul(HASH) >> u32::BITS) as usize & mask
}

/// A column, read or written in place.
#[derive(Copy, Clone)]
pub struct ColumnRef(NonNull<YogColumn>);

unsafe impl Send for ColumnRef {}
unsafe impl Sync for ColumnRef {}

impl ColumnRef {
    /// # Safety
    /// `raw` points at a live column header, alive as long as this is used.
    pub unsafe fn new(raw: *const YogColumn) -> Option<ColumnRef> {
        NonNull::new(raw.cast_mut()).map(ColumnRef)
    }

    pub(crate) fn of(raw: &YogColumn) -> ColumnRef {
        ColumnRef(NonNull::from(raw))
    }

    #[inline]
    fn raw(&self) -> &YogColumn {
        // SAFETY: `new`'s promise.
        unsafe { self.0.as_ref() }
    }

    pub fn as_ptr(&self) -> *const YogColumn {
        self.0.as_ptr()
    }

    pub fn width(&self) -> u32 {
        self.raw().width
    }

    /// Row `row`'s bytes, `None` past the end — with no seqlock: inside
    /// someone else's (a key space's) or the writer's own.
    #[inline]
    pub(crate) fn peek<T>(&self, row: u32, read: impl FnOnce(&[u8]) -> T) -> Option<T> {
        let raw = self.raw();
        if row >= raw.capacity.load(Ordering::Acquire) {
            return None;
        }
        let w = raw.width as usize;
        let data = raw.data.load(Ordering::Acquire);
        // SAFETY: `row < capacity`, and `data` holds `capacity * width`
        // bytes (an old buffer stays allocated while the column lives).
        Some(read(unsafe {
            std::slice::from_raw_parts(data.add(row as usize * w), w)
        }))
    }

    /// Row `row`'s value, consistent (`None` past the end).
    #[inline]
    pub fn get<T: Cell>(&self, row: u32) -> Option<T> {
        seq::read(&self.raw().seq, || self.peek(row, T::take))
    }

    /// Bit `bit` of a bit column.
    #[inline]
    pub(crate) fn peek_bit(&self, bit: u32) -> bool {
        self.peek(bit / WORD_BITS, u64::take)
            .is_some_and(|w| w >> (bit % WORD_BITS) & 1 == 1)
    }

    /// Bit `bit` of a bit column, consistent.
    #[inline]
    pub fn bit(&self, bit: u32) -> bool {
        seq::read(&self.raw().seq, || self.peek_bit(bit))
    }

    /// Starts writing (its one writer): readers retry until it's dropped.
    /// The store doesn't grow a column while it's written (both happen on
    /// the server thread), so where its rows are is read once.
    pub fn write(&self) -> ColumnWriter {
        let raw = self.raw();
        ColumnWriter {
            began: seq::begin(&raw.seq),
            col: *self,
            data: raw.data.load(Ordering::Acquire),
            capacity: raw.capacity.load(Ordering::Acquire),
            width: raw.width as usize,
        }
    }

    /// Word `word` of a bit column, consistent.
    #[inline]
    pub fn word(&self, word: u32) -> u64 {
        seq::read(&self.raw().seq, || self.peek(word, u64::take)).unwrap_or(0)
    }
}

/// A column being written; readers retry until it's dropped.
pub struct ColumnWriter {
    col: ColumnRef,
    began: u32,
    data: *mut u8,
    capacity: u32,
    width: usize,
}

impl Drop for ColumnWriter {
    #[inline]
    fn drop(&mut self) {
        seq::end(&self.col.raw().seq, self.began);
    }
}

impl ColumnWriter {
    /// Row `row`'s bytes, `None` past the end (the store's to make).
    #[inline]
    fn row(&mut self, row: u32) -> Option<&mut [u8]> {
        if row >= self.capacity {
            return None;
        }
        // SAFETY: as in `peek`; this is the column's one writer.
        Some(unsafe {
            std::slice::from_raw_parts_mut(self.data.add(row as usize * self.width), self.width)
        })
    }

    /// Sets row `row` (a row past the end is ignored).
    #[inline]
    pub fn set<T: Cell>(&mut self, row: u32, value: T) {
        if let Some(bytes) = self.row(row) {
            value.put(bytes);
        }
    }

    /// Sets bit `bit` of a bit column.
    pub fn set_bit(&mut self, bit: u32, on: bool) {
        let Some(word) = self.row(bit / WORD_BITS) else {
            return;
        };
        let mask = 1u64 << (bit % WORD_BITS);
        let old = u64::take(word);
        (if on { old | mask } else { old & !mask }).put(word);
    }
}

/// A key space, read in place.
#[derive(Copy, Clone)]
pub struct KeySpaceRef {
    raw: NonNull<YogKeySpace>,
    keys: ColumnRef,
    live: ColumnRef,
    index: ColumnRef,
    parent_keys: Option<ColumnRef>,
}

unsafe impl Send for KeySpaceRef {}
unsafe impl Sync for KeySpaceRef {}

impl KeySpaceRef {
    /// `None` for a null pointer or one whose columns aren't set.
    ///
    /// # Safety
    /// `raw` points at a live key space, alive as long as this is used.
    pub unsafe fn new(raw: *const YogKeySpace) -> Option<KeySpaceRef> {
        let ptr = NonNull::new(raw.cast_mut())?;
        // SAFETY: the caller's promise.
        let space = unsafe { ptr.as_ref() };
        // SAFETY: a key space's columns live as long as it does.
        unsafe {
            Some(KeySpaceRef {
                raw: ptr,
                keys: ColumnRef::new(space.keys)?,
                live: ColumnRef::new(space.live)?,
                index: ColumnRef::new(space.index)?,
                parent_keys: ColumnRef::new(space.parent_keys),
            })
        }
    }

    /// The store's own view of a space it holds: `[keys, live, index]`.
    pub(crate) fn owned(
        raw: &YogKeySpace,
        [keys, live, index]: [&YogColumn; 3],
        parent_keys: Option<&YogColumn>,
    ) -> KeySpaceRef {
        KeySpaceRef {
            raw: NonNull::from(raw),
            keys: ColumnRef::of(keys),
            live: ColumnRef::of(live),
            index: ColumnRef::of(index),
            parent_keys: parent_keys.map(ColumnRef::of),
        }
    }

    #[inline]
    fn raw(&self) -> &YogKeySpace {
        // SAFETY: `new`'s promise.
        unsafe { self.raw.as_ref() }
    }

    pub fn as_ptr(&self) -> *const YogKeySpace {
        self.raw.as_ptr()
    }

    /// The space it's part of.
    pub fn parent(&self) -> Option<KeySpaceRef> {
        // SAFETY: a parent outlives its children (the store frees spaces last).
        unsafe { KeySpaceRef::new(self.raw().parent) }
    }

    /// Row `row`'s place `depth` parents up (a player's row → its
    /// entity's): the space there and the row in it.
    #[inline]
    pub fn up(&self, row: u32, depth: u8) -> Option<(KeySpaceRef, u32)> {
        let (mut space, mut row) = (*self, row);
        for _ in 0..depth {
            let key = space.parent_key(row)?;
            space = space.parent()?;
            row = space.row_of(key)?;
        }
        Some((space, row))
    }

    /// `key`'s row, if it has one.
    #[inline]
    pub fn row_of(&self, key: u64) -> Option<u32> {
        seq::read(&self.raw().seq, || self.probe(key))
    }

    /// `key`'s row, inside the seqlock.
    #[inline]
    fn probe(&self, key: u64) -> Option<u32> {
        let slots = self.index.raw().capacity.load(Ordering::Acquire) as usize;
        let mask = slots.checked_sub(1)?;
        let mut at = slot_of(key, mask);
        for _ in 0..slots {
            let (k, row) = self
                .index
                .peek(at as u32, |b| (u64::take(b), u32::take(&b[KEY_BYTES..])))?;
            match row {
                EMPTY_SLOT => return None,
                GONE_SLOT => {}
                _ if k == key => return Some(row),
                _ => {}
            }
            at = (at + 1) & mask;
        }
        None
    }

    /// Row `row`'s key, if it's live.
    pub fn key_of(&self, row: u32) -> Option<u64> {
        seq::read(&self.raw().seq, || {
            self.live
                .peek_bit(row)
                .then(|| self.keys.peek(row, u64::take))
                .flatten()
        })
    }

    /// Row `row`'s key in the parent space.
    pub fn parent_key(&self, row: u32) -> Option<u64> {
        let parent_keys = self.parent_keys?;
        seq::read(&self.raw().seq, || parent_keys.peek(row, u64::take))
    }

    /// Rows ever used: live ones are among `0..rows()`.
    pub fn rows(&self) -> u32 {
        self.raw().rows.load(Ordering::Acquire)
    }

    /// The live rows and their keys.
    pub fn live(&self) -> impl Iterator<Item = (u32, u64)> + '_ {
        self.live_rows()
            .filter_map(|row| Some((row, self.key_of(row)?)))
    }

    /// Word `word` of the live bits (a bit a row).
    #[inline]
    pub fn live_word(&self, word: u32) -> u64 {
        seq::read(&self.raw().seq, || self.live.peek(word, u64::take)).unwrap_or(0)
    }

    /// The live rows: a word of the live bits read at a time.
    pub fn live_rows(&self) -> impl Iterator<Item = u32> + '_ {
        let words = self.rows().div_ceil(WORD_BITS);
        (0..words).flat_map(move |w| Bits(self.live_word(w)).map(move |b| w * WORD_BITS + b))
    }
}

/// The set bits of a word, lowest first.
pub struct Bits(pub u64);

impl Iterator for Bits {
    type Item = u32;

    #[inline]
    fn next(&mut self) -> Option<u32> {
        if self.0 == 0 {
            return None;
        }
        let b = self.0.trailing_zeros();
        self.0 &= self.0 - 1;
        Some(b)
    }
}
