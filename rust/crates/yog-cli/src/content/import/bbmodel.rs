//! A Blockbench project (`.bbmodel`, the Java block/item format) as a
//! `.yblock` or `.yitem`: its embedded textures bundled, its cubes as
//! elements, its display transforms.

use base64::Engine as _;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use yog_ids::kind::{BLOCK, ITEM};
use yog_ids::model as mc;
use yog_ids::ns::MINECRAFT;

use super::blocks::content_file;
use super::build::{Data, Fields, number, nums, obj, string};
use super::fields as f;
use super::model::{clamp_coord, numbers, rounded, snap_angle, template_of};
use super::target::Target;
use crate::content::valid_path;
use yog_formats::content::vocab::{DIRECTIONS, DISPLAY_SLOTS};

/// The Blockbench formats that map to Minecraft models.
const JAVA_BLOCK: &str = "java_block";
const FREE: &str = "free";
/// What an embedded texture's source starts with.
const PNG_DATA: &str = "data:image/png;base64,";
/// A cube's type, the elements' only one Minecraft has.
const CUBE: &str = "cube";
/// A texture's name when it gives none.
const TEXTURE: &str = "texture";
/// The axes in Blockbench's rotation order.
const AXES: [&str; 3] = [mc::X, mc::Y, "z"];
/// Display transforms' parts, and the values that are no change.
const TRANSFORMS: [&str; 3] = [mc::ROTATION, mc::TRANSLATION, mc::SCALE];
const NO_SCALE: [f64; 3] = [1.0; 3];
const NO_MOVE: [f64; 3] = [0.0; 3];
/// The gui light a `front_gui_light` model takes.
const FRONT: &str = "front";

/// Pixels a block is: a texture's uv resolution maps onto these.
fn block_pixels() -> f64 {
    f64::from(yog_ids::units::BLOCK_PIXELS)
}

/// Standard base64 (data URLs of Blockbench textures), line breaks and
/// spaces ignored; the URL-safe alphabet too.
pub(super) fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let engines = [
        base64::engine::general_purpose::STANDARD,
        base64::engine::general_purpose::URL_SAFE,
    ];
    let padded = engines.iter().find_map(|e| e.decode(&clean).ok());
    padded.or_else(|| {
        [
            base64::engine::general_purpose::STANDARD_NO_PAD,
            base64::engine::general_purpose::URL_SAFE_NO_PAD,
        ]
        .iter()
        .find_map(|e| e.decode(&clean).ok())
    })
}

/// `My Texture (1).png` → `my_texture_1`.
fn sanitize(name: &str) -> String {
    let stem = name.strip_suffix(yog_ids::res::PNG).unwrap_or(name);
    let mut s = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('_') {
            s.push('_');
        }
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { TEXTURE.into() } else { s }
}

/// What a project is imported as: its namespace and name, whether the
/// id was given with its namespace, a block's or an item's.
struct Named {
    ns: String,
    name: String,
    own_ns: bool,
    item: bool,
}

/// The project's textures: slot → value, each slot's uv scale, by uuid
/// its slot.
struct Textures {
    tex: Data,
    uv_scale: Vec<(f64, f64)>,
    by_uuid: BTreeMap<String, usize>,
}

/// Imports the `.bbmodel` at `file` as `id` (its name by default), an item's when `item`.
pub(super) fn import_bbmodel(
    t: &mut Target,
    file: &Path,
    id: Option<&str>,
    item: bool,
) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let bb: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    check_format(t, &bb, file)?;
    let named = named(t, &bb, file, id, item)?;
    let textures = textures(t, &bb, &named)?;
    let elements = elements(t, &bb, &textures);
    let model = model(&bb, textures.tex, elements)?;
    let mut j = obj::<&str>([]);
    if t.ns.is_none() || named.own_ns {
        j.set(f::ID, string(yog_ids::join(&named.ns, &named.name)));
    }
    j.set(f::MODEL, model);
    t.write_file(
        &content_file(item, &named.name),
        &yog_data::binary::encode(&j),
    )
}

