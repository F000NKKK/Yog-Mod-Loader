//! A resource pack — a zip, or a folder with `assets/` — and the models in
//! it, the game's from its client jar behind them.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use yog_ids::pack::ASSETS;
use yog_ids::res;

/// A zip or a directory with `assets/`.
pub(super) enum Pack {
    Dir(PathBuf),
    Zip(BTreeMap<String, Vec<u8>>),
}

/// The files of a pack an import reads.
const KEPT: [&str; 3] = [res::JSON, res::PNG, res::MCMETA];
/// Where a pack's assets start: `assets/`.
const ASSETS_DIR: &str = yog_ids::concatcp!(ASSETS, yog_ids::sep::PATH);

impl Pack {
    pub(super) fn open(path: &Path) -> Result<Pack, String> {
        if path.is_dir() {
            if !path.join(ASSETS).is_dir() {
                return Err(format!("{} has no assets/ folder", path.display()));
            }
            return Ok(Pack::Dir(path.to_path_buf()));
        }
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut files = BTreeMap::new();
        // Packs sometimes wrap everything in one folder.
        let mut prefix: Option<String> = None;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            let name = entry
                .name()
                .map_err(|e| e.to_string())?
                .replace('\\', yog_ids::sep::PATH);
            if prefix.is_none()
                && let Some(at) = name.find(ASSETS_DIR)
            {
                prefix = Some(name[..at].to_string());
            }
            if !KEPT.iter().any(|ext| name.ends_with(ext)) {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            files.insert(name, bytes);
        }
        let prefix = prefix.unwrap_or_default();
        Ok(Pack::Zip(
            files
                .into_iter()
                .filter_map(|(k, v)| k.strip_prefix(&prefix).map(|k| (k.to_string(), v)))
                .collect(),
        ))
    }

    pub(super) fn read(&self, rel: &str) -> Option<Vec<u8>> {
        match self {
            Pack::Dir(d) => std::fs::read(d.join(rel)).ok(),
            Pack::Zip(files) => files.get(rel).cloned(),
        }
    }

    /// Every file's path in the pack, `/`-separated.
    fn paths(&self) -> Vec<String> {
        match self {
            Pack::Dir(d) => crate::files::under(&d.join(ASSETS), &|_| true)
                .iter()
                .filter_map(|p| p.strip_prefix(d).ok().map(crate::slash_path))
                .collect(),
            Pack::Zip(files) => files.keys().cloned().collect(),
        }
    }

    /// Paths under `assets/` matching `assets/<ns>/<dir>/<name>.json` → (ns, name).
    pub(super) fn list(&self, dir: &str) -> Vec<(String, String)> {
        let entry = |rel: &str| -> Option<(String, String)> {
            let (ns, rest) = rel
                .strip_prefix(ASSETS_DIR)?
                .split_once(yog_ids::sep::PATH)?;
            let name = rest
                .strip_prefix(dir)?
                .strip_prefix(yog_ids::sep::PATH)?
                .strip_suffix(res::JSON)?;
            Some((ns.to_string(), name.to_string()))
        };
        let mut out: Vec<(String, String)> = self.paths().iter().filter_map(|p| entry(p)).collect();
        out.sort();
        out
    }
}

/// Reads models: the pack first, then the client jar.
pub(super) struct Models<'a> {
    pub(super) pack: &'a Pack,
    jar: Option<zip::ZipArchive<std::fs::File>>,
    jar_tried: bool,
    root: PathBuf,
}

impl<'a> Models<'a> {
    pub(super) fn new(pack: &'a Pack, root: PathBuf) -> Self {
        Models {
            pack,
            jar: None,
            jar_tried: false,
            root,
        }
    }

    /// Model `id`, and whether the pack has it (else the game's jar does).
    pub(super) fn model(&mut self, id: &str) -> Option<(Map<String, Value>, bool)> {
        let (ns, path) = yog_ids::split(id);
        let rel = yog_ids::asset_file(ns, &yog_ids::model_file(path));
        if let Some(b) = self.pack.read(&rel) {
            return serde_json::from_slice(&b).ok().map(|m| (m, true));
        }
        if !self.jar_tried {
            self.jar_tried = true;
            self.jar =
                crate::design::drawing::assets::newest_client_jar(std::slice::from_ref(&self.root))
                    .and_then(|p| std::fs::File::open(p).ok())
                    .and_then(|f| zip::ZipArchive::new(f).ok());
        }
        let jar = self.jar.as_mut()?;
        let mut entry = jar.by_name(&rel).ok()?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).ok()?;
        serde_json::from_slice(&bytes).ok().map(|m| (m, false))
    }
}
