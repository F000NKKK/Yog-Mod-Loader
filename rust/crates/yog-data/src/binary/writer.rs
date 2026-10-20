//! Writing the binary form: one buffer, strings written where first used.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use super::{MAGIC, NEW_STRING, VERSION, tag, varint, zigzag};
use crate::value::Value;

/// What a writer reserves to start with: most messages fit.
const START_BYTES: usize = 256;
/// Up to this many strings are looked up by comparing them all; past it, a
/// hash index is built — a small message pays for no hashing at all.
const LINEAR_STRINGS: usize = 16;

/// Writes the binary form piece by piece, from data that isn't a [`Value`]
/// — nothing is built or copied on the way. A map or list is given its
/// length, then that many entries (`key` then a value) or values.
///
/// ```
/// let mut w = yog_data::binary::Writer::new();
/// w.map(2).key("id").str("mymod:ruby").key("count").int(3);
/// let v = yog_data::binary::decode(&w.finish()).unwrap();
/// assert_eq!(v.get("count").and_then(|c| c.as_i64()), Some(3));
/// ```
pub struct Writer {
    out: Vec<u8>,
    /// Each string written: where its bytes are in `out`, and how long.
    strings: Vec<(usize, usize)>,
    /// The strings by hash, once there are many.
    index: Option<HashMap<u64, u32, BuildHasherDefault<Fx>>>,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    pub fn new() -> Self {
        let mut out = Vec::with_capacity(START_BYTES);
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        Writer { out, strings: Vec::with_capacity(LINEAR_STRINGS), index: None }
    }

    pub fn null(&mut self) -> &mut Self {
        self.out.push(tag::NULL);
        self
    }

    pub fn bool(&mut self, b: bool) -> &mut Self {
        self.out.push(if b { tag::TRUE } else { tag::FALSE });
        self
    }

    pub fn int(&mut self, n: i64) -> &mut Self {
        self.out.push(tag::INT);
        varint(&mut self.out, zigzag(n));
        self
    }

    pub fn float(&mut self, f: f64) -> &mut Self {
        self.out.push(tag::FLOAT);
        self.out.extend_from_slice(&f.to_le_bytes());
        self
    }

    pub fn str(&mut self, s: &str) -> &mut Self {
        self.out.push(tag::STR);
        self.string(s);
        self
    }

    /// Bytes as they are.
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.out.push(tag::BYTES);
        varint(&mut self.out, b.len() as u64);
        self.out.extend_from_slice(b);
        self
    }

    /// A list of `len` values.
    pub fn list(&mut self, len: usize) -> &mut Self {
        self.out.push(tag::LIST);
        varint(&mut self.out, len as u64);
        self
    }

    /// A list of ints, packed (no tag each).
    pub fn ints(&mut self, items: impl ExactSizeIterator<Item = i64>) -> &mut Self {
        self.out.push(tag::INTS);
        varint(&mut self.out, items.len() as u64);
        for n in items {
            varint(&mut self.out, zigzag(n));
        }
        self
    }

    /// A map of `len` entries, each a [`Writer::key`] and a value.
    pub fn map(&mut self, len: usize) -> &mut Self {
        self.out.push(tag::MAP);
        varint(&mut self.out, len as u64);
        self
    }

    pub fn key(&mut self, k: &str) -> &mut Self {
        self.string(k);
        self
    }

    pub fn value(&mut self, v: &Value) -> &mut Self {
        match v {
            Value::Null => self.null(),
            Value::Bool(b) => self.bool(*b),
            Value::Int(n) => self.int(*n),
            Value::Float(f) => self.float(*f),
            Value::Str(s) => self.str(s),
            Value::List(items) => {
                self.list(items.len());
                for item in items {
                    self.value(item);
                }
                self
            }
            Value::Map(map) => {
                self.map(map.len());
                for (k, v) in map.iter() {
                    self.string(k);
                    self.value(v);
                }
                self
            }
            Value::Ref(r) | Value::Tag(r) => {
                self.out.push(if matches!(v, Value::Ref(_)) { tag::REF } else { tag::TAG });
                self.string(&r.kind);
                self.string(&r.id);
                self
            }
            Value::Res(r) => {
                self.out.push(tag::RES);
                self.string(&r.set);
                self.string(&r.key);
                self
            }
            Value::Text(s) => self.tagged(tag::TEXT, s),
            Value::Image(s) => self.tagged(tag::IMAGE, s),
            Value::Lang(s) => self.tagged(tag::LANG, s),
            Value::Color(c) => {
                self.out.push(tag::COLOR);
                self.out.extend_from_slice(&c.to_le_bytes());
                self
            }
            Value::Bytes(b) => self.bytes(b),
        }
    }

    fn tagged(&mut self, t: u8, s: &str) -> &mut Self {
        self.out.push(t);
        self.string(s);
        self
    }

    /// The bytes.
    pub fn finish(self) -> Vec<u8> {
        self.out
    }

    /// `s`: a reference if it was written before, else itself.
    fn string(&mut self, s: &str) {
        if let Some(i) = self.find(s) {
            varint(&mut self.out, (i as u64) << 1);
            return;
        }
        varint(&mut self.out, ((s.len() as u64) << 1) | NEW_STRING);
        let at = self.out.len();
        self.out.extend_from_slice(s.as_bytes());
        let i = self.strings.len();
        self.strings.push((at, s.len()));
        match &mut self.index {
            Some(index) => {
                index.entry(hash(s)).or_insert(i as u32);
            }
            None if self.strings.len() > LINEAR_STRINGS => {
                let mut index = HashMap::default();
                for (n, &(at, len)) in self.strings.iter().enumerate() {
                    index.entry(hash_bytes(&self.out[at..at + len])).or_insert(n as u32);
                }
                self.index = Some(index);
            }
            None => {}
        }
    }

    /// Which string written before `s` is, if one is.
    fn find(&self, s: &str) -> Option<usize> {
        let is = |&(at, len): &(usize, usize)| len == s.len() && &self.out[at..at + len] == s.as_bytes();
        match &self.index {
            None => self.strings.iter().position(is),
            Some(index) => index.get(&hash(s)).map(|&i| i as usize).filter(|&i| is(&self.strings[i])),
        }
    }
}

fn hash(s: &str) -> u64 {
    hash_bytes(s.as_bytes())
}

fn hash_bytes(b: &[u8]) -> u64 {
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
