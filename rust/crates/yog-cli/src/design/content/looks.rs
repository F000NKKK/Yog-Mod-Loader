//! How the mod's blocks and items look in the editors: a block's faces
//! and model, an item's appearance and icon, a potion's colour.

use serde_json::Value;
use std::path::Path;
use yog_ids::Format;

use super::{ICON_SLOTS, content_file, model_textures, texture_bytes};
use crate::design::project::mod_root;

/// The textures of the mod's block `id` (its `.yblock` model's), per face:
/// up, down, north, south, west, east — as png bytes. `None`: not a block
/// of the mod.
pub fn content_block_faces(root: &Path, id: &str) -> Option<[Option<Vec<u8>>; 6]> {
    let (file, doc) = content_file(root, id, &[Format::Block])?;
    let textures = model_textures(root, &doc, yog_ids::content_field::MODEL);
    let pick = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| textures.get(*k).and_then(Value::as_str))
            .or_else(|| textures.values().find_map(Value::as_str))
            .and_then(|t| texture_bytes(&file, t).ok())
    };
    Some(crate::design::blocks3d::FACE_TEXTURES.map(pick))
}

/// A block's model: its elements, and its textures by variable (png bytes).
pub type BlockModel = (Vec<Value>, std::collections::HashMap<String, Vec<u8>>);

/// The model of the mod's block `id` as its `.yblock` gives it: its
/// `elements` and its textures by variable (png bytes). `None`: not a
/// block of the mod, or its model has no elements of its own.
pub fn content_block_model(root: &Path, id: &str) -> Option<BlockModel> {
    let (file, doc) = content_file(root, id, &[Format::Block])?;
    let elements = match crate::design::data::at(
        &doc,
        &[yog_ids::content_field::MODEL, yog_ids::model::ELEMENTS],
    )
    .and_then(Value::as_array)
    {
        Some(elements) => elements.clone(),
        None if doc
            .get(yog_ids::content_field::KIND)
            .and_then(Value::as_str)
            == Some("stairs") =>
        {
            vec![
                stair_element([0, 0, 0], [16, 8, 16]),
                stair_element([0, 8, 8], [16, 16, 16]),
            ]
        }
        None => return None,
    };
    let textures = model_textures(root, &doc, yog_ids::content_field::MODEL)
        .iter()
        .filter_map(|(k, v)| Some((k.clone(), texture_bytes(&file, v.as_str()?).ok()?)))
        .collect();
    Some((elements, textures))
}

fn stair_element(from: [u8; 3], to: [u8; 3]) -> Value {
    let face = |texture: &str| serde_json::json!({ yog_ids::model::TEXTURE: format!("{}{texture}", yog_formats::content::SLOT_MARK) });
    serde_json::json!({
        "from": from,
        "to": to,
        "faces": {
            "up": face("top"),
            "down": face("bottom"),
            "north": face("side"),
            "south": face("side"),
            "west": face("side"),
            "east": face("side")
        }
    })
}

/// A mod item's definition and, for potions, its effect-derived color.
pub fn content_item_appearance(root: &Path, id: &str) -> Option<(Value, Option<u32>)> {
    let ns = crate::design::project::mod_id(root)?;
    let (want_ns, want) = yog_ids::split(id);
    let want = if want_ns == yog_ids::ns::YOG {
        want.strip_prefix(yog_ids::split(yog_ids::yog::POTION_ALIAS).1)?
    } else if want_ns == ns {
        want
    } else {
        return None;
    };
    for file in crate::files::of(&root.join("content/items"), Format::Item) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Some(doc) = crate::design::data::read_object(&text) else {
            continue;
        };
        let Some(fid) = doc
            .get(yog_ids::content_field::ID)
            .and_then(Value::as_str)
            .or_else(|| file.file_stem().and_then(|s| s.to_str()))
        else {
            continue;
        };
        if fid.split_once(':').map_or(fid, |(_, p)| p) != want {
            continue;
        }
        let definition = Value::Object(doc);
        let color = potion_effect_color(root, &definition);
        return Some((definition, color));
    }
    None
}

fn potion_effect_color(root: &Path, item: &Value) -> Option<u32> {
    let effects = crate::design::data::at(
        item,
        &[
            yog_ids::content_field::ITEM,
            yog_ids::content_field::POTION,
            yog_ids::content_field::EFFECTS,
        ],
    )?
    .as_array()?;
    let mut channels = [0u64; 3];
    let mut count = 0u64;
    for effect in effects {
        let Some(id) = effect
            .get(yog_ids::content_field::EFFECT)
            .and_then(Value::as_str)
        else {
            continue;
        };
        let Some(color) = content_effect_color(root, id) else {
            continue;
        };
        channels[0] += u64::from((color >> 16) & 0xFF);
        channels[1] += u64::from((color >> 8) & 0xFF);
        channels[2] += u64::from(color & 0xFF);
        count += 1;
    }
    (count > 0).then(|| {
        (((channels[0] / count) as u32) << 16)
            | (((channels[1] / count) as u32) << 8)
            | (channels[2] / count) as u32
    })
}

/// The color of the potion effect(s) defined by this `.yitem`.
pub fn item_potion_color(file: &Path) -> Option<u32> {
    let root = mod_root(file)?;
    let text = std::fs::read_to_string(file).ok()?;
    let item = Value::Object(crate::design::data::read_object(&text)?);
    potion_effect_color(&root, &item)
}

fn content_effect_color(root: &Path, reference: &str) -> Option<u32> {
    let mod_ns = crate::design::project::mod_id(root)?;
    let id = reference.strip_prefix("@effect:").unwrap_or(reference);
    let (ns, want) = yog_ids::split(id);
    if ns != mod_ns {
        return None;
    }
    for file in crate::files::of(&root.join("content/effects"), Format::Def) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Some(doc) = crate::design::data::read_object(&text) else {
            continue;
        };
        let Some(id) = doc
            .get(yog_ids::content_field::ID)
            .and_then(Value::as_str)
            .or_else(|| file.file_stem().and_then(|s| s.to_str()))
        else {
            continue;
        };
        if id.split_once(':').map_or(id, |(_, p)| p) == want {
            return doc
                .get(yog_ids::content_field::COLOR)
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
        }
    }
    None
}

/// The icon of the mod's item or block `id` as its `content/` file draws
/// it: the model's `layer0` (else its first texture), as png bytes.
pub fn content_icon(root: &Path, id: &str) -> Option<Vec<u8>> {
    let ns = crate::design::project::mod_id(root)?;
    let (want_ns, want) = yog_ids::split(id);
    if want_ns != ns {
        return None;
    }
    let files = [
        crate::files::of(&root.join(yog_ids::project::CONTENT), Format::Item),
        crate::files::of(&root.join(yog_ids::project::CONTENT), Format::Block),
    ]
    .concat();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Some(doc) = crate::design::data::read_object(&text).map(Value::Object) else {
            continue;
        };
        let fid = doc
            .get(yog_ids::content_field::ID)
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| file.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
        let fid = fid
            .split_once(':')
            .map_or(fid.as_str(), |(_, p)| p)
            .to_string();
        if fid != want {
            continue;
        }
        let textures = model_textures(root, &doc, yog_ids::content_field::MODEL);
        let tex = textures
            .get(yog_ids::model::LAYER0)
            .or_else(|| ICON_SLOTS.iter().find_map(|k| textures.get(*k)))
            .or_else(|| textures.values().next())?
            .as_str()?;
        return texture_bytes(&file, tex).ok();
    }
    None
}
