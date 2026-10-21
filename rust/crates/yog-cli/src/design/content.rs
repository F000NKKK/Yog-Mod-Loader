//! The block/item model editor's backend (`.yblock` / `.yitem`, see
//! `docs/blocks-items.md`): textures as the editor names them, the
//! texture picker's list, vanilla/parent models for preview, and
//! `yog import`.

use crate::design::assets::Resources;
use crate::design::project::mod_root;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use yog_ids::Format;

/// The slots of a block model whose texture stands for it as an icon, best first.
mod choices;
mod looks;
mod texture;

pub use choices::{
    TextureChoice, armor_materials, sound_groups, strings, tags, textures, tool_materials,
};
pub use looks::{
    content_block_faces, content_block_model, content_icon, content_item_appearance,
    item_potion_color,
};
pub use texture::{
    Animation, AnimationFrame, Texture, body_texture, own_texture, texture, write_animation,
    write_texture,
};

const ICON_SLOTS: [&str; 4] = [
    yog_ids::texture_slot::ALL,
    yog_ids::texture_slot::SIDE,
    yog_ids::texture_slot::TOP,
    yog_ids::texture_slot::TEXTURE,
];

/// `[mod] id` of the mod at `root`.
/// The id a content file's texture names: `@texture:<ns>:<path>`, or an id
/// (`<ns>:<path>`; a bare path is the game's).
fn texture_id(texture: &str) -> String {
    yog_ids::qualify(
        texture
            .strip_prefix(yog_ids::reference::TEXTURE)
            .unwrap_or(texture),
    )
}

/// The png bytes of a texture as a content file names it: `@Set.name`,
/// `path.png` (in `resources/`), or a texture id (`ns:block/x`).
fn texture_bytes(file: &Path, texture: &str) -> Result<Vec<u8>, String> {
    match texture_file(file, texture, "png") {
        Ok(bytes) => Ok(bytes),
        Err(original) => {
            // A 1.20.2+ GUI sprite (`minecraft:gui/sprites/<atlas>`) on a game without the atlas: its 1.20.1 region.
            let (ns, path) = yog_ids::split(texture);
            let Some(legacy) = path
                .strip_prefix(yog_ids::res::GUI_SPRITES)
                .and_then(yog_gfx::image::sprite_by_atlas)
                .and_then(|s| s.legacy.as_ref())
                .filter(|_| ns == yog_ids::ns::MINECRAFT)
            else {
                return Err(original);
            };
            let sheet = texture_file(file, &yog_gfx::image::short_source(&legacy.texture), "png")?;
            let r = legacy.region;
            crop_png(&sheet, r.x as u32, r.y as u32, r.w as u32, r.h as u32)
                .map_err(|e| format!("{original}; the older versions' sprite sheet: {e}"))
        }
    }
}

fn crop_png(bytes: &[u8], x: u32, y: u32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let image = crate::design::assets::decode_png(bytes).ok_or("invalid PNG")?;
    if x + width > image.width || y + height > image.height {
        return Err(format!(
            "crop {x},{y} {width}x{height} exceeds {}x{}",
            image.width, image.height
        ));
    }
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for row in y..y + height {
        let start = ((row * image.width + x) * 4) as usize;
        rgba.extend_from_slice(&image.rgba[start..start + (width * 4) as usize]);
    }
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    drop(writer);
    Ok(output)
}

/// A file of texture `texture` — its `png`, or its `png.mcmeta` — as the
/// build makes it (the mod's own, through the CLI service) or the game has it.
fn texture_file(file: &Path, texture: &str, ext: &str) -> Result<Vec<u8>, String> {
    let id = texture_id(texture);
    let (ns, path) = yog_ids::split(&id);
    Resources::for_mod(mod_root(file).as_deref())
        .read(ns, &format!("{}{path}.{ext}", yog_ids::res::TEXTURES))
        .ok_or_else(|| {
            format!(
                "texture {id} not found (the mod's .{}, or the Minecraft client jar)",
                yog_ids::Format::Texture.ext()
            )
        })
}

/// The mod's `content/` file defining item or block `id` (`kind`:
/// `yitem` or `yblock`), parsed.
fn content_file(root: &Path, id: &str, formats: &[Format]) -> Option<(PathBuf, Value)> {
    let ns = crate::design::project::mod_id(root)?;
    let (want_ns, want) = yog_ids::split(id);
    if want_ns != ns {
        return None;
    }
    for format in formats {
        for file in crate::files::of(&root.join(yog_ids::project::CONTENT), *format) {
            let Some(doc) = crate::files::read_source(&file)
                .ok()
                .and_then(|t| crate::design::data::read_object(&t))
                .map(Value::Object)
            else {
                continue;
            };
            let fid = doc
                .get(yog_ids::content_field::ID)
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| file.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
            if fid.split_once(':').map_or(fid.as_str(), |(_, p)| p) == want {
                return Some((file, doc));
            }
        }
    }
    None
}

