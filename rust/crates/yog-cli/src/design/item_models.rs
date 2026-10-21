//! Items as the game's item models show them in a GUI, for what plain
//! textures don't cover: layered models (with their tints — spawn eggs,
//! potions, leather) and `builtin/entity` heads, drawn as a 3D head.

use std::collections::HashMap;
use yog_sync::Arc;

use serde_json::Value;

use super::assets::{Image, Resources, decode_png};
use super::blocks3d::Faces;
use yog_registry::ItemLooks;

/// How an item looks in a slot.
pub enum Look {
    /// A flat picture: its model's layers.
    Flat(Arc<Image>),
    /// A mob's head (skulls): a cube with these faces.
    Head(Faces),
}

/// `id`'s look from its item model; `None` when it has no model this
/// understands (callers then guess its texture by name).
pub fn look(res: &Resources, id: &str) -> Option<Look> {
    let looks = ItemLooks::game();
    let id = yog_ids::qualify(id);
    let (ns, path) = yog_ids::split(&id);
    let appearance = res
        .mod_root
        .as_deref()
        .and_then(|root| crate::design::content::content_item_appearance(root, &id));
    // Yog's alias of a game potion is drawn as the game's potion.
    let alias = id.starts_with(yog_ids::yog::POTION_ALIAS);
    let model = if alias {
        looks.models.potion.clone()
    } else {
        yog_ids::join(ns, &format!("{}{path}", yog_ids::res::ITEM_DIR))
    };
    let (parents, textures) = model_chain(
        res,
        &model,
        appearance.as_ref().and_then(|(item, _)| item.get(yog_ids::content_field::MODEL)),
    );
    let is = |m: &str| parents.iter().any(|x| yog_ids::qualify(x) == m);
    if looks.models.heads.iter().any(|m| is(m))
        && let Some(tex) = looks.head(path)
    {
        return head(res, tex).map(Look::Head);
    }
    let mut layers = Vec::new();
    for i in 0.. {
        let Some(t) = textures.get(&format!("layer{i}")) else {
            break;
        };
        layers.push(t.clone());
    }
    if layers.is_empty() {
        return None;
    }
    // The tints the game's item colors give each layer.
    let tints: Vec<Option<u32>> = if is(yog_ids::yog::SPAWN_EGG_MODEL) {
        // A mod's egg: Yog's egg, tinted as its item definition says.
        definition_tints(res, ns, path)
    } else if is(&looks.models.spawn_egg) {
        let mob = path.strip_suffix(yog_ids::res::SPAWN_EGG_SUFFIX)?;
        let (base, spots) = looks.spawn_egg(mob)?;
        vec![Some(base.0), Some(spots.0)]
    } else if let Some(color) = appearance.as_ref().and_then(|(_, color)| *color) {
        vec![Some(color)]
    } else {
        // An alias without effects of its own is the game's plain potion.
        let tinted_as = if alias { model_item(&looks.models.potion) } else { id.clone() };
        looks.tint(&tinted_as).map(|c| vec![Some(c.0)]).unwrap_or_default()
    };
    let mut out: Option<Image> = None;
    for (i, layer) in layers.iter().enumerate() {
        let (lns, lpath) = yog_ids::split(layer);
        let Some(img) = res
            .read(lns, &yog_ids::texture_file(lpath))
            .and_then(|b| decode_png(&b))
        else {
            continue;
        };
        let img = first_frame(img);
        let tint = tints.get(i).copied().flatten();
        match &mut out {
            None => out = Some(tinted(img, tint)),
            Some(base) if base.width == img.width && base.height == img.height => {
                over(base, &tinted(img, tint))
            }
            Some(_) => {}
        }
    }
    out.map(|i| Look::Flat(Arc::new(i)))
}


/// The constant tints item `ns:path`'s definition (`items/<path>.json`, 1.21.4+) gives its layers, in order.
fn definition_tints(res: &Resources, ns: &str, path: &str) -> Vec<Option<u32>> {
    use crate::consts::item_def;
    let file = format!("{}{path}{}", yog_ids::res::ITEM_DEFINITIONS, yog_ids::res::JSON);
    let Some(def) = res.read(ns, &file).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
        return Vec::new();
    };
    let tints = def[item_def::MODEL][item_def::TINTS].as_array().cloned().unwrap_or_default();
    tints
        .iter()
        .map(|t| (t[item_def::TYPE] == item_def::CONSTANT).then(|| t[item_def::VALUE].as_i64()).flatten().map(|v| v as u32))
        .collect()
}

/// The item an item model is of: `minecraft:item/potion` → `minecraft:potion`.
fn model_item(model: &str) -> String {
    let (ns, path) = yog_ids::split(model);
    yog_ids::join(ns, path.strip_prefix(yog_ids::res::ITEM_DIR).unwrap_or(path))
}

