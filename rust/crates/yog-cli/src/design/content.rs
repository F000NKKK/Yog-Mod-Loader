//! The block/item model editor's backend (`.yblock` / `.yitem`, see
//! `docs/blocks-items.md`): textures as the editor names them, the
//! texture picker's list, vanilla/parent models for preview, and
//! `yog import`.

use yog_ids::Format;
use crate::design::assets::{Resources};
use crate::design::project::mod_root;
use base64::Engine as _;
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// `[mod] id` of the mod at `root`.
/// The id a content file's texture names: `@texture:<ns>:<path>`, or an id
/// (`<ns>:<path>`; a bare path is the game's).
fn texture_id(texture: &str) -> String {
    yog_ids::qualify(texture.strip_prefix(yog_ids::reference::TEXTURE).unwrap_or(texture))
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
            crop_png(&sheet, r.x as u32, r.y as u32, r.w as u32, r.h as u32).map_err(|e| format!("{original}; the 1.20.1 sprite: {e}"))
        }
    }
}

fn crop_png(bytes: &[u8], x: u32, y: u32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let image = crate::design::assets::decode_png(bytes).ok_or("invalid PNG")?;
    if x + width > image.width || y + height > image.height {
        return Err(format!("crop {x},{y} {width}x{height} exceeds {}x{}", image.width, image.height));
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
        .ok_or_else(|| format!("texture {id} not found (the mod's .{}, or the Minecraft client jar)", yog_ids::Format::Texture.ext()))
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
        for file in crate::design::files::of(&root.join("content"), *format) {
            let Some(doc) = std::fs::read_to_string(&file).ok().and_then(|t| crate::design::data::read_object(&t)).map(Value::Object) else {
                continue;
            };
            let fid = doc.get(yog_ids::content_field::ID).and_then(Value::as_str).map(str::to_string)
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
    let Some(model) = doc.get(field) else { return serde_json::Map::new() };
    let read = yog_data::read_value::<yog_formats::content::Model>(&yog_data::json::from_json(model));
    let (Ok(mut model), Some(ns)) = (read, crate::design::project::mod_id(root)) else {
        // Not a model the build would take: its textures as written.
        return model.get(yog_ids::model::TEXTURES).and_then(Value::as_object).cloned().unwrap_or_default();
    };
    if let Some(material) = model.material.clone()
        && let Ok(m) = crate::content::material::load(root, &ns, &material)
    {
        crate::content::material::apply(&mut model, &m);
    }
    model.textures.0.iter().map(|(slot, t)| (slot.clone(), yog_data::json::to_json(&yog_data::Schema::write(t)))).collect()
}

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
    Some(super::blocks3d::FACE_TEXTURES.map(pick))
}

/// A block's model: its elements, and its textures by variable (png bytes).
pub type BlockModel = (Vec<Value>, std::collections::HashMap<String, Vec<u8>>);

/// The model of the mod's block `id` as its `.yblock` gives it: its
/// `elements` and its textures by variable (png bytes). `None`: not a
/// block of the mod, or its model has no elements of its own.
pub fn content_block_model(root: &Path, id: &str) -> Option<BlockModel> {
    let (file, doc) = content_file(root, id, &[Format::Block])?;
    let elements = match super::data::at(&doc, &[yog_ids::content_field::MODEL, yog_ids::model::ELEMENTS]).and_then(Value::as_array) {
        Some(elements) => elements.clone(),
        None if doc.get(yog_ids::content_field::KIND).and_then(Value::as_str) == Some("stairs") => vec![
            stair_element([0, 0, 0], [16, 8, 16]),
            stair_element([0, 8, 8], [16, 16, 16]),
        ],
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
    for file in crate::design::files::of(&root.join("content/items"), Format::Item) {
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
    let effects = super::data::at(item, &[yog_ids::content_field::ITEM, yog_ids::content_field::POTION, yog_ids::content_field::EFFECTS])?.as_array()?;
    let mut channels = [0u64; 3];
    let mut count = 0u64;
    for effect in effects {
        let Some(id) = effect.get(yog_ids::content_field::EFFECT).and_then(Value::as_str) else {
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
    for file in crate::design::files::of(&root.join("content/effects"), Format::Def) {
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
    let files = [crate::design::files::of(&root.join("content"), Format::Item), crate::design::files::of(&root.join("content"), Format::Block)].concat();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let Some(doc) = crate::design::data::read_object(&text).map(Value::Object) else { continue };
        let fid = doc.get(yog_ids::content_field::ID).and_then(Value::as_str).map(str::to_string)
            .or_else(|| file.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
        let fid = fid.split_once(':').map_or(fid.as_str(), |(_, p)| p).to_string();
        if fid != want {
            continue;
        }
        let textures = model_textures(root, &doc, yog_ids::content_field::MODEL);
        let tex = textures
            .get(yog_ids::model::LAYER0)
            .or_else(|| ["all", "side", "top", "texture"].iter().find_map(|k| textures.get(*k)))
            .or_else(|| textures.values().next())?
            .as_str()?;
        return texture_bytes(&file, tex).ok();
    }
    None
}

#[derive(Serialize)]
pub struct Texture {
    /// `data:image/png;base64,...`
    pub url: String,
    /// Its `.png.mcmeta` animation, if it has one.
    pub animation: Option<Animation>,
    /// The texture is the mod's own file — the editor can paint it in place
    /// (otherwise painting copies it into the mod first).
    pub writable: bool,
}

/// A texture's animation (`.png.mcmeta`), as the game plays it: frames
/// `frame_width`×`frame_height` laid row by row over the image, shown in
/// `frames` order, each for its `time` in ticks.
#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    pub frame_width: u32,
    pub frame_height: u32,
    pub frames: Vec<AnimationFrame>,
    /// Blends each frame into the next (the game's `interpolate`).
    pub interpolate: bool,
}

#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct AnimationFrame {
    pub index: u32,
    /// Ticks (1/20 s).
    pub time: u32,
}

impl Animation {
    /// The animation of a `size` image from its `.mcmeta` text; `None` when it has none.
    pub fn read(meta: &str, size: (u32, u32)) -> Option<Animation> {
        let doc: Value = serde_json::from_str(meta).ok()?;
        Self::of(&yog_formats::texture::Animation::from_mcmeta(&doc)?, size)
    }

    /// `a` of a `size` image, every frame resolved: its size (the game's
    /// default a square of the image's smaller side) and each frame's time.
    pub fn of(a: &yog_formats::texture::Animation, (w, h): (u32, u32)) -> Option<Animation> {
        let side = w.min(h);
        let fw = a.width.unwrap_or(side).max(1);
        let fh = a.height.unwrap_or(side).max(1);
        let count = (w / fw) * (h / fh);
        if count == 0 {
            return None;
        }
        let frame_time = a.frame_time().max(1);
        let frames = if a.frames.is_empty() {
            (0..count).map(|index| AnimationFrame { index, time: frame_time }).collect()
        } else {
            a.frames
                .iter()
                .filter(|f| f.index < count)
                .map(|f| AnimationFrame { index: f.index, time: f.time.map_or(frame_time, |t| t.max(1)) })
                .collect()
        };
        Some(Animation { frame_width: fw, frame_height: fh, frames, interpolate: a.interpolate })
    }

    /// It as a `.ytexture` keeps it, for a `size` image: every frame listed with its time.
    pub fn to_format(&self, (w, h): (u32, u32)) -> yog_formats::texture::Animation {
        let side = w.min(h);
        yog_formats::texture::Animation {
            frame_time: None,
            interpolate: self.interpolate,
            frames: self.frames.iter().map(|f| yog_formats::texture::Frame { index: f.index, time: Some(f.time) }).collect(),
            width: (self.frame_width != side).then_some(self.frame_width),
            height: (self.frame_height != side).then_some(self.frame_height),
        }
    }
}

/// Sets the animation of texture `texture` of the mod (`None`: still) in
/// its `.ytexture`; the file written.
pub fn write_animation(file: &Path, texture: &str, animation: Option<&Animation>) -> Result<PathBuf, String> {
    let root = mod_root(file).ok_or("not in a mod")?;
    let id = texture_id(texture);
    let animation = match animation {
        Some(a) => {
            let png = texture_bytes(file, texture)?;
            let size = image_size(&png).ok_or_else(|| format!("{id} isn't a picture"))?;
            Some(a.to_format(size))
        }
        None => None,
    };
    crate::media::save_texture_animation(&root, &id, animation)
}

/// A body model's texture (`.ymodel`): `@texture:<ns>:<path>`, as a content file's.
pub fn body_texture(file: &Path, texture: &str) -> Result<Texture, String> {
    self::texture(file, texture)
}

pub fn texture(file: &Path, texture: &str) -> Result<Texture, String> {
    let bytes = texture_bytes(file, texture)?;
    let animation = image_size(&bytes).and_then(|size| {
        let meta = texture_file(file, texture, "png.mcmeta").ok()?;
        Animation::read(&String::from_utf8_lossy(&meta), size)
    });
    Ok(Texture {
        url: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        ),
        animation,
        writable: own_texture(file, texture).is_some(),
    })
}

/// The mod and the id of texture `texture` when it's the mod's own (it
/// can be painted in place, or added); `None` for the game's, another
/// mod's, a `#slot`.
pub fn own_texture(file: &Path, texture: &str) -> Option<(PathBuf, String)> {
    if texture.starts_with(yog_formats::content::SLOT_MARK) {
        return None;
    }
    let root = mod_root(file)?;
    let id = texture_id(texture);
    crate::media::is_own_texture(&root, &id).then_some((root, id))
}

/// Writes `png` as the picture of the mod's texture `texture`; the `.yres` written.
pub fn write_texture(file: &Path, texture: &str, png: Vec<u8>) -> Result<PathBuf, String> {
    if image_size(&png).is_none() {
        return Err("not a PNG image".into());
    }
    let (root, id) = own_texture(file, texture).ok_or_else(|| format!("{texture} isn't a texture of this mod"))?;
    crate::media::save_texture_picture(&root, &id, png)
}

/// Width and height from a PNG header.
fn image_size(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || &png[1..4] != b"PNG" {
        return None;
    }
    let w = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(png[20..24].try_into().ok()?);
    Some((w, h))
}

#[derive(Serialize)]
pub struct TextureChoice {
    /// What goes in the content file.
    pub value: String,
    /// "Resources", "Resource sets", "Mod assets", "Minecraft blocks", "Minecraft items".
    pub group: &'static str,
}

/// The `@<Set>.<name>` texts of the mod's `.yres` files — the name/tooltip picker's choices.
pub fn strings(file: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Some(root) = mod_root(file) else {
        return out;
    };
    for p in crate::design::files::of(&root.join("resources"), Format::Res) {
        let Some(set) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if set.contains('.') {
            continue; // a culture variant
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(doc) = crate::design::data::read_object(&text) else {
            continue;
        };
        for (name, e) in &doc {
            let ty = e.get("type").and_then(Value::as_str);
            if e.is_string() || ty.is_none_or(|t| t == "string") {
                out.push(format!("@{set}.{name}"));
            }
        }
    }
    out.sort();
    out
}

/// What a `.ydef` reference offers: the game's built-ins by short name,
/// then the mod's own definitions of `kind` as `@<kind>:` refs.
fn definitions(file: &Path, kind: &str, built_ins: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = built_ins.into_iter().map(|id| id.trim_start_matches(&format!("{}{}", yog_ids::ns::MINECRAFT, yog_ids::sep::NAMESPACE)).to_string()).collect();
    let Some(root) = mod_root(file) else {
        return out;
    };
    let ns = crate::design::project::mod_id(&root).unwrap_or_else(|| yog_ids::ns::MINECRAFT.into());
    let mut own = Vec::new();
    for p in crate::design::files::of(&root.join(yog_ids::project::CONTENT), Format::Def) {
        let Some(doc) = std::fs::read_to_string(&p).ok().and_then(|t| crate::design::data::read_object(&t)) else {
            continue;
        };
        if doc.get(yog_ids::content_field::KIND).and_then(Value::as_str) != Some(kind) {
            continue;
        }
        if let Some(id) = doc.get(yog_ids::content_field::ID).and_then(Value::as_str) {
            own.push(format!("{}{kind}{}{}", yog_ids::sep::TYPED, yog_ids::sep::NAMESPACE, yog_ids::qualify_in(&ns, id)));
        }
    }
    own.sort();
    out.extend(own);
    out
}

/// What a block's `sound` offers: the game's sound groups, then the mod's.
pub fn sound_groups(file: &Path) -> Vec<String> {
    definitions(file, "sound_group", yog_formats::def::SoundGroup::built_ins().iter().map(|g| g.id.clone()))
}

/// What a tool's `material` offers: the game's, then the mod's.
pub fn tool_materials(file: &Path) -> Vec<String> {
    definitions(file, "tool_material", yog_formats::def::ToolMaterial::built_ins().iter().map(|m| m.id.clone()))
}

/// What an armor piece's `material` offers: the game's, then the mod's.
pub fn armor_materials(file: &Path) -> Vec<String> {
    definitions(file, "armor_material", yog_formats::def::ArmorMaterial::built_ins().iter().map(|m| m.id.clone()))
}

/// What a `tags` list of content in `registry` (`item`, `block`) offers:
/// the mod's own tags by bare name (as the build last listed them), then
/// Yog's common tags.
pub fn tags(file: &Path, registry: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(root) = mod_root(file) {
        let ns = crate::design::project::mod_id(&root).unwrap_or_default();
        let listed = crate::content::read_tags(&root).remove(registry).unwrap_or_default();
        out.extend(listed.into_keys().filter_map(|t| t.strip_prefix(&format!("{ns}:")).map(str::to_string)));
    }
    out.extend(yog_formats::tags::yog_tags(registry).into_iter().map(|(p, _)| format!("yog:{p}")));
    out
}

/// Everything the texture picker offers.
pub fn textures(file: &Path) -> Vec<TextureChoice> {
    let mut out = Vec::new();
    let root = mod_root(file);
    if let Some(root) = &root {
        out.extend(crate::media::textures(root).into_iter().map(|t| TextureChoice {
            value: format!("{}{}", yog_ids::reference::TEXTURE, t.id),
            group: "Mod textures",
        }));
    }
    if let Some(v) = Resources::for_mod(root.as_deref()).vanilla {
        let (items, blocks) = v.texture_names();
        out.extend(blocks.into_iter().map(|b| TextureChoice {
            value: format!("{}{}", yog_ids::reference::TEXTURE, yog_ids::join(yog_ids::ns::MINECRAFT, &format!("{}{b}", yog_ids::res::BLOCK_DIR))),
            group: "Minecraft blocks",
        }));
        out.extend(items.into_iter().map(|i| TextureChoice {
            value: format!("{}{}", yog_ids::reference::TEXTURE, yog_ids::join(yog_ids::ns::MINECRAFT, &format!("{}{i}", yog_ids::res::ITEM_DIR))),
            group: "Minecraft items",
        }));
    }
    out
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
                if crate::content::TEMPLATES.iter().any(|t| t.sprite && t.parent == full) {
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
        assert_eq!(a.frames, vec![AnimationFrame { index: 0, time: 3 }, AnimationFrame { index: 2, time: 10 }]);
        assert!(!a.interpolate);
        let all = Animation::read(r#"{"animation":{}}"#, (16, 48)).unwrap();
        assert_eq!(all.frames.iter().map(|f| f.index).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert!(Animation::read("{}", (16, 16)).is_none());
    }

    #[test]
    fn animation_round_trips() {
        let a = Animation { frame_width: 16, frame_height: 16, frames: vec![AnimationFrame { index: 1, time: 4 }, AnimationFrame { index: 0, time: 1 }], interpolate: true };
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
        assert_eq!(image_size(&png), Some((16, 64)));
    }

    #[test]
    fn sound_groups_are_the_games_then_the_mods() {
        let dir = std::env::temp_dir().join(format!("idle-sounds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        std::fs::write(dir.join("content/crystal.ydef"), r#"{ kind: "sound_group", id: "crystal" }"#).unwrap();
        std::fs::write(dir.join("content/mythril.ydef"), r#"{ kind: "level", id: "mythril" }"#).unwrap();
        let all = sound_groups(&dir.join("content/ore.yblock"));
        assert!(all.contains(&"stone".to_string()) && all.contains(&"amethyst".to_string()));
        assert_eq!(all.last().map(String::as_str), Some("@sound_group:mymod:crystal"));
        assert!(!all.iter().any(|s| s.contains("mythril")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolves_yres_images_and_resource_paths() {
        let dir = std::env::temp_dir().join(format!("idle-content-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        let png = yog_media::image::encode_png(&yog_media::image::Image { width: 1, height: 1, rgba: vec![1, 2, 3, 255] }).unwrap();
        let file = dir.join("content/ruby.yblock");
        // Painting a new texture of the mod makes its picture and .ytexture.
        assert_eq!(own_texture(&file, "@texture:mymod:block/ruby").map(|(_, id)| id).as_deref(), Some("mymod:block/ruby"));
        write_texture(&file, "@texture:mymod:block/ruby", png.clone()).unwrap();
        assert_eq!(texture_bytes(&file, "@texture:mymod:block/ruby").unwrap(), png);
        assert!(textures(&file).iter().any(|c| c.value == "@texture:mymod:block/ruby"));
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
        assert_eq!(item_potion_color(&dir.join("content/items/potion.yitem")), Some(0x123456));
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
