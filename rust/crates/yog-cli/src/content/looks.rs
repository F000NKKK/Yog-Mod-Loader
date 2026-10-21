//! Looks the game draws from what a mod declares elsewhere: an entity's
//! spawn egg (Yog's egg, tinted with the entity's colours, as a potion is
//! the game's bottle tinted with its effects') and an armor material's
//! worn look as the game's equipment asset (1.21.4+).

mod consts;
use consts as equipment;

use serde_json::json;
use std::path::Path;
use yog_ids::res;

use crate::consts::item_def;
use crate::content::{Compiled, pretty};

/// An opaque colour from `0xRRGGBB`, as the game's item tints take it (ARGB, signed).
const OPAQUE: u32 = 0xFF00_0000;

/// Whether the mod writes `path` itself (a hand-written asset, or another content file's output).
fn own(root: &Path, out: &Compiled, path: &str) -> bool {
    root.join(path).is_file() || out.outputs.iter().any(|(p, _)| p == path)
}

/// Entity `ns:p`'s spawn egg, coloured `[base, spots]`: a model inheriting
/// Yog's egg (two layers the hosts tint before 1.21.4) and an item
/// definition tinting them (1.21.4+). A mod that draws its egg itself keeps its own.
pub(super) fn spawn_egg(
    root: &Path,
    out: &mut Compiled,
    ns: &str,
    p: &str,
    [base, spots]: [u32; 2],
) {
    let item = format!("{p}{}", res::SPAWN_EGG_SUFFIX);
    let model = format!("{}{item}", res::ITEM_DIR);
    let model_path = yog_ids::asset_file(ns, &yog_ids::model_file(&model));
    if !own(root, out, &model_path) {
        out.outputs.push((
            model_path,
            pretty(&json!({ (yog_ids::model::PARENT): yog_ids::yog::SPAWN_EGG_MODEL })),
        ));
    }
    let def_path =
        yog_ids::asset_file(ns, &format!("{}{item}{}", res::ITEM_DEFINITIONS, res::JSON));
    if own(root, out, &def_path) {
        return;
    }
    let tint = |rgb: u32| json!({ (item_def::TYPE): item_def::CONSTANT, (item_def::VALUE): (OPAQUE | rgb) as i32 });
    let def = json!({ (item_def::MODEL): {
        (item_def::TYPE): item_def::PLAIN,
        (item_def::MODEL): yog_ids::join(ns, &model),
        (item_def::TINTS): [tint(base), tint(spots)],
    } });
    out.outputs.push((def_path, pretty(&def)));
}

/// Armor worn with texture `texture` (`mymod:mythril`) on 1.21.4+: its
/// equipment asset, and its two layers' pictures where that version looks
/// for them (`models/armor/mythril_layer_1` → `entity/equipment/humanoid/mythril`).
pub(super) fn equipment(
    root: &Path,
    out: &mut Compiled,
    texture: &str,
    warnings: &mut Vec<String>,
) {
    let (ns, path) = yog_ids::split(texture);
    let layer = json!([{ (equipment::TEXTURE): texture }]);
    let asset_path = yog_ids::asset_file(ns, &format!("{}{path}{}", res::EQUIPMENT, res::JSON));
    if !own(root, out, &asset_path) {
        let def = json!({ (equipment::LAYERS): {
            (equipment::HUMANOID): layer.clone(),
            (equipment::HUMANOID_LEGGINGS): layer,
        } });
        out.outputs.push((asset_path, pretty(&def)));
    }
    for (n, worn) in [(1, equipment::HUMANOID), (2, equipment::HUMANOID_LEGGINGS)] {
        let old = yog_ids::asset_file(
            ns,
            &yog_ids::texture_file(&format!(
                "{}{path}{}{n}",
                res::ARMOR_TEXTURES,
                res::ARMOR_LAYER
            )),
        );
        let new = yog_ids::asset_file(
            ns,
            &yog_ids::texture_file(&format!(
                "{}{worn}{}{path}",
                res::EQUIPMENT_TEXTURES,
                yog_ids::sep::PATH
            )),
        );
        if own(root, out, &new) {
            continue;
        }
        let picture = out
            .outputs
            .iter()
            .find(|(p, _)| *p == old)
            .map(|(_, b)| b.clone())
            .or_else(|| std::fs::read(root.join(&old)).ok());
        match picture {
            Some(bytes) => out.outputs.push((new, bytes)),
            None => warnings.push(format!(
                "armor texture {texture}: no {old}, worn armor shows nothing"
            )),
        }
    }
}
