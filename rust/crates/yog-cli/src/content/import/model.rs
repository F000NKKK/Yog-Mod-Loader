//! The game's model JSON as a content file's model: parents flattened (a
//! template kept), textures bundled, elements and their faces, a
//! blockstate's rotations.

use serde_json::{Map, Value};
use yog_ids::model as mc;
use yog_ids::ns::MINECRAFT;

use super::build::{Data, Fields, number, nums, obj, string};
use super::fields as f;
use super::pack::{Models, Pack};
use super::target::Target;
use crate::content::valid_path;
use crate::content::vocab::{DIRECTIONS, TEMPLATES, Template, template};

/// How many parents a model may go through before it's a mistake.
const MAX_PARENTS: usize = 16;
/// Element angles: in steps of this, up to this either way.
pub(super) const ANGLE_STEP: f64 = 22.5;
pub(super) const MAX_ANGLE: f64 = 45.0;
/// Element coordinates: in this range (a block's 0..16 and a block around it).
const COORD_MIN: f64 = -16.0;
const COORD_MAX: f64 = 32.0;
/// How finely coordinates are kept: 4 decimals.
const PRECISION: f64 = 10000.0;
/// A difference of angles that's none.
const EPSILON: f64 = 1e-6;
/// The game's built-in models (`builtin/generated`), and the sprite one.
const BUILTIN: &str = yog_ids::res::BUILTIN;
const GENERATED: &str = "generated";
/// What a block's model is when nothing gives one.
const CUBE_ALL: &str = "cube_all";
/// What a texture value refers to another slot with: `#0`.
const SLOT_MARK: &str = yog_ids::sep::TAG;
/// A face without a texture's.
const MISSING: &str = "#missing";

/// The template a vanilla model id is.
pub(super) fn template_of(id: &str) -> Option<&'static Template> {
    let id = yog_ids::qualify(id);
    TEMPLATES.iter().find(|t| t.parent == id)
}

/// `v` rounded to [`PRECISION`].
pub(super) fn rounded(v: f64) -> f64 {
    (v * PRECISION).round() / PRECISION
}

/// A model and its parents read together: the first of each that gives it.
#[derive(Default)]
struct Chain {
    textures: Vec<(String, String)>,
    elements: Option<Vec<Value>>,
    display: Map<String, Value>,
    ao: Option<bool>,
    gui_light: Option<String>,
    render_type: Option<String>,
    /// Where the chain stopped: a template, a parent kept as it is.
    parent: Option<String>,
    /// The namespace of the model itself.
    from_ns: String,
}

