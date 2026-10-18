//! `.ytexture` files: the mod's textures as the game takes them. The
//! pictures are `.yres` images; each texture says which atlas it's packed
//! into. The textures of an atlas become **one** picture
//! (`<mod id>:atlas/<atlas>`) the game cuts up again (an `unstitch` source
//! of `assets/<ns>/atlases/<atlas>.json`); an animated texture, or one in no
//! atlas, is a picture of its own with its `.mcmeta`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Value, json};
use yog_formats::texture::{Animation, Frame, TextureFile};
use yog_ids::Format;
use yog_media::image::Image;

use super::Output;
use crate::yres::{RESOURCES_DIR, Refs};

/// The mod's textures: their ids, the files they become, and the pictures
/// an atlas took in (their own files left out of the build).
#[derive(Default)]
pub struct Textures {
    pub ids: BTreeSet<String>,
    pub outputs: Vec<Output>,
    pub packed: BTreeSet<String>,
}

/// Where an atlas's packed picture goes under the mod's textures: `atlas/blocks`.
const ATLAS_DIR: &str = "atlas";
/// What an atlas source of a whole texture, and of one cut from a picture, is.
const SINGLE: &str = "single";
const UNSTITCH: &str = "unstitch";

/// Every `.ytexture` under `resources/` of the mod at `root`; problems go to `errors`.
pub fn compile(root: &Path, mod_id: &str, refs: &Refs, errors: &mut Vec<String>) -> Textures {
    let resources = root.join(RESOURCES_DIR);
    let mut out = Textures::default();
    // Atlas → its packed textures (id, picture) and its own sources.
    let mut packed: BTreeMap<String, Vec<(String, Image)>> = BTreeMap::new();
    let mut singles: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for file in crate::yui::find_files(&resources, Format::Texture.ext()) {
        let shown = super::display(root, &file);
        let rel = file.strip_prefix(&resources).unwrap_or(&file).to_string_lossy().replace('\\', yog_ids::sep::PATH);
        let Some(id) = yog_formats::texture::id_of(mod_id, &rel) else { continue };
        let texture = match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| yog_data::from_text::<TextureFile>(&t).map_err(|e| e.to_string())) {
            Ok(r) => r.value,
            Err(e) => {
                errors.push(format!("{shown}: {e}"));
                continue;
            }
        };
        let reference = format!("{}.{}", texture.image.set, texture.image.key);
        let Some(picture) = refs.pictures.get(&reference) else {
            errors.push(format!("{shown}: image `@{reference}`: no such image in {RESOURCES_DIR}/*.{}", Format::Res.ext()));
            continue;
        };
        if let Some(atlas) = &texture.atlas
            && yog_registry::Atlas::find(atlas).is_none()
        {
            let known: Vec<&str> = yog_registry::Atlas::game().iter().map(|a| a.id.as_str()).collect();
            errors.push(format!("{shown}: atlas `{atlas}`: one of {}", known.join(", ")));
            continue;
        }
        let atlas = texture.atlas.as_deref().map(yog_ids::qualify);
        match (&atlas, &texture.animation) {
            // Packed with the atlas's other textures.
            (Some(atlas), None) if !texture.blur && !texture.clamp => match yog_media::image::decode(&picture.png) {
                Ok(img) => {
                    packed.entry(atlas.clone()).or_default().push((id.clone(), img));
                    out.packed.insert(picture.output.clone());
                }
                Err(e) => errors.push(format!("{shown}: {e}")),
            },
            // A picture of its own: animated, filtered, or in no atlas.
            _ => {
                let (ns, path) = yog_ids::split(&id);
                let target = format!("{}/{ns}/{}", yog_ids::pack::ASSETS, yog_ids::texture_file(path));
                if let Some(meta) = mcmeta(&texture) {
                    out.outputs.push((format!("{target}{}", yog_ids::res::MCMETA), super::pretty(&meta)));
                }
                out.outputs.push((target, picture.png.clone()));
                if let Some(atlas) = atlas {
                    singles.entry(atlas).or_default().push(json!({ "type": SINGLE, "resource": id }));
                }
            }
        }
        out.ids.insert(id);
    }
    for atlas in packed.keys().chain(singles.keys()).cloned().collect::<BTreeSet<_>>() {
        let mut sources = singles.remove(&atlas).unwrap_or_default();
        if let Some(textures) = packed.remove(&atlas) {
            let (sheet, regions) = pack(&textures);
            let picture_id = yog_ids::join(mod_id, &format!("{ATLAS_DIR}{}{}", yog_ids::sep::PATH, yog_ids::split(&atlas).1));
            match yog_media::image::encode_png(&sheet) {
                Ok(png) => out.outputs.push((format!("{}/{mod_id}/{}", yog_ids::pack::ASSETS, yog_ids::texture_file(yog_ids::split(&picture_id).1)), png)),
                Err(e) => errors.push(format!("atlas {atlas}: {e}")),
            }
            sources.insert(0, json!({
                "type": UNSTITCH,
                "resource": picture_id,
                "divisor_x": sheet.width,
                "divisor_y": sheet.height,
                "regions": regions.iter().map(|(id, x, y, w, h)| json!({ "sprite": id, "x": x, "y": y, "width": w, "height": h })).collect::<Vec<_>>(),
            }));
        }
        let (ns, path) = yog_ids::split(&atlas);
        out.outputs.push((format!("{}/{ns}/atlases/{path}{}", yog_ids::pack::ASSETS, yog_ids::res::JSON), super::pretty(&json!({ "sources": sources }))));
    }
    out
}

