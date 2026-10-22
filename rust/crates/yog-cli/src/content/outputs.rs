//! Merging what every file produced: tags, lang files, per-version overlays.

use crate::content::ctx::{caveat, problem};
use crate::content::{Compiled, Output, datapack, pretty, refs};
use crate::diagnostics::Diagnostic;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use yog_ids::tag_file as tf;

/// The mod's data names vanilla content as its datapack version does; for
/// each other version that calls some of it otherwise, an overlay of the
/// file with those names (`when/any/<version>/…`). What a version hasn't
/// got under any name is said in a warning.
pub(super) fn version_overlays(out: &mut Compiled, datapack: &str) {
    fn map(v: &mut Value, version: &str, missing: &mut BTreeSet<String>) -> bool {
        match v {
            Value::String(s) if yog_ids::game_path(s).is_some() => {
                let id = s.clone();
                let eq = refs::equivalent(refs::RefKind::Item, &id, version)
                    .or_else(|| refs::equivalent(refs::RefKind::Block, &id, version));
                match eq {
                    Some(e) if e != id => {
                        *s = e;
                        true
                    }
                    Some(_) => false,
                    // Not an id Yog knows (a loot function, a tag), or one this version hasn't got.
                    None => {
                        if refs::is_vanilla(&id) {
                            missing.insert(id);
                        }
                        false
                    }
                }
            }
            Value::Array(a) => a
                .iter_mut()
                .fold(false, |c, x| map(x, version, missing) | c),
            Value::Object(o) => o
                .values_mut()
                .fold(false, |c, x| map(x, version, missing) | c),
            _ => false,
        }
    }
    let mut overlays = Vec::new();
    for (path, bytes) in &out.outputs {
        if !path.starts_with(&format!("{}{}", yog_ids::pack::DATA, yog_ids::sep::PATH))
            || !path.ends_with(yog_ids::res::JSON)
        {
            continue;
        }
        let Ok(json) = serde_json::from_slice::<Value>(bytes) else {
            continue;
        };
        for version in refs::versions().iter().filter(|v| *v != datapack) {
            let mut copy = json.clone();
            let mut missing = BTreeSet::new();
            if map(&mut copy, version, &mut missing) {
                overlays.push((datapack::for_version(version, path), pretty(&copy)));
            }
            for id in missing {
                out.warnings.push(caveat(format!(
                    "{path} names {id}, which Minecraft {version} hasn't got"
                )));
            }
        }
    }
    out.outputs.extend(overlays);
}

/// Writes [`TAGS_FILE`]: `{ item: { "ns:path": ["ns:id", "#ns:tag", …] }, block: … }`.
pub(super) fn write_tags(root: &Path, compiled: &Compiled) -> Result<(), String> {
    use yog_ids::pack::{DATA, TAGS};
    use yog_ids::sep::PATH;
    let data_dir = format!("{DATA}{PATH}");
    let tags_dir = format!("{PATH}{TAGS}{PATH}");
    let mut by_kind = TagLists::new();
    for (path, entries) in &compiled.tags {
        // `[when/<loader>/<version>/]data/<ns>/tags/<kind>/<path>.json`
        let data = path
            .find(&data_dir)
            .map_or(path.as_str(), |i| &path[i + data_dir.len()..]);
        let Some((ns, rest)) = data.split_once(&tags_dir) else {
            continue;
        };
        let Some((kind, tag)) = rest.split_once(PATH) else {
            continue;
        };
        let tag = tag.trim_end_matches(yog_ids::res::JSON);
        by_kind
            .entry(kind.to_string())
            .or_default()
            .entry(yog_ids::join(ns, tag))
            .or_default()
            .extend(entries.keys().cloned());
    }
    crate::write_if_changed(&root.join(TAGS_FILE), yog_data::to_bytes(&by_kind))
}

/// A JSON object output at `path`: the one already in `outputs`, else the
/// mod's own file there, else empty.
pub(super) fn json_output(
    outputs: &OutputSet,
    root: &Path,
    path: &str,
) -> Result<Map<String, Value>, String> {
    let read;
    let bytes = match outputs.get(path) {
        Some(b) => b,
        None => match std::fs::read(root.join(path)) {
            Ok(b) => {
                read = b;
                &read
            }
            Err(_) => return Ok(Map::new()),
        },
    };
    serde_json::from_slice::<Map<String, Value>>(bytes).map_err(|e| format!("{path}: {e}"))
}

