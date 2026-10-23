//! A key space as its owner (the store) keeps it: rows given to keys and
//! taken back, the index readers probe, the link to a parent space.

use yog_abi::{EMPTY_SLOT, GONE_SLOT, YogKeySpace};
use yog_sync::atomic::{AtomicU32, Ordering};

use crate::columns::buffer::{Buffer, INITIAL_ROWS};
use crate::columns::cell::Cell;
use crate::shared::seq;
use crate::shared::view::{KEY_BYTES, KeySpaceRef, SLOT_BYTES, WORD_BITS, slot_of};

/// Bytes a key or a parent key takes.
const KEY_WIDTH: u32 = KEY_BYTES as u32;
/// Bytes a bitmap word takes.
pub(crate) const BIT_WORD: u32 = WORD_BITS / u8::BITS;
/// The index is rebuilt bigger before more than this share of it is taken
/// (keys and the gone ones): numerator, denominator.
const LOAD: (u32, u32) = (1, 2);

/// A key space and its memory.
pub struct KeySpace {
    raw: Box<YogKeySpace>,
    keys: Buffer,
    live: Buffer,
    index: Buffer,
    parent_keys: Option<Buffer>,
    /// Rows given back, to give again.
    free: Vec<u32>,
    /// Index slots taken by a key or a gone one.
    taken: u32,
}

pub(crate) fn bit_words(rows: u32) -> u32 {
    rows.div_ceil(WORD_BITS)
}

impl KeySpace {
    /// An empty space; part of `parent` when it has one.
    pub fn new(parent: Option<&KeySpace>) -> KeySpace {
        let keys = Buffer::new(KEY_WIDTH, INITIAL_ROWS);
        let live = Buffer::new(BIT_WORD, bit_words(INITIAL_ROWS));
        let mut index = Buffer::new(SLOT_BYTES as u32, INITIAL_ROWS * LOAD.1 / LOAD.0);
        empty_slots(&mut index);
        let parent_keys = parent.map(|_| Buffer::new(KEY_WIDTH, INITIAL_ROWS));
        let raw = Box::new(YogKeySpace {
            seq: AtomicU32::new(0),
            rows: AtomicU32::new(0),
            keys: keys.raw(),
            live: live.raw(),
            index: index.raw(),
            parent: parent.map_or(std::ptr::null(), |p| std::ptr::from_ref(&*p.raw)),
            parent_keys: parent_keys
                .as_ref()
                .map_or(std::ptr::null(), |b| std::ptr::from_ref(b.raw())),
        });
        KeySpace {
            raw,
            keys,
            live,
            index,
            parent_keys,
            free: Vec::new(),
            taken: 0,
        }
    }

    /// Its header, as readers get it.
    pub fn raw(&self) -> &YogKeySpace {
        &self.raw
    }

    pub fn view(&self) -> KeySpaceRef {
        KeySpaceRef::owned(
            &self.raw,
            [self.keys.raw(), self.live.raw(), self.index.raw()],
            self.parent_keys.as_ref().map(Buffer::raw),
        )
    }

    /// Rows its columns must hold (every column of the space grows with it).
    pub fn capacity(&self) -> u32 {
        self.keys.capacity()
    }

    /// Rows ever used.
    pub fn rows(&self) -> u32 {
        self.raw.rows.load(Ordering::Relaxed)
    }

    pub fn row_of(&self, key: u64) -> Option<u32> {
        self.view().row_of(key)
    }

    /// `key`'s row, given one if it has none (`parent_key`: its key in the
    /// parent space).
    pub fn insert(&mut self, key: u64, parent_key: Option<u64>) -> u32 {
        if let Some(row) = self.row_of(key) {
            return row;
        }
        let began = seq::begin(&self.raw.seq);
        let row = self.put_key(key, parent_key);
        seq::end(&self.raw.seq, began);
        row
    }

