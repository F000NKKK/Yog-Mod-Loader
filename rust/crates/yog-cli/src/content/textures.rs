//! `.ytexture` files: the mod's textures as the game takes them. The
//! pictures are `.yres` images; each texture says which atlas it's packed
//! into. The textures of an atlas become **one** picture
//! (`<mod id>:atlas/<atlas>`) the game cuts up again (an `unstitch` source
//! of `assets/<ns>/atlases/<atlas>.json`); an animated texture, or one in no
//! atlas, is a picture of its own with its `.mcmeta`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Value, json};
use yog_formats::texture::TextureFile;
use yog_ids::Format;
use yog_ids::atlas_source as src;
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

/// What the textures make so far: the outputs, and per atlas the textures
/// packed into its picture (sprite, picture) and its sources of their own.
#[derive(Default)]
struct Building {
    out: Textures,
    packed: BTreeMap<String, Vec<(String, Image)>>,
    singles: BTreeMap<String, Vec<Value>>,
}

/// Every `.ytexture` under `resources/` of the mod at `root`; problems go to `errors`.
pub fn compile(root: &Path, mod_id: &str, refs: &Refs, errors: &mut Vec<String>) -> Textures {
    let resources = root.join(RESOURCES_DIR);
    let mut b = Building::default();
    for file in crate::yui::find_files(&resources, Format::Texture.ext()) {
        let shown = super::display(root, &file);
        let rel = file.strip_prefix(&resources).unwrap_or(&file).to_string_lossy().replace('\\', yog_ids::sep::PATH);
        let Some(id) = yog_formats::texture::id_of(mod_id, &rel) else { continue };
        if let Err(e) = texture(&mut b, &file, &id, refs) {
            errors.push(format!("{shown}: {e}"));
        }
    }
    for atlas in b.packed.keys().chain(b.singles.keys()).cloned().collect::<BTreeSet<_>>() {
        if let Err(e) = write_atlas(&mut b, &atlas, mod_id) {
            errors.push(format!("atlas {atlas}: {e}"));
        }
    }
    b.out
}

/// The `.ytexture` at `file` (texture `id`): packed into its atlas's
/// picture, or a picture of its own (animated, filtered, or in no atlas).
fn texture(b: &mut Building, file: &Path, id: &str, refs: &Refs) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let texture = yog_data::from_text::<TextureFile>(&text).map_err(|e| e.to_string())?.value;
    let picture = refs
        .pictures
        .get(&texture.image.name())
        .ok_or_else(|| format!("image `{}`: no such image in {RESOURCES_DIR}/*.{}", texture.image, Format::Res.ext()))?;
    if let Some(atlas) = &texture.atlas
        && yog_registry::Atlas::find(atlas).is_none()
    {
        let known: Vec<&str> = yog_registry::Atlas::game().iter().map(|a| a.id.as_str()).collect();
        return Err(format!("atlas `{atlas}`: one of {}", known.join(", ")));
    }
    let atlas = texture.atlas.as_deref().map(yog_ids::qualify);
    // Its sprite's name in the atlas: a bare atlas's without the folder (`mymod:clarity`).
    let (ns, path) = yog_ids::split(id);
    let sprite = yog_ids::join(ns, atlas.as_deref().and_then(yog_registry::Atlas::find).map_or(path, |a| a.sprite(path)));
    match (&atlas, &texture.animation) {
        (Some(atlas), None) if !texture.blur && !texture.clamp => {
            let img = yog_media::image::decode(&picture.png)?;
            b.packed.entry(atlas.clone()).or_default().push((sprite, img));
            b.out.packed.insert(picture.output.clone());
        }
        _ => {
            let target = format!("{}/{ns}/{}", yog_ids::pack::ASSETS, yog_ids::texture_file(path));
            if let Some(meta) = texture.game_mcmeta() {
                b.out.outputs.push((format!("{target}{}", yog_ids::res::MCMETA), super::pretty(&meta)));
            }
            b.out.outputs.push((target, picture.png.clone()));
            if let Some(atlas) = atlas {
                let mut single = json!({ src::TYPE: src::SINGLE, src::RESOURCE: id });
                if sprite != id {
                    single[src::SPRITE] = json!(sprite);
                }
                b.singles.entry(atlas).or_default().push(single);
            }
        }
    }
    b.out.ids.insert(id.to_string());
    Ok(())
}

/// Atlas `atlas`'s definition: its textures' packed picture, cut up again
/// (an `unstitch` source first), and its textures of their own.
fn write_atlas(b: &mut Building, atlas: &str, mod_id: &str) -> Result<(), String> {
    let mut sources = b.singles.remove(atlas).unwrap_or_default();
    if let Some(textures) = b.packed.remove(atlas) {
        let (sheet, regions) = pack(&textures);
        let picture_path = format!("{ATLAS_DIR}{}{}", yog_ids::sep::PATH, yog_ids::split(atlas).1);
        let png = yog_media::image::encode_png(&sheet)?;
        b.out.outputs.push((format!("{}/{mod_id}/{}", yog_ids::pack::ASSETS, yog_ids::texture_file(&picture_path)), png));
        let regions: Vec<Value> = regions
            .iter()
            .map(|(id, x, y, w, h)| json!({ src::SPRITE: id, src::X: x, src::Y: y, src::WIDTH: w, src::HEIGHT: h }))
            .collect();
        sources.insert(0, json!({
            src::TYPE: src::UNSTITCH,
            src::RESOURCE: yog_ids::join(mod_id, &picture_path),
            src::DIVISOR_X: sheet.width,
            src::DIVISOR_Y: sheet.height,
            src::REGIONS: regions,
        }));
    }
    let (ns, path) = yog_ids::split(atlas);
    let file = format!("{}/{ns}/{}{path}{}", yog_ids::pack::ASSETS, yog_ids::res::ATLASES, yog_ids::res::JSON);
    b.out.outputs.push((file, super::pretty(&json!({ src::SOURCES: sources }))));
    Ok(())
}

/// A texture's region in a packed picture: id, x, y, width, height.
type Region = (String, u32, u32, u32, u32);

/// `textures` packed into one picture, row by row, tallest first: the
/// picture and each texture's region (id, x, y, width, height).
fn pack(textures: &[(String, Image)]) -> (Image, Vec<Region>) {
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
        assert_eq!(atlas[src::SOURCES][0][src::TYPE], src::UNSTITCH);
        assert_eq!(atlas["sources"][0]["regions"].as_array().unwrap().len(), 2);
        assert_eq!(atlas["sources"][1], json!({ "type": SINGLE, "resource": "mymod:block/glow" }));
        // A status effect's icon is named without its folder, as the game looks it up.
        std::fs::create_dir_all(dir.join("resources/mob_effect")).unwrap();
        std::fs::write(dir.join("resources/mob_effect/clarity.ytexture"), r#"{ image: "@Images.mob_effect.clarity", atlas: "minecraft:mob_effects" }"#).unwrap();
        refs.pictures.insert("Images.mob_effect.clarity".into(), picture(18, 18));
        let t = compile(&dir, "mymod", &refs, &mut errors);
        let effects: Value = serde_json::from_slice(&t.outputs.iter().find(|(p, _)| p == "assets/minecraft/atlases/mob_effects.json").unwrap().1).unwrap();
        assert_eq!(effects["sources"][0]["regions"][0]["sprite"], "mymod:clarity");
        let _ = std::fs::remove_dir_all(&dir);
    }

}
