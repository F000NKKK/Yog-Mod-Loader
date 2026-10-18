//! Merging what every file produced: tags, lang files, per-version overlays.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use crate::content::Compiled;
use crate::content::Output;
use crate::content::OutputSet;
use crate::content::TAGS_FILE;
use crate::content::datapack;
use crate::content::pretty;
use crate::content::refs;


/// The mod's data names vanilla content as its datapack version does; for
/// each other version that calls some of it otherwise, an overlay of the
/// file with those names (`when/any/<version>/…`). What a version hasn't
/// got under any name is said in a warning.
pub(super) fn version_overlays(out: &mut Compiled, datapack: &str) {
    fn map(v: &mut Value, version: &str, missing: &mut BTreeSet<String>) -> bool {
        match v {
            Value::String(s) if s.starts_with("minecraft:") => {
                let id = s.trim_start_matches('#').to_string();
                let eq = refs::equivalent(refs::RefKind::Item, &id, version).or_else(|| refs::equivalent(refs::RefKind::Block, &id, version));
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
            Value::Array(a) => a.iter_mut().fold(false, |c, x| map(x, version, missing) | c),
            Value::Object(o) => o.values_mut().fold(false, |c, x| map(x, version, missing) | c),
            _ => false,
        }
    }
    let mut overlays = Vec::new();
    for (path, bytes) in &out.outputs {
        if !path.starts_with("data/") || !path.ends_with(".json") {
            continue;
        }
        let Ok(json) = serde_json::from_slice::<Value>(bytes) else { continue };
        for version in refs::versions().iter().filter(|v| *v != datapack) {
            let mut copy = json.clone();
            let mut missing = BTreeSet::new();
            if map(&mut copy, version, &mut missing) {
                overlays.push((datapack::for_version(version, path), pretty(&copy)));
            }
            for id in missing {
                out.warnings.push(format!("{path} names {id}, which Minecraft {version} hasn't got"));
            }
        }
    }
    out.outputs.extend(overlays);
}


/// Writes [`TAGS_FILE`]: `{ "item": { "ns:path": ["ns:id", "#ns:tag", …] }, "block": … }`.
pub(super) fn write_tags(root: &Path, compiled: &Compiled) -> Result<(), String> {
    let mut by_kind: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for (path, entries) in &compiled.tags {
        // `[when/<loader>/<version>/]data/<ns>/tags/<kind>/<path>.json`
        let data = path.find("data/").map_or(path.as_str(), |i| &path[i + 5..]);
        let Some((ns, rest)) = data.split_once("/tags/") else { continue };
        let Some((kind, tag)) = rest.split_once('/') else { continue };
        let tag = tag.trim_end_matches(".json");
        by_kind.entry(kind.to_string()).or_default().entry(format!("{ns}:{tag}")).or_default().extend(entries.keys().cloned());
    }
    let text = serde_json::to_string_pretty(&by_kind).map_err(|e| e.to_string())?;
    crate::write_if_changed(&root.join(TAGS_FILE), &text)
}

/// A JSON object output at `path`: the one already in `outputs`, else the
/// mod's own file there, else empty.
pub(super) fn json_output(outputs: &OutputSet, root: &Path, path: &str) -> Result<Map<String, Value>, String> {
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
    let mut set = OutputSet::from(std::mem::take(outputs));
    let mut errors = Vec::new();
    // `.yres` texts under the keys the game looks up: in every lang file
    // that has the text.
    if !compiled.lang_aliases.is_empty() {
        let mut lang_files: BTreeSet<String> =
            set.paths().filter(|p| is_lang_path(p)).cloned().collect();
        if let Ok(dirs) = std::fs::read_dir(root.join("assets")) {
            for ns in dirs.flatten() {
                if let Ok(files) = std::fs::read_dir(ns.path().join("lang")) {
                    for f in files.flatten() {
                        if let Ok(rel) = f.path().strip_prefix(root) {
                            let rel = rel.to_string_lossy().replace('\\', "/");
                            if is_lang_path(&rel) {
                                lang_files.insert(rel);
                            }
                        }
                    }
                }
            }
        }
        for path in lang_files {
            let mut map = match json_output(&set, root, &path) {
                Ok(m) => m,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            let mut changed = false;
            for (from, to) in &compiled.lang_aliases {
                if let Some(text) = map.get(from).cloned()
                    && !map.contains_key(to)
                {
                    map.insert(to.clone(), text);
                    changed = true;
                }
            }
            if changed {
                set.put(path, pretty(&Value::Object(map)));
            }
        }
    }
    for ((ns, locale), entries) in &compiled.lang {
        let path = format!("assets/{ns}/lang/{locale}.json");
        let mut map = match json_output(&set, root, &path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        for (k, v) in entries {
            map.entry(k.clone())
                .or_insert_with(|| Value::String(v.clone()));
        }
        set.put(path, pretty(&Value::Object(map)));
    }
    for (path, ids) in &compiled.tags {
        // The mod's own tag file may use the pre-1.21 spelling; the loader
        // maps both to one file, so merge into whichever it has.
        let plural = path.replacen("/tags/block/", "/tags/blocks/", 1);
        let path = if !set.contains(path) && root.join(&plural).is_file() {
            plural
        } else {
            path.clone()
        };
        let mut map = match json_output(&set, root, &path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let values = map
            .entry("values")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(list) = values {
            for (id, &required) in ids {
                let listed = |v: &Value| v.as_str().or_else(|| v.get("id").and_then(Value::as_str)) == Some(id.as_str());
                if !list.iter().any(listed) {
                    list.push(if required { Value::String(id.clone()) } else { serde_json::json!({ "id": id, "required": false }) });
                }
            }
        }
        map.entry("replace").or_insert(Value::Bool(false));
        set.put(path, pretty(&Value::Object(map)));
    }
    for (path, bytes) in compiled.outputs {
        set.put(path, bytes);
    }
    *outputs = set.into_vec();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "merging blocks/items into the mod's files:\n  {}",
            errors.join("\n  ")
        ))
    }
}

pub(super) fn is_lang_path(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    parts.len() == 4 && parts[0] == "assets" && parts[2] == "lang" && parts[3].ends_with(".json")
}