impl Chain {
    /// What model `m` gives that its children didn't.
    fn take(&mut self, m: &Map<String, Value>) {
        if let Some(Value::Object(tx)) = m.get(mc::TEXTURES) {
            for (k, v) in tx {
                if !self.textures.iter().any(|(s, _)| s == k)
                    && let Some(s) = v.as_str()
                {
                    self.textures.push((k.clone(), s.to_string()));
                }
            }
        }
        if self.elements.is_none()
            && let Some(Value::Array(e)) = m.get(mc::ELEMENTS)
        {
            self.elements = Some(e.clone());
        }
        if let Some(Value::Object(d)) = m.get(mc::DISPLAY) {
            for (k, v) in d {
                self.display.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        self.ao = self
            .ao
            .or_else(|| m.get(mc::AMBIENT_OCCLUSION).and_then(Value::as_bool));
        if self.gui_light.is_none() {
            self.gui_light = m
                .get(mc::GUI_LIGHT)
                .and_then(Value::as_str)
                .map(String::from);
        }
        if self.render_type.is_none() {
            self.render_type = m
                .get(mc::RENDER_TYPE)
                .and_then(Value::as_str)
                .map(|s| yog_ids::split(s).1.to_string());
        }
    }
}

/// Model `id` and its parents read up to a template, a model the import
/// can't find (kept as the parent), or one with no parent.
fn read_chain(t: &mut Target, models: &mut Models, id: &str, item: bool) -> Result<Chain, String> {
    let mut chain = Chain::default();
    let mut current = yog_ids::qualify(id);
    chain.from_ns = yog_ids::split(&current).0.to_string();
    for depth in 0..=MAX_PARENTS {
        let Some((m, _)) = models.model(&current) else {
            if depth == 0 {
                return Err(format!("{id}: no such model in the pack"));
            }
            t.warnings.push(format!(
                "{id}: parent {current} not found (no Minecraft client jar?) — kept as the parent"
            ));
            chain.parent = Some(current);
            return Ok(chain);
        };
        chain.take(&m);
        let Some(p) = m.get(mc::PARENT).and_then(Value::as_str) else {
            return Ok(chain);
        };
        if yog_ids::split(p).1.starts_with(BUILTIN) {
            if p.ends_with(GENERATED) {
                chain.parent = Some(GENERATED.into());
                return Ok(chain);
            }
            return Err(format!("{id}: built-in model {p} can't be imported"));
        }
        if chain.elements.is_none()
            && let Some(tpl) = template_of(p)
            && (!tpl.sprite || item)
        {
            chain.parent = Some(tpl.name.to_string());
            return Ok(chain);
        }
        current = yog_ids::qualify(p);
    }
    Err(format!("{id}: parent chain too long"))
}

/// A model of the pack → a content model object (and `render_type`).
pub(super) fn convert_model(
    t: &mut Target,
    models: &mut Models,
    id: &str,
    item: bool,
) -> Result<(Data, Option<String>), String> {
    let mut chain = read_chain(t, models, id, item)?;
    // A sprite parent reached through the jar: `item/generated` itself.
    if chain.elements.is_none() && chain.parent.is_none() {
        chain.parent = Some(if item { GENERATED } else { CUBE_ALL }.into());
    }
    let mut out = obj::<&str>([]);
    if let Some(p) = &chain.parent {
        out.set(f::PARENT, string(p.clone()));
    }
    let tex = textures(t, models.pack, id, &chain)?;
    if tex.has_fields() {
        out.set(f::TEXTURES, tex);
    }
    if let Some(els) = &chain.elements {
        let converted: Vec<Data> = els
            .iter()
            .enumerate()
            .map(|(i, e)| element_from_mc(t, id, i, e))
            .collect();
        out.set(f::ELEMENTS, Data::List(converted));
    }
    if !chain.display.is_empty() {
        out.set(
            f::DISPLAY,
            yog_data::json::from_json(&Value::Object(std::mem::take(&mut chain.display))),
        );
    }
    if let Some(ao) = chain.ao {
        out.set(f::AMBIENT_OCCLUSION, Data::Bool(ao));
    }
    if let Some(g) = chain.gui_light {
        out.set(f::GUI_LIGHT, string(g));
    }
    Ok((out, chain.render_type))
}

/// The chain's textures: references to other slots stay, ids get copied
/// when the pack has them; a template's own `particle` left to it.
fn textures(t: &mut Target, pack: &Pack, id: &str, chain: &Chain) -> Result<Data, String> {
    let mut tex = obj::<&str>([]);
    let tex_ns = t.texture_ns(&chain.from_ns);
    for (slot, v) in &chain.textures {
        let value = if v.starts_with(SLOT_MARK) {
            v.clone()
        } else {
            texture(t, pack, id, v, &tex_ns)?
        };
        tex.set(slot, string(value));
    }
    if let Some(tpl) = chain.parent.as_deref().and_then(template)
        && tpl.has_particle
    {
        tex.without(mc::PARTICLE);
    }
    Ok(tex)
}

/// Texture `v` of model `id`: bundled into namespace `tex_ns` when the
/// pack has its picture, else referred to as it is.
fn texture(t: &mut Target, pack: &Pack, id: &str, v: &str, tex_ns: &str) -> Result<String, String> {
    let (ns, path) = yog_ids::split(v);
    let rel = yog_ids::asset_file(ns, &yog_ids::texture_file(path));
    match pack.read(&rel) {
        Some(png) if valid_path(path) => {
            let meta = pack.read(&format!("{rel}{}", yog_ids::res::MCMETA));
            if ns != tex_ns {
                t.warnings.push(format!(
                    "{id}: texture {ns}:{path} is bundled as {tex_ns}:{path}"
                ));
            }
            t.put_texture(tex_ns, path, &png, meta.as_deref())
        }
        _ => Ok(format!(
            "{}{}",
            yog_ids::reference::TEXTURE,
            yog_ids::qualify(v)
        )),
    }
}

/// The nearest allowed element angle (-45..45 by 22.5).
pub(super) fn snap_angle(t: &mut Target, what: &str, a: f64) -> f64 {
    let snapped = ((a / ANGLE_STEP).round() * ANGLE_STEP).clamp(-MAX_ANGLE, MAX_ANGLE);
    if (snapped - a).abs() > EPSILON {
        t.warnings.push(format!(
            "{what}: rotation {a}° → {snapped}° (Minecraft allows -{MAX_ANGLE}..{MAX_ANGLE} by {ANGLE_STEP})"
        ));
    }
    snapped
}

/// A coordinate in the range elements may take.
pub(super) fn clamp_coord(t: &mut Target, what: &str, v: f64) -> f64 {
    let c = v.clamp(COORD_MIN, COORD_MAX);
    if c != v {
        t.warnings
            .push(format!("{what}: {v} is outside -16..32, clamped"));
    }
    rounded(c)
}

/// The numbers of `v`, an array.
pub(super) fn numbers(v: Option<&Value>) -> Vec<f64> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

/// A Minecraft model element → a content element (same shape, `tint`).
fn element_from_mc(t: &mut Target, id: &str, i: usize, e: &Value) -> Data {
    let what = format!("{id} element {i}");
    let coords = |t: &mut Target, v: Option<&Value>| -> Vec<f64> {
        numbers(v)
            .into_iter()
            .map(|n| clamp_coord(t, &what, n))
            .collect()
    };
    let mut out = obj::<&str>([]);
    if let Some(n) = e.get(yog_ids::word::NAME).and_then(Value::as_str) {
        out.set(f::EL_NAME, string(n));
    }
    out.set(f::FROM, nums(&coords(t, e.get(mc::FROM))));
    out.set(f::TO, nums(&coords(t, e.get(mc::TO))));
    if let Some(r) = e.get(mc::ROTATION) {
        let angle = snap_angle(
            t,
            &what,
            r.get(mc::ANGLE).and_then(Value::as_f64).unwrap_or(0.0),
        );
        if angle != 0.0 {
            let mut rot = obj([(
                f::AXIS,
                string(r.get(mc::AXIS).and_then(Value::as_str).unwrap_or(mc::Y)),
            )]);
            rot.set(f::ANGLE, number(angle));
            rot.set(f::ORIGIN, nums(&coords(t, r.get(mc::ORIGIN))));
            if r.get(mc::RESCALE).and_then(Value::as_bool) == Some(true) {
                rot.set(f::RESCALE, Data::Bool(true));
            }
            out.set(f::ROTATION, rot);
        }
    }
    if e.get(mc::SHADE).and_then(Value::as_bool) == Some(false) {
        out.set(f::SHADE, Data::Bool(false));
    }
    let mut faces = obj::<&str>([]);
    if let Some(Value::Object(fs)) = e.get(mc::FACES) {
        for &dir in DIRECTIONS {
            if let Some(face) = fs.get(dir) {
                faces.set(dir, face_from_mc(face));
            }
        }
    }
    out.set(f::FACES, faces);
    out
}

/// A face: its texture slot, uv, rotation, the side it's culled by
/// (`bottom` is `down`), its tint.
fn face_from_mc(face: &Value) -> Data {
    let mut fo = obj([(
        f::TEXTURE,
        string(
            face.get(mc::TEXTURE)
                .and_then(Value::as_str)
                .unwrap_or(MISSING),
        ),
    )]);
    if let Some(Value::Array(_)) = face.get(mc::UV) {
        let pixels = f64::from(yog_ids::units::BLOCK_PIXELS);
        let uv: Vec<f64> = numbers(face.get(mc::UV))
            .into_iter()
            .map(|n| rounded(n.clamp(0.0, pixels)))
            .collect();
        fo.set(f::UV, nums(&uv));
    }
    if let Some(r) = face
        .get(mc::ROTATION)
        .and_then(Value::as_f64)
        .filter(|r| *r != 0.0)
    {
        fo.set(f::FACE_ROTATION, number(r));
    }
    if let Some(c) = face.get(mc::CULLFACE).and_then(Value::as_str) {
        fo.set(
            f::CULLFACE,
            string(if c == yog_ids::word::BOTTOM {
                yog_ids::direction::DOWN
            } else {
                c
            }),
        );
    }
    if let Some(n) = face.get(mc::TINTINDEX).and_then(Value::as_f64) {
        fo.set(f::TINT, number(n));
    }
    fo
}

/// One variant's model and rotation.
fn variant_rotation(v: &Value) -> (Option<String>, Data) {
    let mut r = obj::<&str>([]);
    for (k, out) in [(mc::X, f::X), (mc::Y, f::Y)] {
        if let Some(n) = v.get(k).and_then(Value::as_f64).filter(|n| *n != 0.0) {
            r.set(out, number(n));
        }
    }
    if let Some(w) = v.get(mc::WEIGHT).and_then(Value::as_f64) {
        r.set(f::WEIGHT, number(w));
    }
    if v.get(mc::UVLOCK).and_then(Value::as_bool) == Some(true) {
        r.set(f::UVLOCK, Data::Bool(true));
    }
    (
        v.get(mc::MODEL).and_then(Value::as_str).map(String::from),
        r,
    )
}

/// The blockstate's first variant (or first part) — the others said.
fn first_variant(t: &mut Target, id: &str, bs: &Value) -> Option<Value> {
    if let Some(Value::Object(vs)) = bs.get(mc::VARIANTS) {
        if vs.len() > 1 {
            t.warnings.push(format!("{id}: the blockstate has {} variants (block state properties) — only the first is imported; content files don't do properties yet", vs.len()));
        }
        return vs.values().next().cloned();
    }
    let Some(Value::Array(parts)) = bs.get(mc::MULTIPART) else {
        return None;
    };
    t.warnings.push(format!(
        "{id}: multipart blockstate — only the first part is imported"
    ));
    parts.first().and_then(|p| p.get(mc::APPLY)).cloned()
}

/// The blockstate's rotations (`"variants": {"": [...]}`) — or none.
pub(super) fn state_rotations(
    t: &mut Target,
    id: &str,
    bs: &Value,
) -> (Option<String>, Option<Data>) {
    match first_variant(t, id, bs) {
        Some(Value::Array(list)) => {
            let mut model = None;
            let mut rots = Vec::new();
            for v in &list {
                let (m, r) = variant_rotation(v);
                model = model.or(m);
                rots.push(r);
            }
            (model, Some(Data::List(rots)))
        }
        Some(v) => {
            let (m, r) = variant_rotation(&v);
            (m, (r.has_fields()).then(|| Data::List(vec![r])))
        }
        None => (None, None),
    }
}

/// The namespace an id without one is in: the mod's, else the game's.
pub(super) fn default_ns(t: &Target) -> String {
    t.ns.clone().unwrap_or_else(|| MINECRAFT.into())
}