/// Only the "Java Block/Item" format maps to Minecraft models; a generic
/// one's rotations get snapped.
fn check_format(t: &mut Target, bb: &Value, file: &Path) -> Result<(), String> {
    let format = bb
        .pointer("/meta/model_format")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !matches!(format, JAVA_BLOCK | FREE | "") {
        return Err(format!(
            "{}: a {format:?} Blockbench model — only the \"Java Block/Item\" format maps to Minecraft models (File > Convert Project)",
            file.display()
        ));
    }
    if format == FREE {
        t.warnings
            .push("a generic Blockbench model: rotations get snapped to Minecraft's".into());
    }
    Ok(())
}

/// The project's id: as given, else its name, else its file's.
fn named(
    t: &Target,
    bb: &Value,
    file: &Path,
    id: Option<&str>,
    item: bool,
) -> Result<Named, String> {
    let name = match id {
        Some(i) => i.to_string(),
        None => {
            let n = bb
                .get(yog_ids::word::NAME)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from);
            sanitize(&n.unwrap_or_else(|| {
                file.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            }))
        }
    };
    let (id_ns, name) = match name.split_once(yog_ids::sep::NAMESPACE) {
        Some((n, p)) => (Some(n.to_string()), p.to_string()),
        None => (None, name),
    };
    if !valid_path(&name) {
        return Err(format!("`{name}` isn't a valid id — pass --id <name>"));
    }
    let own_ns = id_ns.is_some();
    let ns = id_ns
        .or_else(|| t.ns.clone())
        .ok_or("the mod id isn't a valid namespace — pass --id <namespace>:<name>")?;
    Ok(Named {
        ns,
        name,
        own_ns,
        item,
    })
}

/// The project's textures, a slot each by index (`"0"`, `"1"`, … as
/// Blockbench exports them): embedded ones bundled, others referred to.
fn textures(t: &mut Target, bb: &Value, named: &Named) -> Result<Textures, String> {
    let res = |k: &str| {
        bb.pointer(&format!("/resolution/{k}"))
            .and_then(Value::as_f64)
            .unwrap_or(block_pixels())
    };
    let (res_w, res_h) = (res(yog_ids::word::WIDTH), res(yog_ids::word::HEIGHT));
    let folder = if named.item { ITEM } else { BLOCK };
    let list = bb
        .get(mc::TEXTURES)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut tex = obj::<&str>([]);
    let mut uv_scale = Vec::new();
    let mut particle = None;
    for (i, tx) in list.iter().enumerate() {
        let uw = tx.get("uv_width").and_then(Value::as_f64).unwrap_or(res_w);
        let uh = tx.get("uv_height").and_then(Value::as_f64).unwrap_or(res_h);
        uv_scale.push((block_pixels() / uw, block_pixels() / uh));
        let value = texture(t, tx, &named.ns, folder)?;
        if tx.get(mc::PARTICLE).and_then(Value::as_bool) == Some(true) {
            particle = Some(value.clone());
        }
        tex.set(&i.to_string(), string(value));
    }
    if let Some(p) = particle {
        tex.set(mc::PARTICLE, string(p));
    }
    let by_uuid = list
        .iter()
        .enumerate()
        .filter_map(|(i, tx)| {
            tx.get("uuid")
                .and_then(Value::as_str)
                .map(|u| (u.to_string(), i))
        })
        .collect();
    Ok(Textures {
        tex,
        uv_scale,
        by_uuid,
    })
}

/// One texture's value: an embedded picture bundled into the mod, or a
/// reference to the texture it names.
fn texture(t: &mut Target, tx: &Value, ns: &str, folder: &str) -> Result<String, String> {
    let tex_name = sanitize(
        tx.get(yog_ids::word::NAME)
            .and_then(Value::as_str)
            .unwrap_or(TEXTURE),
    );
    let text = |k: &str| tx.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
    let source = tx
        .get(yog_ids::word::SOURCE)
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some(data) = source.strip_prefix(PNG_DATA) {
        let png = base64_decode(data).ok_or("a texture's data isn't base64")?;
        let tex_folder = text("folder").filter(|f| valid_path(f)).unwrap_or(folder);
        let tns = t.texture_ns(ns);
        return t.put_texture(&tns, &format!("{tex_folder}/{tex_name}"), &png, None);
    }
    let tns = text("namespace").unwrap_or(MINECRAFT);
    let tfolder = text("folder").unwrap_or(folder);
    t.warnings.push(format!("texture {tex_name} isn't embedded in the .bbmodel — referenced as {tns}:{tfolder}/{tex_name}"));
    Ok(format!(
        "{}{tns}:{tfolder}/{tex_name}",
        yog_ids::reference::TEXTURE
    ))
}

