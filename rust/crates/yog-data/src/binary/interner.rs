//! Which strings were written already: the binary form's ([`super::Writer`],
//! its strings inline) and a frame's ([`crate::frame::Packer`], its strings
//! in a table at the end) — each string by where its bytes are in a buffer.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// Up to this many strings are looked up by comparing them all; past it, a
/// hash index is built — a small message pays for no hashing at all.
pub(crate) const LINEAR_STRINGS: usize = 16;

/// Slots of the cache of recent strings (a power of two).
const RECENT: usize = 64;

/// The strings written so far, by where their bytes are in the buffer the
/// owner writes them to.
pub(crate) struct Interner {
    /// Each string: where its bytes start, and how long.
    spans: Vec<(usize, usize)>,
    /// Once there are many: their index.
    index: Option<Box<Index>>,
}

/// The strings of an interner that has many.
struct Index {
    /// By hash.
    by_hash: HashMap<u64, u32, BuildHasherDefault<Fx>>,
    /// The last string found at each slot, by where its `&str` points (a
    /// struct's keys are the same `&'static str` every time): its index + 1.
    recent: [u32; RECENT],
}

impl Interner {
    pub(crate) fn new() -> Self {
        Interner {
            spans: Vec::with_capacity(LINEAR_STRINGS),
            index: None,
        }
    }

    /// Forgets every string, keeping what it allocated (a reused writer).
    pub(crate) fn clear(&mut self) {
        self.spans.clear();
        if let Some(index) = &mut self.index {
            index.by_hash.clear();
            index.recent = [0; RECENT];
        }
    }

    /// Which string written before (into `buf`) `s` is, if one is: compared
    /// with each while they're few; else the one last found at its slot if
    /// it's the same, else looked up by hash.
    pub(crate) fn find(&mut self, buf: &[u8], s: &str) -> Option<usize> {
        let is = |&(at, len): &(usize, usize)| len == s.len() && &buf[at..at + len] == s.as_bytes();
        let Some(index) = &mut self.index else {
            return self.spans.iter().position(is);
        };
        let slot = recent_slot(s);
        let recent = index.recent[slot] as usize;
        if recent > 0 && is(&self.spans[recent - 1]) {
            return Some(recent - 1);
        }
        let found = index
            .by_hash
            .get(&hash(s.as_bytes()))
            .map(|&i| i as usize)
            .filter(|&i| is(&self.spans[i]))?;
        index.recent[slot] = found as u32 + 1;
        Some(found)
    }

    /// `s`, just written into `buf` at `at`: its index.
    pub(crate) fn add(&mut self, buf: &[u8], s: &str, at: usize) -> usize {
        let i = self.spans.len();
        self.spans.push((at, s.len()));
        match &mut self.index {
            Some(index) => {
                index.by_hash.entry(hash(s.as_bytes())).or_insert(i as u32);
                index.recent[recent_slot(s)] = i as u32 + 1;
            }
            None if self.spans.len() > LINEAR_STRINGS => {
                let mut by_hash = HashMap::default();
                for (n, &(at, len)) in self.spans.iter().enumerate() {
                    by_hash.entry(hash(&buf[at..at + len])).or_insert(n as u32);
                }
                self.index = Some(Box::new(Index {
                    by_hash,
                    recent: [0; RECENT],
                }));
            }
            None => {}
        }
        i
    }
}

/// `s`'s slot in the cache of recent strings: where it points, and how long.
fn recent_slot(s: &str) -> usize {
    /// The low bits of an address that alignment leaves alike.
    const ADDRESS_SHIFT: u32 = 3;
    ((s.as_ptr() as usize >> ADDRESS_SHIFT) ^ s.len()) & (RECENT - 1)
}

fn hash(b: &[u8]) -> u64 {
    let mut h = Fx::default();
    h.write(b);
    h.finish()
}

/// rustc's Fx hash — the strings are short and not adversarial.
#[derive(Default)]
struct Fx(u64);

/// Fx's multiplier.
const FX_SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
const FX_ROTATE: u32 = 5;
const WORD: usize = 8;

impl Fx {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(FX_ROTATE) ^ word).wrapping_mul(FX_SEED);
    }
}

impl Hasher for Fx {
    fn write(&mut self, bytes: &[u8]) {
        let (words, rest) = bytes.as_chunks::<WORD>();
        for w in words {
            self.add(u64::from_le_bytes(*w));
        }
        if !rest.is_empty() {
            let mut b = [0u8; WORD];
            b[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(b));
        }
    }

    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}