/// Adds the content's outputs to `outputs` (the `.yres` ones, say): lang
/// entries and tags merged into what's there (or into the mod's own files),
/// the rest appended.
pub fn merge_outputs(
    root: &Path,
    outputs: &mut Vec<Output>,
    compiled: Compiled,
) -> Result<(), String> {
    let mut m = Merging {
        set: OutputSet::from(std::mem::take(outputs)),
        root,
        errors: Vec::new(),
    };
    if !compiled.lang_aliases.is_empty() {
        m.lang_aliases(&compiled.lang_aliases);
    }
    for ((ns, locale), entries) in &compiled.lang {
        m.lang(ns, locale, entries);
    }
    for (path, ids) in &compiled.tags {
        m.tag(path, ids);
    }
    for (path, bytes) in compiled.outputs {
        m.set.put(path, bytes);
    }
    *outputs = m.set.into_vec();
    if m.errors.is_empty() {
        Ok(())
    } else {
        crate::diagnostics::report_all(&m.errors);
        Err(format!(
            "merging blocks/items into the mod's files:{}{}",
            crate::diagnostics::LIST_SEP,
            crate::diagnostics::listed(&m.errors)
        ))
    }
}

/// The outputs being merged into, the mod's root (its own files), the problems.
struct Merging<'a> {
    set: OutputSet,
    root: &'a Path,
    errors: Vec<Diagnostic>,
}

impl Merging<'_> {
    /// The JSON object at `path` (an output, the mod's own file, or empty); a
    /// problem noted when it won't read.
    fn object(&mut self, path: &str) -> Option<Map<String, Value>> {
        json_output(&self.set, self.root, path)
            .map_err(|e| self.errors.push(problem(e).in_file(path)))
            .ok()
    }

    /// `.yres` texts under the keys the game looks up: in every lang file
    /// (the outputs' and the mod's own) that has the text.
    fn lang_aliases(&mut self, aliases: &[(String, String)]) {
        let mut files: BTreeSet<String> = self
            .set
            .paths()
            .filter(|p| is_lang_path(p))
            .cloned()
            .collect();
        files.extend(own_lang_files(self.root));
        for path in files {
            let Some(mut map) = self.object(&path) else {
                continue;
            };
            let mut changed = false;
            for (from, to) in aliases {
                if let Some(text) = map.get(from).cloned()
                    && !map.contains_key(to)
                {
                    map.insert(to.clone(), text);
                    changed = true;
                }
            }
            if changed {
                self.set.put(path, pretty(&Value::Object(map)));
            }
        }
    }

    /// A namespace's entries in a locale's lang file, under what's there.
    fn lang(
        &mut self,
        ns: &str,
        locale: &str,
        entries: &std::collections::BTreeMap<String, String>,
    ) {
        let path = yog_ids::asset_file(ns, &yog_ids::lang_file(locale));
        let Some(mut map) = self.object(&path) else {
            return;
        };
        for (k, v) in entries {
            map.entry(k.clone())
                .or_insert_with(|| Value::String(v.clone()));
        }
        self.set.put(path, pretty(&Value::Object(map)));
    }

    /// A tag file's entries (`required` false: kept only if present) added
    /// to what's there. The mod's own file may use the pre-1.21 spelling; the
    /// loader maps both to one file, so it's merged into whichever it has.
    fn tag(&mut self, path: &str, ids: &std::collections::BTreeMap<String, bool>) {
        let block_tags = |dir: &str| {
            format!(
                "{}{}{dir}{}",
                yog_ids::sep::PATH,
                yog_ids::pack::TAGS,
                yog_ids::sep::PATH
            )
        };
        let plural = path.replacen(
            &block_tags(yog_ids::kind::BLOCK),
            &block_tags(yog_ids::word::BLOCKS),
            1,
        );
        let path = if !self.set.contains(path) && self.root.join(&plural).is_file() {
            plural
        } else {
            path.to_string()
        };
        let Some(mut map) = self.object(&path) else {
            return;
        };
        if let Value::Array(list) = map
            .entry(tf::VALUES)
            .or_insert_with(|| Value::Array(Vec::new()))
        {
            for (id, &required) in ids {
                let listed = |v: &Value| {
                    v.as_str().or_else(|| v.get(tf::ID).and_then(Value::as_str))
                        == Some(id.as_str())
                };
                if !list.iter().any(listed) {
                    list.push(if required {
                        Value::String(id.clone())
                    } else {
                        serde_json::json!({ tf::ID: id, tf::REQUIRED: false })
                    });
                }
            }
        }
        map.entry(tf::REPLACE).or_insert(Value::Bool(false));
        self.set.put(path, pretty(&Value::Object(map)));
    }
}

