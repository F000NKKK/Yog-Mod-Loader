//! `.ydef` tags and damage types — the game's data — and where the
//! definitions the code registers are kept.

use serde_json::Value;
use std::collections::BTreeMap;
use yog_data::AnyRefOrTag;
use yog_formats::def::{DamageTypeDef, TagDef};
use yog_ids::Format;
use yog_ids::ns::MINECRAFT;
use yog_ids::sep::{EXT, TAG};

use crate::content::Output;
use crate::content::datapack;
use crate::content::file::generated;
use crate::content::pretty;
use crate::content::refs;

/// A tag's entries: id (`#ns:tag` for a tag) → required (`false`: kept only if present).
pub(in crate::content) type TagEntries = BTreeMap<String, bool>;

/// A `.ydef` tag: where it's written and its entries (`optional` ones
/// kept only if present). What it names was checked with the file's refs.
pub(super) fn tag_entries(t: &TagDef, ns: &str) -> Result<(String, TagEntries), Vec<String>> {
    let id = yog_ids::qualify_in(ns, &t.id);
    let mut problems = Vec::new();
    let mut entries = BTreeMap::new();
    for (list, required) in [(&t.values, true), (&t.optional, false)] {
        for v in list {
            if v.kind() != t.of {
                problems.push(format!("`{v}` in a tag of {}", t.of));
                continue;
            }
            let (entry, required) = match v {
                AnyRefOrTag::One(r) => (yog_ids::qualify_in(MINECRAFT, &r.id), required),
                // A tag only some versions or loaders have: where it isn't, the rest still counts.
                AnyRefOrTag::Tag(r) => {
                    let tag = yog_ids::qualify_in(MINECRAFT, &r.id);
                    let everywhere = !matches!(
                        refs::tag_hosts(yog_ids::registry_of(&t.of), &tag),
                        Some(Some(_))
                    );
                    (format!("{TAG}{tag}"), required && everywhere)
                }
            };
            entries.insert(entry, required);
        }
    }
    if problems.is_empty() {
        Ok((datapack::tag_file(&t.of, &id), entries))
    } else {
        Err(problems)
    }
}

/// Damage type `id` (the mod's own, checked): its `damage_type/` file, and
/// the damage type tags it joins (path, its id).
pub(super) fn damage_type(d: &DamageTypeDef, id: &str) -> (Output, Vec<(String, String)>) {
    let (ns, path) = yog_ids::split(id);
    let mut v = serde_json::json!({
        "message_id": d.message_id.clone().unwrap_or_else(|| [ns, path].join(EXT)),
        "scaling": d.scaling,
        "exhaustion": d.exhaustion,
    });
    if let Some(e) = &d.effects {
        v["effects"] = Value::String(e.clone());
    }
    if let Some(m) = &d.death_message_type {
        v["death_message_type"] = Value::String(m.clone());
    }
    let kind = yog_ids::kind::DAMAGE_TYPE;
    let tags = d
        .tags
        .iter()
        .map(|t| (datapack::tag_file(kind, t), id.to_string()))
        .collect();
    ((datapack::data_file(kind, id), pretty(&v)), tags)
}

/// A definition's file under `.yog-build/generated/`.
pub(in crate::content) fn definition_path(kind: &str, id: &str) -> String {
    generated(kind, id, Format::Data)
}

/// A loot modifier's file under `.yog-build/generated/`.
pub(in crate::content) fn loot_path(id: &str) -> String {
    definition_path(yog_ids::word::LOOT, id)
}