/// The project's cubes as elements; anything else skipped.
fn elements(t: &mut Target, bb: &Value, textures: &Textures) -> Vec<Data> {
    let list = bb
        .get(mc::ELEMENTS)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for (i, e) in list.iter().enumerate() {
        let what = format!(
            "element {i} ({})",
            e.get(yog_ids::word::NAME)
                .and_then(Value::as_str)
                .unwrap_or("?")
        );
        if e.get(yog_ids::word::TYPE)
            .and_then(Value::as_str)
            .is_some_and(|ty| ty != CUBE)
        {
            t.warnings.push(format!("{what}: not a cube — skipped"));
            continue;
        }
        if e.get("visibility").and_then(Value::as_bool) == Some(false) {
            t.warnings
                .push(format!("{what}: hidden in Blockbench — imported anyway"));
        }
        out.push(element(t, e, &what, textures));
    }
    out
}

/// One cube: its box (inflated both ways), its one rotation, its faces.
fn element(t: &mut Target, e: &Value, what: &str, textures: &Textures) -> Data {
    let vec3 = |t: &mut Target, k: &str| -> Vec<f64> {
        numbers(e.get(k))
            .into_iter()
            .map(|n| clamp_coord(t, what, n))
            .collect()
    };
    let mut el = obj::<&str>([]);
    if let Some(n) = e.get(yog_ids::word::NAME).and_then(Value::as_str) {
        el.set(f::EL_NAME, string(n));
    }
    // Blockbench's inflate grows the box both ways.
    let inflate = e.get("inflate").and_then(Value::as_f64).unwrap_or(0.0);
    let from: Vec<f64> = vec3(t, mc::FROM).into_iter().map(|v| v - inflate).collect();
    let to: Vec<f64> = vec3(t, mc::TO).into_iter().map(|v| v + inflate).collect();
    el.set(f::FROM, nums(&from));
    el.set(f::TO, nums(&to));
    if let Some(r) = rotation(t, e, what) {
        el.set(f::ROTATION, r);
    }
    if e.get(mc::SHADE).and_then(Value::as_bool) == Some(false) {
        el.set(f::SHADE, Data::Bool(false));
    }
    let mut faces = obj::<&str>([]);
    for dir in DIRECTIONS {
        if let Some(fo) = e
            .pointer(&format!("/{}/{dir}", mc::FACES))
            .and_then(|fv| face(fv, textures))
        {
            faces.set(dir, fo);
        }
    }
    el.set(f::FACES, faces);
    el
}

/// A cube's rotation: on its first rotated axis only (Minecraft rotates
/// elements on one), snapped.
fn rotation(t: &mut Target, e: &Value, what: &str) -> Option<Data> {
    let rot = numbers(e.get(mc::ROTATION));
    let nonzero: Vec<(usize, f64)> = rot
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, a)| *a != 0.0)
        .collect();
    if nonzero.len() > 1 {
        t.warnings.push(format!("{what}: rotated on more than one axis — only the first kept (Minecraft rotates elements on one axis)"));
    }
    let (axis, angle) = nonzero.first().copied()?;
    let angle = snap_angle(t, what, angle);
    if angle == 0.0 {
        return None;
    }
    let mut r = obj([(f::AXIS, string(AXES.get(axis).copied().unwrap_or(AXES[2])))]);
    r.set(f::ANGLE, number(angle));
    let origin: Vec<f64> = numbers(e.get(mc::ORIGIN))
        .into_iter()
        .map(|n| clamp_coord(t, what, n))
        .collect();
    r.set(f::ORIGIN, nums(&origin));
    if e.get(mc::RESCALE).and_then(Value::as_bool) == Some(true) {
        r.set(f::RESCALE, Data::Bool(true));
    }
    Some(r)
}

