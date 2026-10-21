//! A resource pack's blocks and items as `.yblock`/`.yitem` files: one by
//! id, or every one.

use serde_json::{Map, Value};

use super::fields as f;
use super::model::{convert_model, default_ns, state_rotations};
use super::pack::{Models, Pack};
use super::target::Target;
use crate::content::json::{J, to_pretty};
use crate::content::vocab::RENDER_LAYERS;
use yog_ids::kind::{BLOCK, ITEM};
use yog_ids::sep::PATH;

/// Where pack models live: blockstates, block and item models.
const BLOCKSTATES: &str = "blockstates";
const BLOCK_MODELS: &str = yog_ids::concatcp!("models", PATH, BLOCK);
const ITEM_MODELS: &str = yog_ids::concatcp!("models", PATH, ITEM);
/// A model id's folder: `block/lamp`, `item/lamp`.
const BLOCK_DIR: &str = yog_ids::res::BLOCK_DIR;
const ITEM_DIR: &str = yog_ids::res::ITEM_DIR;
/// The render layer the game's blocks have when they say none.
const SOLID: &str = "solid";

/// A content file's place: `content/blocks/<leaf>.yblock`, `content/items/<leaf>.yitem`.
pub(super) fn content_file(item: bool, name: &str) -> String {
    let leaf = name.rsplit(PATH).next().unwrap_or(name);
    let (dir, format) = if item {
        (yog_ids::word::ITEMS, yog_ids::Format::Item)
    } else {
        (yog_ids::word::BLOCKS, yog_ids::Format::Block)
    };
    [crate::content::CONTENT_DIR, dir, &format.file(leaf)].join(PATH)
}

/// The pack's English name for a key, if it has a plain `en_us.json`.
fn lang_name(pack: &Pack, ns: &str, key: &str) -> Option<String> {
    let bytes = pack.read(&yog_ids::asset_file(
        ns,
        &yog_ids::lang_file(yog_ids::game::LOCALE),
    ))?;
    let m: Map<String, Value> = serde_json::from_slice(&bytes).ok()?;
    m.get(key).and_then(Value::as_str).map(String::from)
}

/// A content file's start: its id (when the mod's namespace isn't it), its name.
fn header(t: &Target, ns: &str, name: &str, pack: &Pack, kind: &str) -> J {
    let mut j = J::obj::<&str>([]);
    if let Some(id) = t.id_for(ns, name) {
        j.set(f::ID, J::string(id));
    }
    if let Some(n) = lang_name(pack, ns, &yog_ids::lang_key(kind, &yog_ids::join(ns, name))) {
        j.set(f::NAME, J::string(n));
    }
    j
}

/// Block `ns:name`: its blockstate's model and rotations, its render
/// layer, its own item model when it isn't just the block's.
fn import_block(t: &mut Target, models: &mut Models, ns: &str, name: &str) -> Result<(), String> {
    let pack = models.pack;
    let bs: Option<Value> = pack
        .read(&yog_ids::asset_file(ns, &yog_ids::blockstate_file(name)))
        .and_then(|b| serde_json::from_slice(&b).ok());
    let (model_id, rotations) = match &bs {
        Some(bs) => state_rotations(t, &yog_ids::join(ns, name), bs),
        None => (None, None),
    };
    let model_id = model_id.unwrap_or_else(|| yog_ids::join(ns, &format!("{BLOCK_DIR}{name}")));
    let (model, render) = convert_model(t, models, &model_id, false)?;
    let mut j = header(t, ns, name, pack, BLOCK);
    if let Some(r) = render.filter(|r| RENDER_LAYERS.contains(&r.as_str()) && r != SOLID) {
        j.set(f::BLOCK, J::obj([(f::RENDER_LAYER, J::string(r))]));
    }
    j.set(f::MODEL, model);
    let item_id = yog_ids::join(ns, &format!("{ITEM_DIR}{name}"));
    if let Some((im, _)) = models.model(&item_id) {
        let plain = im.len() == 1
            && im
                .get(yog_ids::model::PARENT)
                .and_then(Value::as_str)
                .map(yog_ids::qualify)
                == Some(yog_ids::qualify(&model_id));
        if !plain {
            let (item_model, _) = convert_model(t, models, &item_id, true)?;
            j.set(f::ITEM_MODEL, item_model);
        }
    }
    if let Some(r) = rotations {
        j.set(f::STATES, J::obj([(f::ROTATIONS, r)]));
    }
    t.write_file(&content_file(false, name), to_pretty(&j).as_bytes())
}