/// The textures of a content file's model (`field`) as the build takes them:
/// its material's (`.ymaterial`) under its own, as `@texture:` text.
fn model_textures(root: &Path, doc: &Value, field: &str) -> serde_json::Map<String, Value> {
    let Some(model) = doc.get(field) else {
        return serde_json::Map::new();
    };
    let read =
        yog_data::read_value::<yog_formats::content::Model>(&yog_data::json::from_json(model));
    let (Ok(mut model), Some(ns)) = (read, crate::design::project::mod_id(root)) else {
        // Not a model the build would take: its textures as written.
        return model
            .get(yog_ids::model::TEXTURES)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
    };
    if let Some(material) = model.material.clone()
        && let Ok(m) = crate::content::material::load(root, &ns, &material)
    {
        crate::content::material::apply(&mut model, &m);
    }
    model
        .textures
        .0
        .iter()
        .map(|(slot, t)| {
            (
                slot.clone(),
                yog_data::json::to_json(&yog_data::Schema::write(t)),
            )
        })
        .collect()
}

/// A model by id, parents flattened: `{textures, elements?, display, parent?}`
/// where `parent` is the vanilla sprite/builtin root it ends at, if any.
pub fn parent_model(file: &Path, id: &str) -> Result<Value, String> {
    let resources = Resources::for_mod(mod_root(file).as_deref());
    let mut textures = Map::new();
    let mut display = Map::new();
    let mut elements: Option<Value> = None;
    let mut current = id.to_string();
    let mut root_parent: Option<String> = None;
    for _ in 0..16 {
        let (ns, path) = yog_ids::split(&current);
        let bytes = resources
            .read(ns, &yog_ids::model_file(path))
            .ok_or_else(|| format!("model {ns}:{path} not found"))?;
        let m: Map<String, Value> =
            serde_json::from_slice(&bytes).map_err(|e| format!("{ns}:{path}: {e}"))?;
        if let Some(Value::Object(t)) = m.get(yog_ids::model::TEXTURES) {
            for (k, v) in t {
                textures.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        if let Some(Value::Object(d)) = m.get(yog_ids::model::DISPLAY) {
            for (k, v) in d {
                display.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        if elements.is_none() {
            elements = m.get(yog_ids::model::ELEMENTS).cloned();
        }
        match m.get(yog_ids::model::PARENT).and_then(Value::as_str) {
            None => break,
            Some(p) if p.contains(yog_ids::res::BUILTIN) => {
                root_parent = Some(p.to_string());
                break;
            }
            Some(p) => {
                let full = yog_ids::qualify(p);
                // A sprite's parent (`item/generated`): where the model ends.
                if crate::content::TEMPLATES
                    .iter()
                    .any(|t| t.sprite && t.parent == full)
                {
                    root_parent = Some(full.clone());
                }
                current = full;
            }
        }
    }
    let mut out = Map::new();
    out.insert(yog_ids::model::TEXTURES.into(), Value::Object(textures));
    out.insert(yog_ids::model::DISPLAY.into(), Value::Object(display));
    if let Some(e) = elements {
        out.insert(yog_ids::model::ELEMENTS.into(), e);
    }
    if let Some(p) = root_parent {
        out.insert(yog_ids::model::PARENT.into(), Value::String(p));
    }
    Ok(Value::Object(out))
}

/// `yog import <kind> <source> [--model <id>] [--force]` (`kind`: `resourcepack`,
/// `bbmodel`, or `bbmodel-item`) in the mod of
/// `file` (a content file or the mod folder) → what it printed.
pub fn import(
    file: &Path,
    kind: &str,
    source: &str,
    model: Option<&str>,
    force: bool,
) -> Result<String, String> {
    let root = mod_root(file).ok_or("not in a mod (no yog.toml)")?;
    // `bbmodel-item`: a Blockbench model as an item.
    let (what, item) = match kind {
        "resourcepack" => ("resourcepack", false),
        "bbmodel" => ("bbmodel", false),
        "bbmodel-item" => ("bbmodel", true),
        _ => return Err(format!("unknown import kind {kind}")),
    };
    let mut cmd = crate::design::this_cli();
    cmd.current_dir(&root).args(["import", what, source]);
    if let Some(m) = model.filter(|m| !m.is_empty()) {
        cmd.args([if what == "bbmodel" { "--id" } else { "--model" }, m]);
    } else if what == "resourcepack" {
        cmd.arg("--all");
    }
    if item {
        cmd.arg("--item");
    }
    if force {
        cmd.arg("--force");
    }
    let out = cmd
        .output()
        .map_err(|e| format!("failed to run yog: {e}"))?;
    let text = String::from_utf8_lossy(&out.stderr).replace("yog: error: ", "");
    if out.status.success() {
        Ok(text.trim().to_string())
    } else {
        Err(text.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn animation_reads_the_games_mcmeta() {
        let meta = r#"{"animation":{"frametime":3,"frames":[0,{"index":2,"time":10},7]}}"#;
        let a = Animation::read(meta, (16, 64)).unwrap();
        assert_eq!((a.frame_width, a.frame_height), (16, 16));
        // Frame 7 is past the four the image holds.
        assert_eq!(
            a.frames,
            vec![
                AnimationFrame { index: 0, time: 3 },
                AnimationFrame { index: 2, time: 10 }
            ]
        );
        assert!(!a.interpolate);
        let all = Animation::read(r#"{"animation":{}}"#, (16, 48)).unwrap();
        assert_eq!(
            all.frames.iter().map(|f| f.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(Animation::read("{}", (16, 16)).is_none());
    }

    #[test]
    fn animation_round_trips() {
        let a = Animation {
            frame_width: 16,
            frame_height: 16,
            frames: vec![
                AnimationFrame { index: 1, time: 4 },
                AnimationFrame { index: 0, time: 1 },
            ],
            interpolate: true,
        };
        assert_eq!(Animation::of(&a.to_format((16, 32)), (16, 32)), Some(a));
    }

    use super::*;

    #[test]
    fn png_size() {
        let mut png = vec![
            0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R',
        ];
        png.extend(16u32.to_be_bytes());
        png.extend(64u32.to_be_bytes());
        assert_eq!(super::texture::image_size(&png), Some((16, 64)));
    }

    #[test]
    fn sound_groups_are_the_games_then_the_mods() {
        let dir = std::env::temp_dir().join(format!("idle-sounds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        std::fs::write(
            dir.join("content/crystal.ydef"),
            r#"{ kind: "sound_group", id: "crystal" }"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("content/mythril.ydef"),
            r#"{ kind: "level", id: "mythril" }"#,
        )
        .unwrap();
        let all = sound_groups(&dir.join("content/ore.yblock"));
        assert!(all.contains(&"stone".to_string()) && all.contains(&"amethyst".to_string()));
        assert_eq!(
            all.last().map(String::as_str),
            Some("@sound_group:mymod:crystal")
        );
        assert!(!all.iter().any(|s| s.contains("mythril")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolves_yres_images_and_resource_paths() {
        let dir = std::env::temp_dir().join(format!("idle-content-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        let png = yog_media::image::encode_png(&yog_media::image::Image {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 255],
        })
        .unwrap();
        let file = dir.join("content/ruby.yblock");
        // Painting a new texture of the mod makes its picture and .ytexture.
        assert_eq!(
            own_texture(&file, "@texture:mymod:block/ruby")
                .map(|(_, id)| id)
                .as_deref(),
            Some("mymod:block/ruby")
        );
        write_texture(&file, "@texture:mymod:block/ruby", png.clone()).unwrap();
        assert_eq!(
            texture_bytes(&file, "@texture:mymod:block/ruby").unwrap(),
            png
        );
        assert!(
            textures(&file)
                .iter()
                .any(|c| c.value == "@texture:mymod:block/ruby")
        );
        assert_eq!(own_texture(&file, "@texture:minecraft:item/apple"), None);
        assert_eq!(own_texture(&file, "#layer0"), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn item_appearance_reads_its_model_and_effect_colors() {
        let dir = std::env::temp_dir().join(format!("idle-item-appearance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content/items")).unwrap();
        std::fs::create_dir_all(dir.join("content/effects")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        std::fs::write(
            dir.join("content/items/potion.yitem"),
            r#"{ model: { parent: "minecraft:item/potion" }, item: { potion: { effects: [{ effect: "@effect:mymod:glow" }] } } }"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("content/effects/glow.ydef"),
            r#"{ kind: "effect", id: "glow", color: 1193046 }"#,
        )
        .unwrap();

        let (definition, color) = content_item_appearance(&dir, "mymod:potion").unwrap();
        assert_eq!(
            definition.pointer("/model/parent").and_then(Value::as_str),
            Some("minecraft:item/potion")
        );
        assert_eq!(color, Some(0x123456));
        assert_eq!(
            item_potion_color(&dir.join("content/items/potion.yitem")),
            Some(0x123456)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stairs_without_custom_elements_get_a_step_shape() {
        let dir = std::env::temp_dir().join(format!("idle-stairs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content/blocks")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        std::fs::write(
            dir.join("content/blocks/stairs.yblock"),
            r#"{ kind: "stairs", model: { textures: { top: "block/top.png", bottom: "block/bottom.png", side: "block/side.png" } } }"#,
        )
        .unwrap();

        let (elements, _) = content_block_model(&dir, "mymod:stairs").unwrap();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0]["from"], serde_json::json!([0, 0, 0]));
        assert_eq!(elements[0]["to"], serde_json::json!([16, 8, 16]));
        assert_eq!(elements[1]["from"], serde_json::json!([0, 8, 8]));
        assert_eq!(elements[1]["to"], serde_json::json!([16, 16, 16]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