/// The mod's own lang files (`assets/<ns>/lang/<locale>.json`), as paths from `root`.
fn own_lang_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(root.join(yog_ids::pack::ASSETS)) else {
        return out;
    };
    for ns in dirs.flatten() {
        let Ok(files) = std::fs::read_dir(ns.path().join(yog_ids::res::LANG)) else {
            continue;
        };
        for f in files.flatten() {
            if let Ok(rel) = f.path().strip_prefix(root) {
                let rel = rel.to_string_lossy().replace('\\', yog_ids::sep::PATH);
                if is_lang_path(&rel) {
                    out.push(rel);
                }
            }
        }
    }
    out
}

/// Whether `p` is a lang file: `assets/<ns>/lang/<locale>.json`.
pub(super) fn is_lang_path(p: &str) -> bool {
    let lang = yog_ids::res::LANG.trim_end_matches(yog_ids::sep::PATH);
    matches!(p.split(yog_ids::sep::PATH).collect::<Vec<_>>().as_slice(),
        [assets, _, dir, file] if *assets == yog_ids::pack::ASSETS && *dir == lang && file.ends_with(yog_ids::res::JSON))
}

/// Where the mod's tags are listed for tools (`ns:path` → its entries, by
/// kind), as binary Yog Data — the IDE shows a tag's items from here, as the
/// build makes them.
pub const TAGS_FILE: &str =
    yog_ids::concatcp!(".yog-build/generated/tags.", yog_ids::Format::Data.ext());

/// The mod's tags as [`TAGS_FILE`] lists them: kind → tag → its entries.
pub type TagLists = BTreeMap<String, BTreeMap<String, Vec<String>>>;

/// [`TAGS_FILE`] of the mod at `root`, as the build last wrote it; empty without one.
pub fn read_tags(root: &Path) -> TagLists {
    std::fs::read(root.join(TAGS_FILE))
        .ok()
        .and_then(|b| yog_data::read_bytes::<TagLists>(&b).ok())
        .unwrap_or_default()
}

/// Outputs in order, keyed by path: [`OutputSet::put`] replaces a path's
/// output (moving it to the end) without rescanning the list.
#[derive(Default)]
pub struct OutputSet {
    items: Vec<Option<Output>>,
    at: HashMap<String, usize>,
}

impl OutputSet {
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        let i = *self.at.get(path)?;
        self.items[i].as_ref().map(|(_, b)| b.as_slice())
    }

    pub fn contains(&self, path: &str) -> bool {
        self.at.contains_key(path)
    }

    pub fn paths(&self) -> impl Iterator<Item = &String> {
        self.at.keys()
    }

    /// Sets `path`'s output, dropping the one it had.
    pub fn put(&mut self, path: String, bytes: Vec<u8>) {
        if let Some(i) = self.at.insert(path.clone(), self.items.len()) {
            self.items[i] = None;
        }
        self.items.push(Some((path, bytes)));
    }

    pub fn into_vec(self) -> Vec<Output> {
        self.items.into_iter().flatten().collect()
    }
}

impl From<Vec<Output>> for OutputSet {
    fn from(outputs: Vec<Output>) -> Self {
        let mut set = OutputSet::default();
        for (path, bytes) in outputs {
            set.put(path, bytes);
        }
        set
    }
}