/// `textures` packed into one picture, row by row, tallest first: the
/// picture and each texture's region (id, x, y, width, height).
fn pack(textures: &[(String, Image)]) -> (Image, Vec<(String, u32, u32, u32, u32)>) {
    let mut order: Vec<&(String, Image)> = textures.iter().collect();
    order.sort_by(|a, b| b.1.height.cmp(&a.1.height).then_with(|| a.0.cmp(&b.0)));
    let area: u64 = textures.iter().map(|(_, i)| u64::from(i.width) * u64::from(i.height)).sum();
    let widest = textures.iter().map(|(_, i)| i.width).max().unwrap_or(1);
    let width = (area as f64).sqrt().ceil().max(f64::from(widest)) as u32;
    let width = width.next_power_of_two();
    let (mut x, mut y, mut row_h) = (0, 0, 0);
    let mut regions = Vec::new();
    for (id, img) in &order {
        if x + img.width > width {
            (x, y, row_h) = (0, y + row_h, 0);
        }
        regions.push((id.clone(), x, y, img.width, img.height));
        x += img.width;
        row_h = row_h.max(img.height);
    }
    let height = (y + row_h).max(1).next_power_of_two();
    let mut sheet = Image::new(width, height);
    for ((_, img), (_, rx, ry, _, _)) in order.iter().zip(&regions) {
        for row in 0..img.height {
            let from = (row * img.width * 4) as usize;
            let to = (((ry + row) * width + rx) * 4) as usize;
            sheet.rgba[to..to + (img.width * 4) as usize].copy_from_slice(&img.rgba[from..from + (img.width * 4) as usize]);
        }
    }
    (sheet, regions)
}

/// The game's `.mcmeta` of a texture, when it animates or filters.
fn mcmeta(t: &TextureFile) -> Option<Value> {
    let mut meta = serde_json::Map::new();
    if let Some(a) = &t.animation {
        meta.insert("animation".into(), animation(a));
    }
    if t.blur || t.clamp {
        meta.insert("texture".into(), json!({ "blur": t.blur, "clamp": t.clamp }));
    }
    (!meta.is_empty()).then_some(Value::Object(meta))
}