    /// A row given to `key` (inside a write).
    fn put_key(&mut self, key: u64, parent_key: Option<u64>) -> u32 {
        let row = self.free.pop().unwrap_or_else(|| {
            let row = self.raw.rows.load(Ordering::Relaxed);
            self.raw.rows.store(row + 1, Ordering::Release);
            row
        });
        self.keys.reserve(row + 1);
        self.live.reserve(bit_words(self.keys.capacity()));
        key.put(self.keys.row_mut(row));
        set_bit(&mut self.live, row, true);
        if let Some(parent_keys) = &mut self.parent_keys {
            parent_keys.reserve(row + 1);
            parent_key.unwrap_or_default().put(parent_keys.row_mut(row));
        }
        if (self.taken + 1) * LOAD.1 > self.index.capacity() * LOAD.0 {
            self.rebuild_index();
        }
        self.put_slot(key, row);
        row
    }

    /// Takes `key`'s row back: its row, if it had one.
    pub fn remove(&mut self, key: u64) -> Option<u32> {
        let row = self.row_of(key)?;
        let began = seq::begin(&self.raw.seq);
        self.take_key(key, row);
        seq::end(&self.raw.seq, began);
        Some(row)
    }

    /// `key`'s slot and row taken back (inside a write).
    fn take_key(&mut self, key: u64, row: u32) {
        let mask = self.index.capacity() as usize - 1;
        let mut at = slot_of(key, mask);
        while slot(&self.index, at).0 != key || slot(&self.index, at).1 != row {
            at = (at + 1) & mask;
        }
        write_slot(&mut self.index, at, key, GONE_SLOT);
        set_bit(&mut self.live, row, false);
        self.free.push(row);
    }

    /// Puts `key` at the first free slot of its probe (inside a write).
    fn put_slot(&mut self, key: u64, row: u32) {
        let mask = self.index.capacity() as usize - 1;
        let mut at = slot_of(key, mask);
        while !matches!(slot(&self.index, at).1, EMPTY_SLOT | GONE_SLOT) {
            at = (at + 1) & mask;
        }
        if slot(&self.index, at).1 == EMPTY_SLOT {
            self.taken += 1;
        }
        write_slot(&mut self.index, at, key, row);
    }

    /// The index made again, twice the live keys' room, gone slots dropped
    /// (inside a write).
    fn rebuild_index(&mut self) {
        let live: Vec<(u64, u32)> = (0..self.rows())
            .filter(|&row| bit(&self.live, row))
            .map(|row| {
                (
                    u64::take(&self.keys.bytes()[row as usize * KEY_BYTES..]),
                    row,
                )
            })
            .collect();
        let slots = ((live.len() as u32 + 1) * LOAD.1 / LOAD.0 * 2).next_power_of_two();
        self.index.reserve(slots);
        empty_slots(&mut self.index);
        self.taken = 0;
        for (key, row) in live {
            self.put_slot(key, row);
        }
    }
}

fn slot(index: &Buffer, at: usize) -> (u64, u32) {
    let b = &index.bytes()[at * SLOT_BYTES..];
    (u64::take(b), u32::take(&b[KEY_BYTES..]))
}

fn write_slot(index: &mut Buffer, at: usize, key: u64, row: u32) {
    let b = index.row_mut(at as u32);
    key.put(b);
    row.put(&mut b[KEY_BYTES..]);
}

/// Every slot empty.
fn empty_slots(index: &mut Buffer) {
    index.clear();
    for at in 0..index.capacity() as usize {
        write_slot(index, at, 0, EMPTY_SLOT);
    }
}

pub(crate) fn bit(bits: &Buffer, n: u32) -> bool {
    let w = u64::take(&bits.bytes()[(n / WORD_BITS) as usize * BIT_WORD as usize..]);
    w >> (n % WORD_BITS) & 1 == 1
}

pub(crate) fn set_bit(bits: &mut Buffer, n: u32, on: bool) {
    let word = bits.row_mut(n / WORD_BITS);
    let w = u64::take(word);
    let mask = 1u64 << (n % WORD_BITS);
    (if on { w | mask } else { w & !mask }).put(word);
}
