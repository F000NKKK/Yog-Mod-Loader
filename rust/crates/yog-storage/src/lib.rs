//! Persistent key-value storage for Yog mods.
//!
//! Each [`Storage`] is backed by a plain text file under
//! `<game_dir>/yog-data/<namespace>.kvs` (one `key\tvalue` pair per line).
//! No external dependencies; values are always `String`.
//!
//! ```no_run
//! # use yog_storage::Storage;
//! // Typically called from an event handler that receives a &dyn Server:
//! // let mut store = Storage::open(srv.game_dir(), "mymod:economy");
//! // let coins: i64 = store.get("player_steve").and_then(|v| v.parse().ok()).unwrap_or(0);
//! // store.set("player_steve", (coins + 10).to_string());
//! // store.save().ok();
//! ```

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// A simple key→value store backed by a file on disk.
///
/// Keys and values must not contain `\t` or `\n`.
pub struct Storage {
    path: PathBuf,
    data: HashMap<String, String>,
}

impl Storage {
    /// Open (or create) a storage file for `namespace` under `game_dir`.
    ///
    /// The file lives at `<game_dir>/yog-data/<namespace>.kvs`.
    /// `namespace` may contain `:` (e.g. `"mymod:economy"`); the colon is
    /// replaced with `_` in the filename.
    pub fn open(game_dir: &str, namespace: &str) -> Self {
        let safe_name = namespace.replace(':', "_").replace('/', "_");
        let dir = Path::new(game_dir).join("yog-data");
        let path = dir.join(format!("{safe_name}.kvs"));
        let data = Self::load(&path).unwrap_or_default();
        Self { path, data }
    }

    fn load(path: &Path) -> io::Result<HashMap<String, String>> {
        let file = std::fs::File::open(path)?;
        let mut map = HashMap::new();
        for line in io::BufReader::new(file).lines() {
            let line = line?;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('\t') {
                map.insert(k.to_string(), v.to_string());
            }
        }
        Ok(map)
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.data.get(key).map(String::as_str).unwrap_or(default)
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.data.insert(key.into(), value.into());
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Persist the current state to disk. Creates the `yog-data/` directory if
    /// it does not yet exist.
    pub fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&self.path)?;
        for (k, v) in &self.data {
            writeln!(file, "{k}\t{v}")?;
        }
        Ok(())
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.data.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
