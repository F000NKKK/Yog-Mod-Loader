//! Scoped, typed, auto-flushing persistent key-value storage for Yog mods.
//!
//! # Quick start
//! ```no_run
//! use yog_storage::{Storage, Value};
//!
//! // Global store — one file per mod
//! let mut store = Storage::open("/path/to/world", "mymod");
//! store.set("motd", "Hello!");
//! store.set("spawn_x", 0i64);
//!
//! // Per-player store — one file per UUID
//! let mut ps = Storage::open_player("/path/to/world", "mymod", "player-uuid");
//! ps.set("coins", 100i64);
//! let coins = ps.get_int("coins").unwrap_or(0);
//!
//! // Auto-flushed on drop.  Call flush() explicitly for earlier persistence.
//! ```
//!
//! # Where
//! Every store lives under a `root`: the world's directory
//! (`game::world_dir`) for what belongs to the world being played — its
//! blocks' state, its players' progress, its dimensions — or the game's
//! (`game::dir`) for what belongs to the install, whatever world
//! (settings).
//!
//! # File layout
//! ```text
//! <root>/yog-data/<mod_id>/global.ydata
//! <root>/yog-data/<mod_id>/player/<uuid>.ydata
//! <root>/yog-data/<mod_id>/world/<dim_safe>.ydata
//! <root>/yog-data/<mod_id>/entity/<uuid>.ydata
//! <root>/yog-data/<mod_id>/chunk/<dim_safe>_<cx>_<cz>.ydata
//! ```
//!
//! # File format
//! Binary Yog Data: one map, key → value. Writes are atomic (to a
//! temporary file, then renamed over the store) so a crash mid-save leaves
//! the old data intact.

use std::collections::BTreeMap;
use std::io::{self, Write};
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
    /// Raw byte sequence.
    Bytes(Vec<u8>),
}

impl Value {
    /// As Yog Data, how it's stored.
    fn to_data(&self) -> yog_data::Value {
        match self {
            Value::Str(s) => yog_data::Value::Str(s.clone()),
            Value::Int(n) => yog_data::Value::Int(*n),
            Value::Float(f) => yog_data::Value::Float(*f),
            Value::Bool(b) => yog_data::Value::Bool(*b),
            Value::Bytes(b) => yog_data::Value::Bytes(b.clone()),
        }
    }

    /// Read back from Yog Data; `None` for what a store never holds.
    fn from_data(v: &yog_data::Value) -> Option<Value> {
        Some(match v {
            yog_data::Value::Str(s) => Value::Str(s.clone()),
            yog_data::Value::Int(n) => Value::Int(*n),
            yog_data::Value::Float(f) => Value::Float(*f),
            yog_data::Value::Bool(b) => Value::Bool(*b),
            yog_data::Value::Bytes(b) => Value::Bytes(b.clone()),
            _ => return None,
        })
    }

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

    /// Open the **global** store for `mod_id` under `root` (see [the crate docs](crate)).
    pub fn open(root: &str, mod_id: &str) -> Self {
        Self::from_path(scope_path(root, mod_id, StorageScope::Global))
    }

    /// Open a store with an explicit [`StorageScope`].
    pub fn open_scoped(root: &str, mod_id: &str, scope: StorageScope<'_>) -> Self {
        Self::from_path(scope_path(root, mod_id, scope))
    }

    /// Open a **per-player** store (keyed by player UUID).
    pub fn open_player(root: &str, mod_id: &str, player_uuid: &str) -> Self {
        Self::from_path(scope_path(root, mod_id, StorageScope::Player(player_uuid)))
    }

    /// Open a **per-dimension** store.
    pub fn open_world(root: &str, mod_id: &str, dimension: &str) -> Self {
        Self::from_path(scope_path(root, mod_id, StorageScope::World(dimension)))
    }

    /// Open a **per-entity** store (keyed by entity UUID).
    pub fn open_entity(root: &str, mod_id: &str, entity_uuid: &str) -> Self {
        Self::from_path(scope_path(root, mod_id, StorageScope::Entity(entity_uuid)))
    }