/// Item `ns:name`: its model.
fn import_item(t: &mut Target, models: &mut Models, ns: &str, name: &str) -> Result<(), String> {
    let (model, _) = convert_model(
        t,
        models,
        &yog_ids::join(ns, &format!("{ITEM_DIR}{name}")),
        true,
    )?;
    let mut j = header(t, ns, name, models.pack, ITEM);
    j.set(f::MODEL, model);
    t.write_file(&content_file(true, name), to_pretty(&j).as_bytes())
}

/// What a pack has: its blockstates', block models' and item models' (ns, name).
struct Listed {
    blockstates: Vec<(String, String)>,
    block_models: Vec<(String, String)>,
    item_models: Vec<(String, String)>,
}

impl Listed {
    fn has(list: &[(String, String)], ns: &str, path: &str) -> bool {
        list.iter().any(|(n, p)| n == ns && p == path)
    }

    /// Every id it has, a line each.
    fn ids(&self) -> String {
        let blocks = self
            .blockstates
            .iter()
            .chain(&self.block_models)
            .map(|(ns, p)| format!("\n  {ns}:{BLOCK_DIR}{p}"));
        let items = self
            .item_models
            .iter()
            .map(|(ns, p)| format!("\n  {ns}:{ITEM_DIR}{p}"));
        let list: String = blocks.chain(items).collect();
        if list.is_empty() {
            "\n  (no models)".to_string()
        } else {
            list
        }
    }
}

/// Imports the pack's `model` (`ns:block/name`, `ns:item/name`, or a
/// name the pack has), or with `all` every block and item.
pub(super) fn import_pack(
    t: &mut Target,
    pack: &Pack,
    model: Option<&str>,
    all: bool,
) -> Result<(), String> {
    let mut models = Models::new(pack, t.root.clone());
    let listed = Listed {
        blockstates: pack.list(BLOCKSTATES),
        block_models: pack.list(BLOCK_MODELS),
        item_models: pack.list(ITEM_MODELS),
    };
    if let Some(m) = model {
        return import_one(t, &mut models, &listed, m);
    }
    if !all {
        return Err(format!(
            "pick one with --model <id>, or --all for every block (blockstate) and item. The pack has:{}",
            listed.ids()
        ));
    }
    let mut failed = Vec::new();
    for (ns, name) in &listed.blockstates {
        failed.extend(import_block(t, &mut models, ns, name).err());
    }
    // A block's item model came with the block.
    for (ns, name) in listed
        .item_models
        .iter()
        .filter(|(ns, name)| !Listed::has(&listed.blockstates, ns, name))
    {
        failed.extend(import_item(t, &mut models, ns, name).err());
    }
    t.warnings.extend(failed);
    Ok(())
}

/// One block or item by id: its folder says which, else what the pack has.
fn import_one(t: &mut Target, models: &mut Models, listed: &Listed, m: &str) -> Result<(), String> {
    let qualified = m.contains(yog_ids::sep::NAMESPACE);
    let (ns, path) = if qualified {
        (yog_ids::split(m).0.to_string(), yog_ids::split(m).1)
    } else {
        (default_ns(t), m)
    };
    if let Some(name) = path.strip_prefix(BLOCK_DIR) {
        return import_block(t, models, &ns, name);
    }
    if let Some(name) = path.strip_prefix(ITEM_DIR) {
        return import_item(t, models, &ns, name);
    }
    if Listed::has(&listed.blockstates, &ns, path) || Listed::has(&listed.block_models, &ns, path) {
        return import_block(t, models, &ns, path);
    }
    if Listed::has(&listed.item_models, &ns, path) {
        return import_item(t, models, &ns, path);
    }
    Err(format!("no block or item {ns}:{path} in the pack"))
}
