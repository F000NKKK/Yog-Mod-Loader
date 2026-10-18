//! What a content reference (`@<kind>:<id>`) can name, for the designers'
//! pickers — the same three sources `yog build` checks references against:
//! the mod's own content (`content/`), the other mods' ids it declares in
//! `yog.toml` (`[external.<namespace>]`) and the game's (read from the
//! client jar on this machine, when there is one). And the mod's own
//! `.ydef`s of a kind with what they say (an advancement's parent, an
//! effect's color), for the designers that draw several at once.

use yog_ids::Format;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use serde_json::Value;

use crate::design::assets::Resources;
use crate::design::project::{self, mod_root};

/// Kinds of content defined by files of their own, by extension; any other
/// kind is the `.ydef`s of that `kind`.
const FILE_KINDS: [(&str, &[Format]); 4] = [
    (yog_ids::kind::ITEM, &[Format::Item, Format::Block]),
    (yog_ids::kind::BLOCK, &[Format::Block]),
    (yog_ids::kind::ENTITY, &[Format::Entity]),
    (yog_ids::kind::RECIPE, &[Format::Recipe]),
];

/// Body files (`.ymodel`, `.yanim`): named by their path under `content/`
/// without the extension (`body/wings`), as an entity's `model` names them.
const BODY_FORMATS: [Format; 2] = [Format::Model, Format::Anim];

/// What `[external.<namespace>]` lists each kind under.
const EXTERNAL_KEYS: [(&str, &str); 4] = [
    (yog_ids::kind::ITEM, "items"),
    (yog_ids::kind::BLOCK, "blocks"),
    (yog_ids::kind::ENTITY, "entities"),
    (yog_ids::kind::RECIPE, "recipes"),
];

/// Where the game's ids of a kind are in its client jar: translation keys
/// of its `en_us` lang (`entity.minecraft.pig` → `pig`; `dotted`: the rest
/// may hold dots, `generic.max_health`), files of its data folders, or
/// the keys of one of its assets (`sounds.json`: sound events).
enum GameIds {
    Lang { prefixes: &'static [&'static str], dotted: bool },
    Data(&'static [&'static str]),
    Keys(&'static str),
}

const GAME_IDS: [(&str, GameIds); 12] = [
    ("item", GameIds::Lang { prefixes: &["item.minecraft.", "block.minecraft."], dotted: false }),
    ("block", GameIds::Lang { prefixes: &["block.minecraft."], dotted: false }),
    ("entity", GameIds::Lang { prefixes: &["entity.minecraft."], dotted: false }),
    ("effect", GameIds::Lang { prefixes: &["effect.minecraft."], dotted: false }),
    ("attribute", GameIds::Lang { prefixes: &["attribute.name."], dotted: true }),
    ("biome", GameIds::Lang { prefixes: &["biome.minecraft."], dotted: false }),
    ("enchantment", GameIds::Lang { prefixes: &["enchantment.minecraft."], dotted: false }),
    // 1.20 names data folders in the plural, 1.21 in the singular.
    ("loot_table", GameIds::Data(&["data/minecraft/loot_tables/", "data/minecraft/loot_table/"])),
    ("advancement", GameIds::Data(&["data/minecraft/advancements/", "data/minecraft/advancement/"])),
    ("damage_type", GameIds::Data(&["data/minecraft/damage_type/"])),
    ("recipe", GameIds::Data(&["data/minecraft/recipes/", "data/minecraft/recipe/"])),
    ("sound", GameIds::Keys("minecraft/sounds.json")),
];

/// Where the game's tags of a kind of content are (`tag:<kind>`): its data
/// folders under `data/minecraft/tags/` — 1.21's singular, 1.20's plural.
const GAME_TAG_DIRS: [(&str, &[&str]); 6] = [
    ("item", &["item", "items"]),
    ("block", &["block", "blocks"]),
    ("entity", &["entity_type", "entity_types"]),
    ("fluid", &["fluid", "fluids"]),
    ("biome", &["worldgen/biome"]),
    ("damage_type", &["damage_type"]),
];

/// One of the mod's own: its id and the file that defines it.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct OwnRef {
    pub id: String,
    pub file: String,
}

/// What a reference of one kind may name, ids with their namespace.
#[derive(Serialize, Debug, Default)]
pub struct RefChoices {
    pub own: Vec<OwnRef>,
    /// Declared in `yog.toml`, another mod's.
    pub external: Vec<String>,
    /// The game's (`minecraft:…`); empty without a client jar.
    pub game: Vec<String>,
}

/// One of the mod's `.ydef`s: its id, its file and what it says.
#[derive(Serialize, Debug)]
pub struct OwnDef {
    pub id: String,
    pub file: String,
    pub value: Value,
}