/// An animation the game's way.
fn animation(a: &Animation) -> Value {
    let mut anim = serde_json::Map::new();
    anim.insert("frametime".into(), json!(a.frame_time()));
    if a.interpolate {
        anim.insert("interpolate".into(), json!(true));
    }
    if !a.frames.is_empty() {
        let frames = a.frames.iter().map(|f| match f.time {
            Some(time) => json!({ "index": f.index, "time": time }),
            None => json!(f.index),
        });
        anim.insert("frames".into(), Value::Array(frames.collect()));
    }
    if let Some(w) = a.width {
        anim.insert("width".into(), json!(w));
    }
    if let Some(h) = a.height {
        anim.insert("height".into(), json!(h));
    }
    Value::Object(anim)
}

/// The animation a game `.mcmeta` describes.
pub fn animation_of_mcmeta(mcmeta: &[u8]) -> Option<Animation> {
    let a = serde_json::from_slice::<Value>(mcmeta).ok()?.get("animation")?.clone();
    let u32_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
    Some(Animation {
        frame_time: u32_of(&a, "frametime"),
        interpolate: a.get("interpolate").and_then(Value::as_bool).unwrap_or(false),
        frames: a
            .get("frames")
            .and_then(Value::as_array)
            .map(|fs| {
                fs.iter()
                    .filter_map(|f| match f.as_u64() {
                        Some(i) => Some(Frame { index: u32::try_from(i).ok()?, time: None }),
                        None => Some(Frame { index: u32_of(f, "index")?, time: u32_of(f, "time") }),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        width: u32_of(&a, "width"),
        height: u32_of(&a, "height"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yres::Picture;

    fn picture(w: u32, h: u32) -> Picture {
        Picture { png: yog_media::image::encode_png(&Image { width: w, height: h, rgba: vec![255; (w * h * 4) as usize] }).unwrap(), output: String::new() }
    }

    #[test]
    fn an_atlas_is_one_picture_and_an_animation_its_own() {
        let dir = std::env::temp_dir().join(format!("yog-ytexture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("resources/block")).unwrap();
        std::fs::write(dir.join("resources/block/ruby.ytexture"), r#"{ image: "@Images.block.ruby", atlas: "minecraft:blocks" }"#).unwrap();
        std::fs::write(dir.join("resources/block/opal.ytexture"), r#"{ image: "@Images.block.opal", atlas: "minecraft:blocks" }"#).unwrap();
        std::fs::write(dir.join("resources/block/glow.ytexture"), r#"{ image: "@Images.block.glow", atlas: "minecraft:blocks", animation: { frame_time: 3 } }"#).unwrap();
        let mut refs = Refs::default();
        for (name, h) in [("block.ruby", 16), ("block.opal", 16), ("block.glow", 32)] {
            refs.pictures.insert(format!("Images.{name}"), picture(16, h));
        }
        let mut errors = Vec::new();
        let t = compile(&dir, "mymod", &refs, &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(t.ids.len(), 3);
        let names: BTreeSet<&str> = t.outputs.iter().map(|(p, _)| p.as_str()).collect();
        assert!(names.contains("assets/mymod/textures/atlas/blocks.png"), "{names:?}");
        assert!(names.contains("assets/mymod/textures/block/glow.png.mcmeta"), "{names:?}");
        let atlas: Value = serde_json::from_slice(&t.outputs.iter().find(|(p, _)| p == "assets/minecraft/atlases/blocks.json").unwrap().1).unwrap();
        assert_eq!(atlas["sources"][0]["type"], UNSTITCH);
        assert_eq!(atlas["sources"][0]["regions"].as_array().unwrap().len(), 2);
        assert_eq!(atlas["sources"][1], json!({ "type": SINGLE, "resource": "mymod:block/glow" }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mcmeta_round_trips() {
        let meta = br#"{"animation":{"frametime":2,"interpolate":true,"frames":[1,{"index":0,"time":5}]}}"#;
        let a = animation_of_mcmeta(meta).unwrap();
        assert_eq!(json!({ "animation": animation(&a) }), serde_json::from_slice::<Value>(meta).unwrap());
    }
}
