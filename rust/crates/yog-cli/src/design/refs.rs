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
use yog_sync::{Mutex, OnceLock};

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

/// Resources defined by files of their own under `resources/`, named by
/// their path there without the extension (`block/glass`).
const RESOURCE_KINDS: [(&str, Format); 2] = [(yog_ids::kind::TEXTURE, Format::Texture), (yog_ids::kind::MATERIAL, Format::Material)];

/// What `[external.<namespace>]` lists each kind under.
const EXTERNAL_KEYS: [(&str, &str); 4] = [
    (yog_ids::kind::ITEM, yog_ids::external::ITEMS),
    (yog_ids::kind::BLOCK, yog_ids::external::BLOCKS),
    (yog_ids::kind::ENTITY, yog_ids::external::ENTITIES),
    (yog_ids::kind::RECIPE, yog_ids::external::RECIPES),
];

/// Where the game's ids of a kind are in its client jar: translation keys
/// of its lang (`entity.minecraft.pig` → `pig`; `dotted`: the rest may hold
/// dots, `generic.max_health`), files of its data folders (by kind), or the
/// keys of one of its assets (`sounds.json`: sound events).
enum GameIds {
    /// Keys starting with the game's `<kind>.minecraft.` of each of these kinds.
    Lang { kinds: &'static [&'static str], dotted: bool },
    /// A prefix of its own (`attribute.name.`).
    LangPrefix(&'static str),
    Data,
    Keys(&'static str),
}

/// The game's sound events: the keys of its `sounds.json`.
const SOUNDS_ASSET: &str = yog_ids::res::SOUNDS_JSON;
/// Attributes' translation keys: `attribute.name.generic.max_health`.
const ATTRIBUTE_LANG_PREFIX: &str = "attribute.name.";
/// The game's potions' item names: `item.minecraft.potion.effect.<name>`, a Yog potion alias.
const POTION_LANG_PREFIX: &str = "potion.effect.";

const GAME_IDS: [(&str, GameIds); 12] = [
    (yog_ids::kind::ITEM, GameIds::Lang { kinds: &[yog_ids::kind::ITEM, yog_ids::kind::BLOCK], dotted: false }),
    (yog_ids::kind::BLOCK, GameIds::Lang { kinds: &[yog_ids::kind::BLOCK], dotted: false }),
    (yog_ids::kind::ENTITY, GameIds::Lang { kinds: &[yog_ids::kind::ENTITY], dotted: false }),
    (yog_ids::kind::EFFECT, GameIds::Lang { kinds: &[yog_ids::kind::EFFECT], dotted: false }),
    (yog_ids::kind::ATTRIBUTE, GameIds::LangPrefix(ATTRIBUTE_LANG_PREFIX)),
    (yog_ids::kind::BIOME, GameIds::Lang { kinds: &[yog_ids::kind::BIOME], dotted: false }),
    (ENCHANTMENT, GameIds::Lang { kinds: &[ENCHANTMENT], dotted: false }),
    (yog_ids::kind::LOOT_TABLE, GameIds::Data),
    (yog_ids::kind::ADVANCEMENT, GameIds::Data),
    (yog_ids::kind::DAMAGE_TYPE, GameIds::Data),
    (yog_ids::kind::RECIPE, GameIds::Data),
    (SOUND, GameIds::Keys(SOUNDS_ASSET)),
];

/// Kinds of the game's content a reference may name that no Yog file defines.
const ENCHANTMENT: &str = "enchantment";
const SOUND: &str = "sound";

/// The kinds whose game tags a reference may name (`tag:<kind>`).
const GAME_TAG_KINDS: [&str; 6] = [yog_ids::kind::ITEM, yog_ids::kind::BLOCK, yog_ids::kind::ENTITY, yog_ids::kind::FLUID, yog_ids::kind::BIOME, yog_ids::kind::DAMAGE_TYPE];

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
    let tag_of = format!("{}{}", yog_ids::typed::TAG, yog_ids::sep::NAMESPACE);
    if let Some(of) = kind.strip_prefix(tag_of.as_str()) {
        for d in defs(root, yog_ids::kind::TAG) {
            if d.value.get(yog_ids::content_field::OF).and_then(Value::as_str) == Some(of) {
                out.push(OwnRef { id: d.id, file: d.file });
            }
        }
        return out;
    }
    // Files named by their path: body files under content/, resources under resources/.
    let by_path = body_format(kind).map(|f| (yog_ids::project::CONTENT, f)).or_else(|| {
        RESOURCE_KINDS.iter().find(|(k, _)| *k == kind).map(|(_, f)| (yog_ids::project::RESOURCES, *f))
    });
    if let Some((dir, format)) = by_path {
        let dir = root.join(dir);
        for f in crate::design::files::of(&dir, format) {
            let rel = f.strip_prefix(&dir).unwrap_or(&f).with_extension("");
            out.push(OwnRef { id: yog_ids::join(&ns, &rel.to_string_lossy().replace('\\', yog_ids::sep::PATH)), file: f.to_string_lossy().into_owned() });
        }
        return out;
    }
    if let Some((_, exts)) = FILE_KINDS.iter().find(|(k, _)| *k == kind) {
        for format in *exts {
            for f in files(root, *format) {
                let given = read(&f).and_then(|d| d.get(yog_ids::content_field::ID).and_then(Value::as_str).map(str::to_string));
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
        if doc.get(yog_ids::content_field::KIND).and_then(Value::as_str) != Some(kind) {
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
    let tag_of = format!("{}{}", yog_ids::typed::TAG, yog_ids::sep::NAMESPACE);
    let tags = kind.strip_prefix(tag_of.as_str()).filter(|of| GAME_TAG_KINDS.contains(of)).map(yog_ids::registry_dirs);
    let how = GAME_IDS.iter().find(|(k, _)| *k == kind).map(|(_, h)| h);
    if tags.is_none() && how.is_none() {
        return Vec::new();
    }
    let Some(vanilla) = Resources::for_mod(root).vanilla else { return Vec::new() };
    type Cache = Mutex<HashMap<(PathBuf, String), Vec<String>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = (vanilla.jar.clone(), kind.to_string());
    if let Some(hit) = CACHE.get_or_init(Default::default).lock().get(&key).cloned() {
        return hit;
    }
    let keys = |asset: &str| {
        vanilla
            .read_all(asset)
            .into_iter()
            .find_map(|b| serde_json::from_slice::<serde_json::Map<String, Value>>(&b).ok())
            .unwrap_or_default()
    };
    let game = |path: &str| yog_ids::join(yog_ids::ns::MINECRAFT, path);
    // The `.json` files under `data/minecraft/<dir>/`, as ids.
    let data_ids = |dirs: Vec<String>| -> Vec<String> {
        dirs.iter()
            .flat_map(|d| vanilla.names_under(&format!("{}/{}/{d}/", yog_ids::pack::DATA, yog_ids::ns::MINECRAFT)))
            .filter_map(|n| n.strip_suffix(yog_ids::res::JSON).map(game))
            .collect()
    };
    let lang_ids = |prefixes: Vec<String>, dotted: bool| -> Vec<String> {
        keys(&format!("{}/{}", yog_ids::ns::MINECRAFT, yog_ids::lang_file(yog_ids::game::LOCALE)))
            .keys()
            .filter_map(|k| prefixes.iter().find_map(|p| k.strip_prefix(p.as_str())))
            .filter(|rest| rest.starts_with(POTION_LANG_PREFIX) || dotted || !rest.contains('.'))
            .map(|rest| match rest.strip_prefix(POTION_LANG_PREFIX).filter(|_| kind == yog_ids::kind::ITEM) {
                // `item.minecraft.potion.effect.swiftness` → `yog:potion/swiftness`.
                Some(name) => format!("{}{name}", yog_ids::yog::POTION_ALIAS),
                None => game(rest),
            })
            .collect()
    };
    let mut out: Vec<String> = match (tags, how) {
        (Some(dirs), _) => data_ids(dirs.iter().map(|d| format!("{}/{d}", yog_ids::pack::TAGS)).collect()),
        (None, None) => Vec::new(),
        (None, Some(GameIds::Lang { kinds, dotted })) => lang_ids(kinds.iter().map(|k| yog_ids::lang_prefix(k, yog_ids::ns::MINECRAFT)).collect(), *dotted),
        (None, Some(GameIds::LangPrefix(prefix))) => lang_ids(vec![prefix.to_string()], true),
        (None, Some(GameIds::Data)) => data_ids(yog_ids::registry_dirs(kind)),
        (None, Some(GameIds::Keys(asset))) => keys(&format!("{}/{asset}", yog_ids::ns::MINECRAFT)).keys().map(|k| game(k)).collect(),
    };
    out.sort();
    out.dedup();
    let mut c = CACHE.get_or_init(Default::default).lock();
    c.insert(key, out.clone());
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
