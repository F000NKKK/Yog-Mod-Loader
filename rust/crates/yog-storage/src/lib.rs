//! Scoped, typed, auto-flushing persistent key-value storage for Yog mods.
//!
//! # Quick start
//! ```no_run
//! use yog_storage::{Storage, Value};
//!
//! // Global store — one file per mod
//! let mut store = Storage::open("/path/to/game", "mymod");
//! store.set("motd", "Hello!");
//! store.set("spawn_x", 0i64);
//!
//! // Per-player store — one file per UUID
//! let mut ps = Storage::open_player("/path/to/game", "mymod", "player-uuid");
//! ps.set("coins", 100i64);
//! let coins = ps.get_int("coins").unwrap_or(0);
//!
//! // Auto-flushed on drop.  Call flush() explicitly for earlier persistence.
//! ```
//!
//! # File layout
//! ```text
//! <game_dir>/yog-data/<mod_id>/global.kv
//! <game_dir>/yog-data/<mod_id>/player/<uuid>.kv
//! <game_dir>/yog-data/<mod_id>/world/<dim_safe>.kv
//! <game_dir>/yog-data/<mod_id>/entity/<uuid>.kv
//! <game_dir>/yog-data/<mod_id>/chunk/<dim_safe>_<cx>_<cz>.kv
//! ```
//!
//! # File format
//! Plain text, one entry per line: `key\ttype\tvalue`.  Human-readable and
//! diff-friendly.  Lines starting with `#` are comments.  Writes are atomic
//! (write to `.kv.tmp`, then rename) so a crash mid-save leaves old data intact.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

// ── StorageScope ──────────────────────────────────────────────────────────────

/// Determines which backing file a [`Storage`] uses.
#[derive(Debug, Clone, Copy)]
pub enum StorageScope<'a> {
    /// One store shared across the entire server (the default).
    Global,
    /// Per-player store, keyed by UUID string.
    Player(&'a str),
    /// Per-dimension store, keyed by dimension id (e.g. `"minecraft:overworld"`).
    World(&'a str),
    /// Per-entity store, keyed by UUID string.
    Entity(&'a str),
    /// Per-chunk store, keyed by dimension + chunk coordinates.
    Chunk(&'a str, i32, i32),
}

// ── Value ─────────────────────────────────────────────────────────────────────

/// A typed storage value.
///
/// [`From`] is implemented for `String`, `&str`, `i64`, `i32`, `u32`, `u64`,
/// `usize`, `f64`, `f32`, `bool`, and `Vec<u8>`, so `store.set("k", 42i64)`
/// works without an explicit constructor.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    /// Raw byte sequence; stored as lowercase hex on disk.
    Bytes(Vec<u8>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        if let Value::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }

    /// Returns the integer value.  A `Float` is truncated to `i64`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    /// Returns the float value.  An `Int` is widened to `f64`.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        if let Value::Bytes(b) = self {
            Some(b)
        } else {
            None
        }
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}
impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Int(n)
    }
}
impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Int(n as i64)
    }
}
impl From<u32> for Value {
    fn from(n: u32) -> Self {
        Value::Int(n as i64)
    }
}
impl From<u64> for Value {
    fn from(n: u64) -> Self {
        Value::Int(n as i64)
    }
}
impl From<usize> for Value {
    fn from(n: usize) -> Self {
        Value::Int(n as i64)
    }
}
impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}
impl From<f32> for Value {
    fn from(f: f32) -> Self {
        Value::Float(f as f64)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
impl From<Vec<u8>> for Value {
    fn from(b: Vec<u8>) -> Self {
        Value::Bytes(b)
    }
}

// ── Storage ───────────────────────────────────────────────────────────────────

/// Typed, scoped persistent key-value store.
///
/// Mutations are buffered in memory.  The store is auto-flushed on [`Drop`]
/// if any key was mutated.  Call [`flush`](Self::flush) explicitly when you
/// need the data on disk immediately (e.g. end of a critical event handler).
pub struct Storage {
    path: PathBuf,
    data: BTreeMap<String, Value>,
    dirty: bool,
}

impl Storage {
    // ── constructors ─────────────────────────────────────────────────────────

    /// Open the **global** store for `mod_id`.
    pub fn open(game_dir: &str, mod_id: &str) -> Self {
        Self::from_path(scope_path(game_dir, mod_id, StorageScope::Global))
    }

    /// Open a store with an explicit [`StorageScope`].
    pub fn open_scoped(game_dir: &str, mod_id: &str, scope: StorageScope<'_>) -> Self {
        Self::from_path(scope_path(game_dir, mod_id, scope))
    }

    /// Open a **per-player** store (keyed by player UUID).
    pub fn open_player(game_dir: &str, mod_id: &str, player_uuid: &str) -> Self {
        Self::from_path(scope_path(
            game_dir,
            mod_id,
            StorageScope::Player(player_uuid),
        ))
    }

    /// Open a **per-dimension** store.
    pub fn open_world(game_dir: &str, mod_id: &str, dimension: &str) -> Self {
        Self::from_path(scope_path(game_dir, mod_id, StorageScope::World(dimension)))
    }

    /// Open a **per-entity** store (keyed by entity UUID).
    pub fn open_entity(game_dir: &str, mod_id: &str, entity_uuid: &str) -> Self {
        Self::from_path(scope_path(
            game_dir,
            mod_id,
            StorageScope::Entity(entity_uuid),
        ))
    }

    /// Open a **per-chunk** store.
    pub fn open_chunk(game_dir: &str, mod_id: &str, dimension: &str, cx: i32, cz: i32) -> Self {
        Self::from_path(scope_path(
            game_dir,
            mod_id,
            StorageScope::Chunk(dimension, cx, cz),
        ))
    }

    fn from_path(path: PathBuf) -> Self {
        let data = load_file(&path);
        Self {
            path,
            data,
            dirty: false,
        }
    }

    // ── read ─────────────────────────────────────────────────────────────────

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.data.get(key)?.as_str()
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.data.get(key)?.as_int()
    }

    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.data.get(key)?.as_float()
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.data.get(key)?.as_bool()
    }

