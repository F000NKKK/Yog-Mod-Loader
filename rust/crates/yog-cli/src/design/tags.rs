//! What an item tag holds (`#minecraft:planks`), for slots that show each
//! of its items in turn, as JEI does: the mod's own tags as `yog build`
//! makes them (its `.yog-build/generated/tags.ydata`, `content::TAGS_FILE` — content files'
//! `tags`, `.ydef` tags), and the game's — from the newest version that has
//! the tag (one removed in 1.21 still shows its 1.20 items) — nested tags
//! opened. Yog's common tags (`yog:ingots/iron`) are the game's and the
//! loaders' tags they stand for, and a loader's (`c:`, `forge:`) holds what
//! `yog_formats::tags` says.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use yog_ids::Format;

use serde_json::Value;

use super::assets::vanilla_versions;

/// The tags shown: item tags.
const REGISTRY: &str = yog_ids::kind::ITEM;

/// The mod's item tags, as the CLI listed them (`yog content check` / `yog build`).
fn mod_tags(root: Option<&Path>) -> BTreeMap<String, Vec<String>> {
    let Some(root) = root else {
        return BTreeMap::new();
    };
    let mut tags = crate::content::read_tags(root)
        .remove(REGISTRY)
        .unwrap_or_default();
    let Some(ns) = crate::design::project::mod_id(root) else {
        return tags;
    };
    for format in [Format::Block, Format::Item, Format::Def] {
        for file in crate::files::of(&root.join(yog_ids::project::CONTENT), format) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let Some(doc) = crate::design::data::read_object(&text) else {
                continue;
            };
            let value = Value::Object(doc);
            if format == Format::Def {
                if value
                    .get(yog_ids::content_field::KIND)
                    .and_then(Value::as_str)
                    != Some(yog_ids::kind::TAG)
                    || value
                        .get(yog_ids::content_field::OF)
                        .and_then(Value::as_str)
                        != Some(yog_ids::kind::ITEM)
                {
                    continue;
                }
                let Some(raw_id) = value.get(yog_ids::tag_file::ID).and_then(Value::as_str) else {
                    continue;
                };
                let tag = yog_ids::qualify_in(&ns, raw_id);
                let entries = tags.entry(tag).or_default();
                for values in [yog_ids::tag_file::VALUES, yog_ids::tag_file::OPTIONAL]
                    .into_iter()
                    .filter_map(|k| value.get(k).and_then(Value::as_array))
                {
                    for entry in values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|v| tag_entry(&ns, v))
                    {
                        if !entries.contains(&entry) {
                            entries.push(entry);
                        }
                    }
                }
            } else {
                let Some(raw_id) = value
                    .get(yog_ids::tag_file::ID)
                    .and_then(Value::as_str)
                    .or_else(|| file.file_stem().and_then(|s| s.to_str()))
                else {
                    continue;
                };
                let item_id = yog_ids::qualify_in(&ns, raw_id);
                for raw_tag in value
                    .get("tags")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    let entries = tags
                        .entry(yog_ids::qualify_in(&ns, raw_tag.trim_start_matches('#')))
                        .or_default();
                    if !entries.contains(&item_id) {
                        entries.push(item_id.clone());
                    }
                }
            }
        }
    }
    tags
}

fn tag_entry(ns: &str, value: &str) -> String {
    let tag = value
        .strip_prefix("@tag:item:")
        .or_else(|| value.strip_prefix("@tag:block:"));
    if let Some(tag) = tag {
        return format!("#{}", yog_ids::qualify_in(ns, tag));
    }
    if let Some(tag) = value.strip_prefix('#') {
        return format!("#{}", yog_ids::qualify_in(ns, tag));
    }
    value
        .strip_prefix("@item:")
        .or_else(|| value.strip_prefix("@block:"))
        .map_or_else(
            || yog_ids::qualify_in(ns, value),
            |id| yog_ids::qualify_in(ns, id),
        )
}

