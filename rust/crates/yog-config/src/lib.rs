//! Mod configuration — typed key/value config files under `<game_dir>/yog-config/`.
//!
//! File format: `key = value` pairs, one per line. Lines starting with `#` are
//! comments. Whitespace around keys and values is stripped.
//!
//! ```text
//! # mymod configuration
//! max_players = 20
//! welcome_message = Hello World!
//! pvp_enabled = true
//! damage_multiplier = 1.5
//! ```
//!
//! Obtain a game dir from `srv.game_dir()` inside an event handler.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// A key/value configuration file backed by `<game_dir>/yog-config/<mod_id>.cfg`.
pub struct Config {
    path: PathBuf,
    data: HashMap<String, String>,
}

impl Config {
    /// Load (or create) a config file for `mod_id` under `game_dir`.
    ///
    /// The file lives at `<game_dir>/yog-config/<mod_id>.cfg`.
    /// `:` and `/` in `mod_id` are replaced with `_` in the filename.
    pub fn load(game_dir: &str, mod_id: &str) -> Self {
        let safe = mod_id.replace([':', '/'], "_");
        let dir  = Path::new(game_dir).join("yog-config");
        let path = dir.join(format!("{safe}.cfg"));
        let data = Self::parse(&path).unwrap_or_default();
        Self { path, data }
    }

    fn parse(path: &Path) -> io::Result<HashMap<String, String>> {
        let file = std::fs::File::open(path)?;
        let mut map = HashMap::new();
        for line in io::BufReader::new(file).lines() {
            let line = line?;
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') { continue; }
            if let Some((k, v)) = t.split_once('=') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        Ok(map)
    }

    /// Get a value as a string slice.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    /// Get a string value, or `default` if the key is absent.
    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    /// Get a value parsed as `i64`.
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key)?.trim().parse().ok()
    }

    /// Get an integer value, or `default` if absent or unparseable.
    pub fn get_int_or(&self, key: &str, default: i64) -> i64 {
        self.get_int(key).unwrap_or(default)
    }

    /// Get a value parsed as `f64`.
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key)?.trim().parse().ok()
    }

    /// Get a float value, or `default` if absent or unparseable.
    pub fn get_float_or(&self, key: &str, default: f64) -> f64 {
        self.get_float(key).unwrap_or(default)
    }

    /// Get a value parsed as `bool`.
    ///
    /// Truthy: `true`, `yes`, `1`, `on`. Falsy: `false`, `no`, `0`, `off`.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)?.trim().to_lowercase().as_str() {
            "true" | "yes" | "1" | "on"  => Some(true),
            "false"| "no"  | "0" | "off" => Some(false),
            _ => None,
        }
    }

    /// Get a bool value, or `default` if absent or unrecognised.
    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }

    /// Set (or overwrite) a key. Call [`save`] to persist.
    pub fn set(&mut self, key: impl Into<String>, value: impl ToString) {
        self.data.insert(key.into(), value.to_string());
    }

    /// Remove a key. Returns the old value if present.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }

    /// Returns `true` if the key exists in the config.
    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Persist the current state to disk. Creates `yog-config/` if needed.
    ///
    /// Comments from the original file are not preserved; keys are written
    /// in alphabetical order for determinism.
    pub fn save(&self) -> io::Result<()> {
        if let Some(p) = self.path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut file = std::fs::File::create(&self.path)?;
        writeln!(file, "# Yog mod configuration — auto-generated")?;
        let mut keys: Vec<_> = self.data.keys().collect();
        keys.sort();
        for k in keys {
            writeln!(file, "{} = {}", k, self.data[k])?;
        }
        Ok(())
    }

    /// Save only if the config file does not yet exist (write defaults on first run).
    pub fn save_defaults(&self) -> io::Result<()> {
        if !self.path.exists() { self.save() } else { Ok(()) }
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize { self.data.len() }

    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.data.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