/// A model's parents (itself first) and its textures, the child's winning.
fn model_chain(
    res: &Resources,
    model: &str,
    content_model: Option<&Value>,
) -> (Vec<String>, HashMap<String, String>) {
    let mut chain = Vec::new();
    let mut textures = HashMap::new();
    let mut at = model.to_string();
    let mut pending = content_model.cloned();
    for _ in 0..8 {
        chain.push(at.clone());
        let (ns, path) = yog_ids::split(&at);
        let json = pending.take().or_else(|| {
            res.read(ns, &yog_ids::model_file(path))
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        });
        let Some(json) = json else {
            break;
        };
        if let Some(t) = json.get(yog_ids::model::TEXTURES).and_then(Value::as_object) {
            for (k, v) in t {
                if let Some(v) = v.as_str() {
                    textures.entry(k.clone()).or_insert_with(|| v.to_string());
                }
            }
        }
        match json.get(yog_ids::model::PARENT).and_then(Value::as_str) {
            Some(p) => at = p.to_string(),
            None => break,
        }
    }
    (chain, textures)
}

/// An animated texture's first frame (square, at the top).
fn first_frame(img: Image) -> Image {
    if img.height <= img.width {
        return img;
    }
    crop(&img, 0, 0, img.width, img.width)
}

fn crop(img: &Image, x: u32, y: u32, w: u32, h: u32) -> Image {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for yy in y..(y + h).min(img.height) {
        for xx in x..(x + w).min(img.width) {
            rgba.extend_from_slice(&img.pixel(xx, yy).to_rgba8());
        }
    }
    let w = w.min(img.width.saturating_sub(x));
    let h = h.min(img.height.saturating_sub(y));
    Image { width: w, height: h, rgba }
}

/// `img` with its colors multiplied by `tint` (`0xRRGGBB`).
fn tinted(mut img: Image, tint: Option<u32>) -> Image {
    if let Some(t) = tint {
        let m = [(t >> 16) & 0xFF, (t >> 8) & 0xFF, t & 0xFF];
        for px in img.pixels_mut() {
            for c in 0..3 {
                px[c] = (px[c] as u32 * m[c] / 255) as u8;
            }
        }
    }
    img
}

/// `top` drawn over `base` (alpha blended).
fn over(base: &mut Image, top: &Image) {
    for (b, t) in base.pixels_mut().iter_mut().zip(top.pixels()) {
        let a = t[3] as u32;
        for c in 0..3 {
            b[c] = ((t[c] as u32 * a + b[c] as u32 * (255 - a)) / 255) as u8;
        }
        b[3] = (a + b[3] as u32 * (255 - a) / 255).min(255) as u8;
    }
}

/// A head's faces from its mob's texture (the head cube's UVs, 8 px at 64 wide).
fn head(res: &Resources, tex: &str) -> Option<Faces> {
    let (ns, path) = yog_ids::split(tex);
    let img = decode_png(&res.read(ns, &yog_ids::texture_file(path))?)?;
    let u = img.width / 64;
    let f = |x: u32, y: u32| Some(Arc::new(crop(&img, x * u, y * u, 8 * u, 8 * u)));
    // up, down, north (the face), south, west, east
    Some([f(8, 0), f(16, 0), f(8, 8), f(24, 8), f(16, 8), f(0, 8)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(pixel: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixel).unwrap();
        drop(writer);
        bytes
    }

    #[test]
    fn content_potion_uses_vanilla_layers_and_effect_color() {
        let dir = std::env::temp_dir().join(format!("idle-item-model-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content/items")).unwrap();
        std::fs::create_dir_all(dir.join("content/effects")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"hexcasting\"\n").unwrap();
        std::fs::write(
            dir.join("content/items/enlarge_grid.yitem"),
            r#"{ model: { parent: "minecraft:item/potion" }, item: { potion: { effects: [{ effect: "@effect:hexcasting:enlarge_grid" }] } } }"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("content/effects/enlarge_grid.ydef"),
            r#"{ kind: "effect", id: "enlarge_grid", color: 13137407 }"#,
        )
        .unwrap();
        // The game's potion, in a stand-in client jar.
        let jar = dir.join("client.jar");
        {
            use std::io::Write as _;
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
            let files: [(&str, Vec<u8>); 3] = [
                ("assets/minecraft/models/item/potion.json", br#"{"textures":{"layer0":"minecraft:item/potion_overlay","layer1":"minecraft:item/potion"}}"#.to_vec()),
                ("assets/minecraft/textures/item/potion_overlay.png", png([255, 255, 255, 255])),
                ("assets/minecraft/textures/item/potion.png", png([0, 0, 0, 0])),
            ];
            for (name, bytes) in files {
                zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(&bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        let res = Resources {
            mod_root: Some(dir.clone()),
            vanilla: super::super::assets::Vanilla::from_jar(jar, "1.21.1").map(yog_sync::Arc::new),
        };
        let Some(Look::Flat(image)) = look(&res, "hexcasting:enlarge_grid") else {
            panic!("content potion should resolve to a layered item model");
        };
        let color = 13_137_407u32;
        assert_eq!(
            &image.rgba[..],
            &[
                ((color >> 16) & 0xFF) as u8,
                ((color >> 8) & 0xFF) as u8,
                (color & 0xFF) as u8,
                255,
            ]
        );
        assert!(matches!(
            look(&res, "yog:potion/awkward"),
            Some(Look::Flat(_))
        ));
        let Some(Look::Flat(alias)) = look(&res, "yog:potion/enlarge_grid") else {
            panic!("potion alias should resolve to the configured potion");
        };
        assert_eq!(alias.rgba, image.rgba);
        let _ = std::fs::remove_dir_all(dir);
    }
}