/// Tag `tag`'s entries: the mod's, then the game's from the newest version with it.
fn entries(roots: &[PathBuf], own: &BTreeMap<String, Vec<String>>, tag: &str) -> Vec<String> {
    let mut out = own.get(tag).cloned().unwrap_or_default();
    let (ns, path) = yog_ids::split(tag);
    if ns == yog_ids::ns::YOG
        && let Some(of) = yog_formats::tags::yog_tag(REGISTRY, path)
    {
        out.extend(of.iter().map(|t| format!("#{t}")));
        return out;
    }
    if let Some(has) = yog_formats::tags::loader_tag(REGISTRY, tag) {
        out.extend(has.iter().cloned());
        return out;
    }
    'versions: for v in vanilla_versions(roots) {
        for dir in yog_ids::registry_dirs(REGISTRY) {
            let rel = format!(
                "{ns}/{}/{dir}/{path}{}",
                yog_ids::pack::TAGS,
                yog_ids::res::JSON
            );
            let Some(file) = v
                .read_data(&rel)
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            else {
                continue;
            };
            for e in file
                .get(yog_ids::tag_file::VALUES)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                // `"id"` or `{ "id": …, "required": false }`.
                if let Some(id) = e
                    .as_str()
                    .or_else(|| e.get(yog_ids::tag_file::ID).and_then(Value::as_str))
                {
                    out.push(id.to_owned());
                }
            }
            break 'versions;
        }
    }
    out
}

/// How deep tags nest before the rest is left out (a loop of tags ends sooner: each is opened once).
const MAX_DEPTH: usize = 16;

/// A tag's items gathered: where tags are looked up, those opened, the items in order.
struct Gather<'a> {
    roots: &'a [PathBuf],
    own: &'a BTreeMap<String, Vec<String>>,
    seen: BTreeSet<String>,
    out: Vec<String>,
}

impl Gather<'_> {
    /// `tag`'s items, its nested tags opened.
    fn open(&mut self, tag: &str, depth: usize) {
        if depth > MAX_DEPTH || !self.seen.insert(tag.to_owned()) {
            return;
        }
        for id in entries(self.roots, self.own, tag) {
            match id.strip_prefix(yog_ids::sep::TAG_SHOWN) {
                Some(inner) => self.open(inner, depth + 1),
                None => {
                    let full = yog_ids::qualify(&id);
                    if !self.out.contains(&full) {
                        self.out.push(full);
                    }
                }
            }
        }
    }
}

/// The items of tag `tag` (`minecraft:planks`, no `#`), for the mod the
/// file at `path` belongs to; empty if the tag is unknown.
pub fn item_tag(path: &Path, tag: &str) -> Vec<String> {
    let root = super::mod_root(path);
    let roots: Vec<PathBuf> = root
        .iter()
        .flat_map(|r| {
            [
                r.clone(),
                r.parent().map(Path::to_path_buf).unwrap_or_default(),
            ]
        })
        .collect();
    let own = mod_tags(root.as_deref());
    let full = yog_ids::qualify(tag.trim_start_matches(yog_ids::sep::TAG_SHOWN));
    let mut gather = Gather {
        roots: &roots,
        own: &own,
        seen: BTreeSet::new(),
        out: Vec::new(),
    };
    gather.open(&full, 0);
    gather.out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hex Casting's own tag of planks.
    #[test]
    #[ignore = "needs ../HexMod-Yog"]
    fn a_mods_item_tag_has_its_items() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../HexMod-Yog")
            .join(yog_ids::file::MANIFEST);
        let items = item_tag(&manifest, "#hexcasting:edified_planks");
        assert!(
            items.iter().any(|i| i.contains("edified_planks")),
            "{items:?}"
        );
    }

    #[test]
    fn opens_mod_tags_from_source_without_a_prior_build() {
        let root = std::env::temp_dir().join(format!("idle-tags-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("content/blocks")).unwrap();
        std::fs::create_dir_all(root.join("content/tags")).unwrap();
        std::fs::create_dir_all(root.join("content/recipes")).unwrap();
        std::fs::write(root.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        std::fs::write(
            root.join("content/blocks/wood.yblock"),
            r#"{ tags: ["planks"] }"#,
        )
        .unwrap();
        std::fs::write(
            root.join("content/tags/all_wood.ydef"),
            r#"{ kind: "tag", of: "item", id: "all_wood", values: ["@tag:item:mymod:planks"] }"#,
        )
        .unwrap();

        let items = item_tag(&root.join("content/recipes/test.yrecipe"), "mymod:all_wood");
        assert_eq!(items, ["mymod:wood"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn yog_tags_open_into_the_tags_they_stand_for() {
        let items = item_tag(Path::new("/nowhere/r.yrecipe"), "#yog:ingots/iron");
        assert!(
            items.iter().any(|i| i == "minecraft:iron_ingot"),
            "{items:?}"
        );
    }
}