fn files(root: &Path, format: Format) -> Vec<PathBuf> {
    crate::design::files::of(&root.join("content"), format)
}

fn read(file: &Path) -> Option<serde_json::Map<String, Value>> {
    crate::design::data::read_object(&std::fs::read_to_string(file).ok()?)
}

/// The choices for `@<kind>:` in the file at `file` (`kind` may also be a
/// body format, `ymodel`/`yanim`, or `tag:<kind>` — the tags of a kind:
/// the mod's own `.ydef` tags of it, the game's).
pub fn refs(file: &Path, kind: &str) -> RefChoices {
    let mut out = RefChoices::default();
    if let Some(root) = mod_root(file) {
        out.own = own(&root, kind);
        out.external = external(&root, kind);
    }
    if body_format(kind).is_none() {
        out.game = game(mod_root(file).as_deref(), kind);
    }
    out
}

/// `kind` as a body format (`ymodel`, `yanim`).
fn body_format(kind: &str) -> Option<Format> {
    Format::of(kind).filter(|f| BODY_FORMATS.contains(f))
}

fn own(root: &Path, kind: &str) -> Vec<OwnRef> {
    let ns = crate::design::project::mod_id(root).unwrap_or_else(|| yog_ids::ns::MINECRAFT.into());
    let mut out = Vec::new();
    if let Some(of) = kind.strip_prefix("tag:") {
        for d in defs(root, yog_ids::kind::TAG) {
            if d.value.get("of").and_then(Value::as_str) == Some(of) {
                out.push(OwnRef { id: d.id, file: d.file });
            }
        }
        return out;
    }
    if let Some(format) = body_format(kind) {
        let content = root.join("content");
        for f in files(root, format) {
            let rel = f.strip_prefix(&content).unwrap_or(&f).with_extension("");
            out.push(OwnRef { id: format!("{ns}:{}", rel.to_string_lossy().replace('\\', "/")), file: f.to_string_lossy().into_owned() });
        }
        return out;
    }
    if let Some((_, exts)) = FILE_KINDS.iter().find(|(k, _)| *k == kind) {
        for format in *exts {
            for f in files(root, *format) {
                let given = read(&f).and_then(|d| d.get("id").and_then(Value::as_str).map(str::to_string));
                let id = given.unwrap_or_else(|| f.file_stem().unwrap_or_default().to_string_lossy().into_owned());
                out.push(OwnRef { id: yog_ids::qualify_in(&ns, &id), file: f.to_string_lossy().into_owned() });
            }
        }
    } else {
        out = defs(root, kind).into_iter().map(|d| OwnRef { id: d.id, file: d.file }).collect();
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// The mod's `.ydef`s of `kind`, in file order.
fn defs(root: &Path, kind: &str) -> Vec<OwnDef> {
    let ns = crate::design::project::mod_id(root).unwrap_or_else(|| yog_ids::ns::MINECRAFT.into());
    let mut out = Vec::new();
    for f in files(root, Format::Def) {
        let Some(doc) = read(&f) else { continue };
        if doc.get("kind").and_then(Value::as_str) != Some(kind) {
            continue;
        }
        let Some(id) = doc.get("id").and_then(Value::as_str) else { continue };
        out.push(OwnDef { id: yog_ids::qualify_in(&ns, id), file: f.to_string_lossy().into_owned(), value: Value::Object(doc) });
    }
    out
}

/// The mod's `.ydef`s of `kind` beside the file at `file`.
pub fn own_defs(file: &Path, kind: &str) -> Vec<OwnDef> {
    mod_root(file).map(|r| defs(&r, kind)).unwrap_or_default()
}

fn external(root: &Path, kind: &str) -> Vec<String> {
    let Some(key) = EXTERNAL_KEYS.iter().find(|(k, _)| *k == kind).map(|(_, k)| *k) else { return Vec::new() };
    let Ok(manifest) = crate::manifest::YogToml::read(&project::path(root)) else { return Vec::new() };
    let mut out = Vec::new();
    for external in &manifest.externals {
        for (_, ids) in external.ids.iter().filter(|(k, _)| k == key) {
            out.extend(ids.iter().map(|id| yog_ids::qualify_in(&external.namespace, id)));
        }
    }
    out
}

/// The game's ids of `kind`, from the client jar (cached per jar).
fn game(root: Option<&Path>, kind: &str) -> Vec<String> {
    let tags = kind.strip_prefix("tag:").and_then(|of| GAME_TAG_DIRS.iter().find(|(k, _)| *k == of)).map(|(_, dirs)| *dirs);
    let how = GAME_IDS.iter().find(|(k, _)| *k == kind).map(|(_, h)| h);
    if tags.is_none() && how.is_none() {
        return Vec::new();
    }
    let Some(vanilla) = Resources::for_mod(root).vanilla else { return Vec::new() };
    type Cache = Mutex<HashMap<(PathBuf, String), Vec<String>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = (vanilla.jar.clone(), kind.to_string());
    if let Some(hit) = CACHE.get_or_init(Default::default).lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let keys = |asset: &str| {
        vanilla
            .read_all(asset)
            .into_iter()
            .find_map(|b| serde_json::from_slice::<serde_json::Map<String, Value>>(&b).ok())
            .unwrap_or_default()
    };
    let mut out: Vec<String> = match (tags, how) {
        (Some(dirs), _) => dirs
            .iter()
            .flat_map(|d| vanilla.names_under(&format!("data/minecraft/tags/{d}/")))
            .filter_map(|n| n.strip_suffix(".json").map(|p| format!("minecraft:{p}")))
            .collect(),
        (None, None) => Vec::new(),
        (None, Some(GameIds::Lang { prefixes, dotted })) => {
            keys("minecraft/lang/en_us.json")
                .keys()
                .filter_map(|k| prefixes.iter().find_map(|p| k.strip_prefix(p)))
                .filter(|rest| rest.starts_with("potion.effect.") || *dotted || !rest.contains('.'))
                .map(|rest| {
                    if kind == "item" {
                        rest.strip_prefix("potion.effect.")
                            .map_or_else(|| format!("minecraft:{rest}"), |name| format!("yog:potion/{name}"))
                    } else {
                        format!("minecraft:{rest}")
                    }
                })
                .collect()
        }
        (None, Some(GameIds::Data(dirs))) => dirs
            .iter()
            .flat_map(|d| vanilla.names_under(d))
            .filter_map(|n| n.strip_suffix(".json").map(|p| format!("minecraft:{p}")))
            .collect(),
        (None, Some(GameIds::Keys(asset))) => keys(asset).keys().map(|k| format!("minecraft:{k}")).collect(),
    };
    out.sort();
    out.dedup();
    if let Ok(mut c) = CACHE.get_or_init(Default::default).lock() {
        c.insert(key, out.clone());
    }
    out
}

/// The game's kinds of block (`kind: "stairs"` on a `.yblock`): their
/// names, texture slots, the `block` fields they take and their defaults —
/// as `yog-formats` has them.
pub fn block_kinds() -> Value {
    use yog_data::Schema;
    Value::Array(yog_formats::content::BlockKind::built_ins().iter().map(|k| crate::design::data::to_json(&k.write())).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn a_mods_own_content_external_ids_and_defs() {
        let dir = std::env::temp_dir().join(format!("yog-idle-refs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, "yog.toml", "[mod]\nid = \"mymod\"\n\n[external.create]\nentities = [\"seat\"]\n");
        write(&dir, "content/items/wand.yitem", "{ name: \"Wand\" }");
        write(&dir, "content/items/potion.yitem", "{ item: { potion: { effects: [] } } }");
        write(&dir, "content/blocks/lamp.yblock", "// a lamp\n{ id: \"glow_lamp\" }");
        write(&dir, "content/mobs/wisp.yentity", "{ kind: \"mob\", width: 0.5, height: 0.5 }");
        write(&dir, "content/body/wisp.ymodel", "{ texture: \"\", bones: [] }");
        write(&dir, "content/fx/glow.ydef", "{ kind: \"effect\", id: \"glow\", category: \"beneficial\", color: 255 }");
        write(&dir, "content/fx/dim.ydef", "{ kind: \"attribute\", id: \"dim\", default: 1, min: 0, max: 2 }");
        write(&dir, "content/tags/plants.ydef", "{ kind: \"tag\", of: \"block\", id: \"plants\", values: [] }");
        let probe = dir.join("content/items/wand.yitem");
        let ids = |kind: &str| refs(&probe, kind).own.into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids("item"), ["mymod:glow_lamp", "mymod:potion", "mymod:wand"], "items include potion-only .yitems and a block's item form");
        assert_eq!(ids("block"), ["mymod:glow_lamp"]);
        assert_eq!(ids("entity"), ["mymod:wisp"]);
        assert_eq!(ids("ymodel"), ["mymod:body/wisp"]);
        assert_eq!(ids("effect"), ["mymod:glow"]);
        assert_eq!(refs(&probe, "entity").external, ["create:seat"]);
        assert_eq!(ids("tag:block"), ["mymod:plants"]);
        assert!(ids("tag:item").is_empty());
        let effects = own_defs(&probe, "effect");
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].value["color"], 255);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_games_block_kinds_are_listed() {
        let kinds = block_kinds();
        let names: Vec<&str> = kinds.as_array().unwrap().iter().filter_map(|k| k["name"].as_str()).collect();
        assert!(names.contains(&"stairs") && names.contains(&"pressure_plate"), "{names:?}");
    }
}