    pub fn get_bytes(&self, key: &str) -> Option<&[u8]> {
        self.data.get(key)?.as_bytes()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    // ── write ─────────────────────────────────────────────────────────────────

    /// Insert or replace a value.
    ///
    /// Accepts any type that implements `Into<Value>` — `i64`, `f64`, `bool`,
    /// `&str`, `String`, `Vec<u8>`, etc.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.data.insert(key.into(), value.into());
        self.dirty = true;
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let v = self.data.remove(key);
        if v.is_some() {
            self.dirty = true;
        }
        v
    }

    pub fn clear(&mut self) {
        if !self.data.is_empty() {
            self.data.clear();
            self.dirty = true;
        }
    }

    // ── meta ─────────────────────────────────────────────────────────────────

    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.data.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Absolute path of the backing file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    // ── persistence ──────────────────────────────────────────────────────────

    /// Atomically write all data to disk.
    ///
    /// Writes to `<path>.kv.tmp` first, then renames it over `<path>`.
    /// A crash mid-write leaves the previous on-disk state intact.
    pub fn flush(&mut self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("kv.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            writeln!(f, "# yog-storage v2")?;
            for (k, v) in &self.data {
                let (typ, enc) = encode_value(v);
                writeln!(f, "{}\t{}\t{}", str_escape(k), typ, enc)?;
            }
            f.flush()?;
        }
        std::fs::rename(&tmp, &self.path)?;
        self.dirty = false;
        Ok(())
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        if self.dirty {
            let _ = self.flush();
        }
    }
}

// ── internal helpers ──────────────────────────────────────────────────────────

fn scope_path(game_dir: &str, mod_id: &str, scope: StorageScope<'_>) -> PathBuf {
    let safe_mod = mod_id.replace([':', '/'], "_");
    let base = Path::new(game_dir).join("yog-data").join(safe_mod);
    match scope {
        StorageScope::Global => base.join("global.kv"),
        StorageScope::Player(uuid) => base.join("player").join(format!("{uuid}.kv")),
        StorageScope::World(dim) => base.join("world").join(format!("{}.kv", dim_safe(dim))),
        StorageScope::Entity(uuid) => base.join("entity").join(format!("{uuid}.kv")),
        StorageScope::Chunk(dim, x, z) => base
            .join("chunk")
            .join(format!("{}_{x}_{z}.kv", dim_safe(dim))),
    }
}

fn dim_safe(dim: &str) -> String {
    dim.replace([':', '/'], "_")
}

fn load_file(path: &Path) -> BTreeMap<String, Value> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return BTreeMap::new(),
    };
    let mut map = BTreeMap::new();
    for line in io::BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.splitn(3, '\t');
        let (Some(raw_k), Some(typ), Some(raw_v)) = (cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        if let Some(v) = parse_value(typ, raw_v) {
            map.insert(str_unescape(raw_k), v);
        }
    }
    map
}

fn parse_value(typ: &str, raw: &str) -> Option<Value> {
    match typ {
        "s" => Some(Value::Str(str_unescape(raw))),
        "i" => raw.parse::<i64>().ok().map(Value::Int),
        "f" => raw.parse::<f64>().ok().map(Value::Float),
        "b" => Some(Value::Bool(raw == "1")),
        "x" => Some(Value::Bytes(hex_decode(raw))),
        _ => None,
    }
}

fn encode_value(v: &Value) -> (&'static str, String) {
    match v {
        Value::Str(s) => ("s", str_escape(s)),
        Value::Int(n) => ("i", n.to_string()),
        Value::Float(f) => ("f", f.to_string()),
        Value::Bool(b) => ("b", if *b { "1" } else { "0" }.to_string()),
        Value::Bytes(b) => ("x", hex_encode(b)),
    }
}

fn str_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '\t' => out.push_str(r"\t"),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            c => out.push(c),
        }
    }
    out
}

fn str_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some(c) => {
                    out.push('\\');
                    out.push(c);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn hex_encode(b: &[u8]) -> String {
    use std::fmt::Write as FmtWrite;
    b.iter()
        .fold(String::with_capacity(b.len() * 2), |mut s, byte| {
            let _ = write!(s, "{byte:02x}");
            s
        })
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| s.get(i..i + 2))
        .filter_map(|h| u8::from_str_radix(h, 16).ok())
        .collect()
}