    /// Open a **per-chunk** store.
    pub fn open_chunk(root: &str, mod_id: &str, dimension: &str, cx: i32, cz: i32) -> Self {
        Self::from_path(scope_path(
            root,
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
    /// Writes to a temporary file beside it first, then renames that over it.
    /// A crash mid-write leaves the previous on-disk state intact.
    pub fn flush(&mut self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension(TMP_EXT);
        let map: yog_data::Map = self
            .data
            .iter()
            .map(|(k, v)| (k.as_str(), v.to_data()))
            .collect();
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&yog_data::binary::encode(&map.into()))?;
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

/// Where a mod's stores go under the save, and how each scope's is named.
const DATA_DIR: &str = "yog-data";
const STORE_EXT: &str = yog_ids::Format::Data.ext();
/// What a store is written to before it's renamed over the old one.
const TMP_EXT: &str = yog_ids::concatcp!(yog_ids::Format::Data.ext(), yog_ids::sep::EXT, "tmp");
const GLOBAL: &str = "global";
const PLAYER_DIR: &str = "player";
const WORLD_DIR: &str = "world";
const ENTITY_DIR: &str = "entity";
const CHUNK_DIR: &str = "chunk";

fn scope_path(root: &str, mod_id: &str, scope: StorageScope<'_>) -> PathBuf {
    let safe_mod = file_safe(mod_id);
    let base = Path::new(root).join(DATA_DIR).join(safe_mod);
    let (dir, name) = match scope {
        StorageScope::Global => (None, GLOBAL.to_string()),
        StorageScope::Player(uuid) => (Some(PLAYER_DIR), uuid.to_string()),
        StorageScope::World(dim) => (Some(WORLD_DIR), dim_safe(dim)),
        StorageScope::Entity(uuid) => (Some(ENTITY_DIR), uuid.to_string()),
        StorageScope::Chunk(dim, x, z) => (Some(CHUNK_DIR), format!("{}_{x}_{z}", dim_safe(dim))),
    };
    let file = format!("{name}.{STORE_EXT}");
    match dir {
        Some(dir) => base.join(dir).join(file),
        None => base.join(file),
    }
}

/// What a name's separators become in a file name.
const SAFE_SEP: &str = "_";

/// `name` (`ns:path/x`) as one file name: its separators replaced.
fn file_safe(name: &str) -> String {
    name.replace(yog_ids::sep::NAMESPACE, SAFE_SEP)
        .replace(yog_ids::sep::PATH, SAFE_SEP)
}

fn dim_safe(dim: &str) -> String {
    file_safe(dim)
}

fn load_file(path: &Path) -> BTreeMap<String, Value> {
    let Ok(bytes) = std::fs::read(path) else {
        return BTreeMap::new();
    };
    match yog_data::binary::decode(&bytes) {
        Ok(yog_data::Value::Map(m)) => m
            .iter()
            .filter_map(|(k, v)| Value::from_data(v).map(|v| (k.to_string(), v)))
            .collect(),
        _ => BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_dir(name: &str) -> String {
        let d =
            std::env::temp_dir().join(format!("yog-storage-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.to_string_lossy().into_owned()
    }

    #[test]
    fn every_value_comes_back_after_a_flush() {
        let dir = game_dir("roundtrip");
        let odd = "tab\there\nnew line \\ backslash";
        {
            let mut s = Storage::open(&dir, "mymod");
            s.set("text", odd);
            s.set("int", -42i64);
            s.set("float", 2.5f64);
            s.set("yes", true);
            s.set("bytes", vec![0u8, 9, 10, 255]);
            assert!(s.is_dirty());
            s.flush().unwrap();
            assert!(!s.is_dirty());
        }
        let s = Storage::open(&dir, "mymod");
        assert_eq!(s.get_str("text"), Some(odd), "tabs and newlines survive");
        assert_eq!(s.get_int("int"), Some(-42));
        assert_eq!(s.get_float("float"), Some(2.5));
        assert_eq!(s.get_bool("yes"), Some(true));
        assert_eq!(s.get_bytes("bytes"), Some(&[0u8, 9, 10, 255][..]));
        assert_eq!(s.len(), 5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dropping_saves_and_scopes_keep_apart() {
        let dir = game_dir("scopes");
        {
            let mut p = Storage::open_player(&dir, "mymod", "uuid-1");
            p.set("coins", 7i64);
            let mut c = Storage::open_chunk(&dir, "mymod", "minecraft:overworld", -1, 2);
            c.set("ore", "copper");
        }
        assert_eq!(
            Storage::open_player(&dir, "mymod", "uuid-1").get_int("coins"),
            Some(7),
            "saved on drop"
        );
        assert_eq!(
            Storage::open_player(&dir, "mymod", "uuid-2").get_int("coins"),
            None
        );
        assert_eq!(
            Storage::open_chunk(&dir, "mymod", "minecraft:overworld", -1, 2).get_str("ore"),
            Some("copper")
        );
        assert!(
            Storage::open(&dir, "mymod").is_empty(),
            "the global store is its own"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn values_convert() {
        assert_eq!(Value::from(3u32).as_int(), Some(3));
        assert_eq!(Value::from("x").as_str(), Some("x"));
        assert_eq!(Value::from(true).as_int(), None);
    }
}