/// A face with a texture (Blockbench doesn't export one without): its
/// slot, its uv scaled to the block, rotation, cullface, tint.
fn face(fv: &Value, textures: &Textures) -> Option<Data> {
    let index = match fv.get(mc::TEXTURE) {
        Some(Value::Number(n)) => n.as_u64().and_then(|n| usize::try_from(n).ok()),
        Some(Value::String(u)) => textures.by_uuid.get(u).copied(),
        _ => None,
    };
    let (su, sv) = *textures.uv_scale.get(index?)?;
    let mut fo = obj([(
        f::TEXTURE,
        string(format!("{}{}", yog_ids::sep::TAG, index?)),
    )]);
    if let Some(Value::Array(_)) = fv.get(mc::UV) {
        let uv: Vec<f64> = numbers(fv.get(mc::UV))
            .into_iter()
            .enumerate()
            .map(|(k, n)| {
                rounded((n * if k % 2 == 0 { su } else { sv }).clamp(0.0, block_pixels()))
            })
            .collect();
        fo.set(f::UV, nums(&uv));
    }
    if let Some(r) = fv
        .get(mc::ROTATION)
        .and_then(Value::as_f64)
        .filter(|r| *r != 0.0)
    {
        fo.set(f::FACE_ROTATION, number(r));
    }
    if let Some(c) = fv
        .get(mc::CULLFACE)
        .and_then(Value::as_str)
        .filter(|c| !c.is_empty())
    {
        fo.set(f::CULLFACE, string(c));
    }
    if let Some(n) = fv
        .get(f::TINT)
        .and_then(Value::as_f64)
        .filter(|n| *n >= 0.0)
    {
        fo.set(f::TINT, number(n));
    }
    Some(fo)
}

/// The content model: a parent (a template, or a model id) when it has no
/// cubes, its textures and elements, display transforms, lighting.
fn model(bb: &Value, tex: Data, elements: Vec<Data>) -> Result<Data, String> {
    let mut model = obj::<&str>([]);
    match bb
        .get(mc::PARENT)
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty())
    {
        Some(p) if elements.is_empty() => model.set(
            f::PARENT,
            string(template_of(p).map_or_else(|| yog_ids::qualify(p), |tpl| tpl.name.to_string())),
        ),
        Some(_) => {}
        None if elements.is_empty() => return Err("the model has no cubes".into()),
        None => {}
    }
    if tex.has_fields() {
        model.set(f::TEXTURES, tex);
    }
    if !elements.is_empty() {
        model.set(f::ELEMENTS, Data::List(elements));
    }
    if let Some(Value::Object(d)) = bb.get(mc::DISPLAY) {
        let display = display(d);
        if display.has_fields() {
            model.set(f::DISPLAY, display);
        }
    }
    if bb.get(mc::AMBIENT_OCCLUSION).and_then(Value::as_bool) == Some(false) {
        model.set(f::AMBIENT_OCCLUSION, Data::Bool(false));
    }
    if bb.get("front_gui_light").and_then(Value::as_bool) == Some(true) {
        model.set(f::GUI_LIGHT, string(FRONT));
    }
    Ok(model)
}

/// The display transforms the game has, each part that changes something.
fn display(d: &serde_json::Map<String, Value>) -> Data {
    let mut display = obj::<&str>([]);
    for (slot, v) in d
        .iter()
        .filter(|(slot, _)| DISPLAY_SLOTS.contains(&slot.as_str()))
    {
        let mut o = obj::<&str>([]);
        for k in TRANSFORMS {
            let a = numbers(v.get(k));
            let unchanged = if k == mc::SCALE { NO_SCALE } else { NO_MOVE };
            if v.get(k).is_some_and(Value::is_array)
                && a.len() == unchanged.len()
                && a[..] != unchanged[..]
            {
                o.set(k, nums(&a));
            }
        }
        if o.has_fields() {
            display.set(slot, o);
        }
    }
    display
}
