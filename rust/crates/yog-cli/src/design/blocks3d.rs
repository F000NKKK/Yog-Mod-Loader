//! `draw2d_blocks` in the designer: blocks in 3D, as the game draws them in
//! a GUI (an orthographic view, turned and tilted), each block a cube with
//! its model's boxes (`elements`: a cable's core, a slab's half) with
//! their textures — the faces you'd see, back to front, the top lit, the
//! sides shaded like Minecraft's. A model without elements is a cube.
//! A block state picks its blockstate's variant (turned by its `x`/`y`)
//! or every multipart part whose `when` it meets; elements keep their own
//! rotations.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::Value;

use super::assets::{Image, Resources, decode_png, missing_image};
use super::raster::Canvas;

/// A cube's textures per face: up, down, north, south, west, east.
pub type Faces = [Option<Arc<Image>>; 6];

/// One box of a model, in sixteenths of a block: per face (up, down,
/// north, south, west, east) its texture, already cut to the face's uv
/// (`None`: the box has no such face), and the turns it's drawn with, in
/// order (its element's, then its variant's).
#[derive(Clone)]
pub struct Cuboid {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub faces: Faces,
    pub turns: Vec<Turn>,
}

/// A turn of a box about `axis` (0 x, 1 y, 2 z) through `origin`
/// (sixteenths of a block), `degrees` counter-clockwise looking down the
/// axis; `rescale` stretches the turned box's other axes back to the block
/// (an element's `rescale`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    pub origin: [f32; 3],
    pub axis: usize,
    pub degrees: f32,
    pub rescale: bool,
}

/// A block's centre, in sixteenths: what a variant turns about.
const CENTRE: [f32; 3] = [8.0, 8.0, 8.0];
/// A block's side, in the sixteenths models are written in.
const BLOCK_UNITS: f32 = 16.0;

impl Turn {
    /// `p` (sixteenths) turned.
    fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let (s, c) = self.degrees.to_radians().sin_cos();
        let (a, b) = match self.axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        let d = [0, 1, 2].map(|i| p[i] - self.origin[i]);
        let mut out = d;
        out[a] = d[a] * c - d[b] * s;
        out[b] = d[a] * s + d[b] * c;
        if self.rescale && c.abs() > f32::EPSILON {
            let k = 1.0 / c.abs();
            out[a] *= k;
            out[b] *= k;
        }
        [0, 1, 2].map(|i| out[i] + self.origin[i])
    }

    /// A direction turned (no origin, no rescale).
    fn apply_dir(&self, v: [f32; 3]) -> [f32; 3] {
        Turn { origin: [0.0; 3], rescale: false, ..*self }.apply(v)
    }
}

/// A block as drawn: its model's boxes.
pub type Shape = Vec<Cuboid>;

/// A whole-block cube with `faces` (a missing one: the missing texture).
pub fn cube(faces: Faces) -> Shape {
    let faces = faces.map(|f| Some(f.unwrap_or_else(|| Arc::new(missing_image()))));
    vec![Cuboid { from: [0.0; 3], to: [BLOCK_UNITS; 3], faces, turns: Vec::new() }]
}

const FACE_NAMES: [&str; 6] = ["up", "down", "north", "south", "west", "east"];

/// The texture variables the game's block models draw each face with (in
/// [`FACE_NAMES`] order), most particular first: a cube's `up`, a
/// column's `end`, a `cube_all`'s `all`, a one-texture model's `texture`.
pub const FACE_TEXTURES: [&[&str]; 6] = [
    &["up", "top", "end", "all", "texture"],
    &["down", "bottom", "end", "all", "texture"],
    &["north", "front", "side", "all", "texture"],
    &["south", "side", "all", "texture"],
    &["west", "side", "all", "texture"],
    &["east", "side", "all", "texture"],
];

/// What a face falls back to when its model has none of [`FACE_TEXTURES`].
const PARTICLE: &str = "particle";

/// The part of a face's texture its box shows when the model gives no `uv`.
fn default_uv(slot: usize, f: [f32; 3], t: [f32; 3]) -> [f32; 4] {
    match slot {
        0 => [f[0], f[2], t[0], t[2]],
        1 => [f[0], BLOCK_UNITS - t[2], t[0], BLOCK_UNITS - f[2]],
        2 => [BLOCK_UNITS - t[0], BLOCK_UNITS - t[1], BLOCK_UNITS - f[0], BLOCK_UNITS - f[1]],
        3 => [f[0], BLOCK_UNITS - t[1], t[0], BLOCK_UNITS - f[1]],
        4 => [f[2], BLOCK_UNITS - t[1], t[2], BLOCK_UNITS - f[1]],
        _ => [BLOCK_UNITS - t[2], BLOCK_UNITS - t[1], BLOCK_UNITS - f[2], BLOCK_UNITS - f[1]],
    }
}

/// `img`'s part `uv` (sixteenths of it; reversed: mirrored).
fn cut(img: &Image, [u0, v0, u1, v1]: [f32; 4]) -> Image {
    let (sx, sy) = (img.width as f32 / BLOCK_UNITS, img.width.min(img.height) as f32 / BLOCK_UNITS);
    let (x0, x1) = ((u0 * sx).round() as i32, (u1 * sx).round() as i32);
    let (y0, y1) = ((v0 * sy).round() as i32, (v1 * sy).round() as i32);
    let (w, h) = ((x1 - x0).unsigned_abs().max(1), (y1 - y0).unsigned_abs().max(1));
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let px = if x1 >= x0 { x0 + x } else { x0 - 1 - x };
            let py = if y1 >= y0 { y0 + y } else { y0 - 1 - y };
            let (px, py) = (px.clamp(0, img.width as i32 - 1) as u32, py.clamp(0, img.height as i32 - 1) as u32);
            rgba.extend_from_slice(&img.pixel(px, py).to_rgba8());
        }
    }
    Image { width: w, height: h, rgba }
}

/// A model's `elements` as boxes; `texture("#var" or "var")` gives a face's texture.
pub fn shape_of(elements: &[Value], texture: &mut dyn FnMut(&str) -> Option<Arc<Image>>) -> Shape {
    let triple = |v: Option<&Value>| -> Option<[f32; 3]> {
        let a = v?.as_array()?;
        Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
    };
    elements
        .iter()
        .filter_map(|e| {
            let (from, to) = (triple(e.get("from"))?, triple(e.get("to"))?);
            let (f, t) = ([0, 1, 2].map(|i| from[i].min(to[i])), [0, 1, 2].map(|i| from[i].max(to[i])));
            let faces = e.get("faces")?;
            let faces: Faces = std::array::from_fn(|slot| {
                let face = faces.get(FACE_NAMES[slot])?;
                let img = face.get("texture").and_then(Value::as_str).and_then(&mut *texture).unwrap_or_else(|| Arc::new(missing_image()));
                let uv = face
                    .get("uv")
                    .and_then(Value::as_array)
                    .and_then(|a| Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32, a.get(3)?.as_f64()? as f32]))
                    .unwrap_or_else(|| default_uv(slot, f, t));
                Some(Arc::new(cut(&img, uv)))
            });
            Some(Cuboid { from: f, to: t, faces, turns: element_turn(e).into_iter().collect() })
        })
        .collect()
}

/// An element's `rotation`: `{ origin, axis: "x"|"y"|"z", angle, rescale }`.
fn element_turn(e: &Value) -> Option<Turn> {
    let r = e.get("rotation")?;
    let o = r.get("origin")?.as_array()?;
    let origin = [o.first()?.as_f64()? as f32, o.get(1)?.as_f64()? as f32, o.get(2)?.as_f64()? as f32];
    let axis = ["x", "y", "z"].iter().position(|a| r.get("axis").and_then(Value::as_str) == Some(*a))?;
    let degrees = r.get("angle")?.as_f64()? as f32;
    Some(Turn { origin, axis, degrees, rescale: r.get("rescale").and_then(Value::as_bool).unwrap_or(false) })
}

/// The first `elements` up a model's parents.
fn model_elements(res: &Resources, model: &str, depth: usize) -> Option<Vec<Value>> {
    if depth > 8 {
        return None;
    }
    let (ns, path) = yog_ids::split(model);
    let path = path.strip_prefix("block/").map_or(format!("block/{path}"), |p| format!("block/{p}"));
    let json: Value = serde_json::from_slice(&res.read(ns, &yog_ids::model_file(&path))?).ok()?;
    if let Some(e) = json.get("elements").and_then(Value::as_array) {
        return Some(e.clone());
    }
    model_elements(res, json.get("parent")?.as_str()?, depth + 1)
}

/// The texture variables of a block model, its parents' included.
fn model_textures(res: &Resources, model: &str, depth: usize) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if depth > 8 {
        return out;
    }
    let (ns, path) = yog_ids::split(model);
    let path = path.strip_prefix("block/").map_or(format!("block/{path}"), |p| format!("block/{p}"));
    let Some(json) = res.read(ns, &yog_ids::model_file(&path)).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
        return out;
    };
    if let Some(parent) = json.get("parent").and_then(Value::as_str) {
        out = model_textures(res, parent, depth + 1);
    }
    if let Some(t) = json.get("textures").and_then(Value::as_object) {
        for (k, v) in t {
            if let Some(v) = v.as_str() {
                out.insert(k.clone(), v.to_string());
            }
        }
    }
    out
}

/// A block state's id and its properties: `minecraft:furnace[facing=north]`.
fn split_state(state: &str) -> (&str, HashMap<&str, &str>) {
    let (id, props) = state.split_once('[').unwrap_or((state, ""));
    let props = props.trim_end_matches(']').split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
    (id, props)
}

/// Whether `props` meet a multipart `when` (every property one of its
/// `a|b` values; `OR`/`AND`: any/all of a list). A property the state
/// doesn't name meets nothing.
fn meets(when: &Value, props: &HashMap<&str, &str>) -> bool {
    let Some(map) = when.as_object() else { return true };
    if let Some(any) = map.get("OR").and_then(Value::as_array) {
        return any.iter().any(|w| meets(w, props));
    }
    if let Some(all) = map.get("AND").and_then(Value::as_array) {
        return all.iter().all(|w| meets(w, props));
    }
    map.iter().all(|(k, v)| {
        let want = v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
        props.get(k.as_str()).is_some_and(|have| want.split('|').any(|w| w == *have))
    })
}

/// A blockstate's `apply`/variant: its model and its turns (`x` then `y`,
/// about the block's centre; the game turns models clockwise looking down
/// an axis).
fn applied(v: &Value) -> Option<(String, Vec<Turn>)> {
    let v = if v.is_array() { v.get(0)? } else { v };
    let model = v.get("model")?.as_str()?.to_string();
    let mut turns = Vec::new();
    for (axis, key) in [(0, "x"), (1, "y")] {
        let deg = v.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        if deg != 0.0 {
            turns.push(Turn { origin: CENTRE, axis, degrees: -deg, rescale: false });
        }
    }
    Some((model, turns))
}

/// The models block state `state` shows: its variant (the one whose
/// properties it has; the first otherwise), or each multipart part it meets.
fn block_models(res: &Resources, state: &str) -> Vec<(String, Vec<Turn>)> {
    let (id, props) = split_state(state);
    let (ns, path) = yog_ids::split(id);
    let Some(json) = res.read(ns, &yog_ids::blockstate_file(path)).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
        return Vec::new();
    };
    if let Some(variants) = json.get("variants").and_then(Value::as_object) {
        let fits = |key: &str| key.split(',').filter(|kv| !kv.is_empty()).all(|kv| kv.split_once('=').is_some_and(|(k, v)| props.get(k) == Some(&v)));
        let chosen = variants.iter().find(|(k, _)| fits(k)).or_else(|| variants.iter().next());
        return chosen.and_then(|(_, v)| applied(v)).into_iter().collect();
    }
    json.get("multipart")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| part.get("when").is_none_or(|w| meets(w, &props)))
        .filter_map(|part| applied(part.get("apply")?))
        .collect()
}

/// Whether `id` is a block (the game's, or the mod's `.yblock`).
pub fn is_block(res: &Resources, id: &str) -> bool {
    let id = split_state(id).0;
    let (ns, path) = yog_ids::split(id);
    res.read(ns, &yog_ids::blockstate_file(path)).is_some()
        || res.mod_root.as_deref().is_some_and(|root| crate::design::content::content_block_faces(root, id).is_some())
}

fn shape(res: &Resources, state: &str, cache: &mut HashMap<String, Shape>) -> Shape {
    if let Some(s) = cache.get(state) {
        return s.clone();
    }
    let id = split_state(state).0;
    let s = mod_shape(res, id).unwrap_or_else(|| game_shape(res, state));
    cache.insert(state.to_string(), s.clone());
    s
}

/// The mod's own block: its `.yblock` model — its elements, else a cube of its textures.
fn mod_shape(res: &Resources, id: &str) -> Option<Shape> {
    let root = res.mod_root.as_deref()?;
    if let Some((elements, textures)) = crate::design::content::content_block_model(root, id) {
        let mut decoded: HashMap<String, Option<Arc<Image>>> = HashMap::new();
        let mut texture = |var: &str| -> Option<Arc<Image>> {
            let var = var.trim_start_matches('#');
            decoded.entry(var.to_string()).or_insert_with(|| textures.get(var).and_then(|b| decode_png(b)).map(Arc::new)).clone()
        };
        let s = shape_of(&elements, &mut texture);
        if !s.is_empty() {
            return Some(s);
        }
    }
    crate::design::content::content_block_faces(root, id).map(|faces| cube(faces.map(|b| b.and_then(|b| decode_png(&b)).map(Arc::new))))
}

/// The game's block state: every model its blockstate shows for it, each
/// turned as the blockstate says.
fn game_shape(res: &Resources, state: &str) -> Shape {
    let mut models = block_models(res, state);
    if models.is_empty() {
        let (ns, path) = yog_ids::split(split_state(state).0);
        models.push((format!("{ns}:block/{path}"), Vec::new()));
    }
    let mut out = Shape::new();
    for (model, turns) in models {
        for mut cuboid in model_shape(res, &model) {
            cuboid.turns.extend(turns.iter().copied());
            out.push(cuboid);
        }
    }
    out
}

/// One block model's boxes: its elements, else a cube of its textures.
fn model_shape(res: &Resources, model: &str) -> Shape {
    let vars = model_textures(res, model, 0);
    // `#var` → its value, following references.
    let resolve = |key: &str| -> Option<String> {
        let mut v = vars.get(key)?.clone();
        for _ in 0..8 {
            match v.strip_prefix('#') {
                Some(r) => v = vars.get(r)?.clone(),
                None => return Some(v),
            }
        }
        None
    };
    let load = |tex: Option<String>| -> Option<Arc<Image>> {
        let tex = tex?;
        let (ns, path) = yog_ids::split(&tex);
        decode_png(&res.read(ns, &yog_ids::texture_file(path))?).map(Arc::new)
    };
    if let Some(elements) = model_elements(res, model, 0) {
        let mut texture = |var: &str| load(resolve(var.trim_start_matches('#')));
        let s = shape_of(&elements, &mut texture);
        if !s.is_empty() {
            return s;
        }
    }
    cube(FACE_TEXTURES.map(|keys| load(keys.iter().chain(&[PARTICLE]).find_map(|k| resolve(k)))))
}

/// `x y z state` lines → blocks.
fn parse(list: &str) -> Vec<([i32; 3], String)> {
    list.lines()
        .filter_map(|l| {
            let mut p = l.trim().splitn(4, ' ');
            let at = [p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?];
            Some((at, p.next()?.to_string()))
        })
        .collect()
}

pub fn draw(c: &mut Canvas, list: &str, rect: [f32; 4], yaw: f32, pitch: f32) {
    let mut cache = HashMap::new();
    draw_with(c, &parse(list), rect, yaw, pitch, |res, id| shape(res, id, &mut cache));
}

/// One cube with its own `faces` (a mob's head, say), as [`draw`] draws a block.
pub fn draw_cube(c: &mut Canvas, faces: Faces, rect: [f32; 4], yaw: f32, pitch: f32) {
    draw_with(c, &[([0, 0, 0], String::new())], rect, yaw, pitch, |_, _| cube(faces.clone()));
}

fn draw_with(
    c: &mut Canvas,
    blocks: &[([i32; 3], String)],
    [x, y, w, h]: [f32; 4],
    yaw: f32,
    pitch: f32,
    mut shape_of: impl FnMut(&Resources, &str) -> Shape,
) {
    if blocks.is_empty() || w <= 0.0 || h <= 0.0 {
        return;
    }
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for (p, _) in blocks {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let size = (0..3).map(|i| hi[i] - lo[i]).max().unwrap_or(0) as f32 + 1.0;
    // As the hosts: as big as fits (1.7 × the largest side).
    let s = w.min(h) / (size * 1.7);
    let centre = [(lo[0] + hi[0] + 1) as f32 / 2.0, (lo[1] + hi[1] + 1) as f32 / 2.0, (lo[2] + hi[2] + 1) as f32 / 2.0];
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    // World → view: about Y by yaw, then about X by pitch; x right, y up, z toward the viewer.
    let view = |p: [f32; 3]| {
        let (px, py, pz) = (p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]);
        let x1 = px * cy + pz * sy;
        let z1 = -px * sy + pz * cy;
        let y2 = py * cp - z1 * sp;
        let z2 = py * sp + z1 * cp;
        [x1, y2, z2]
    };
    let screen = |v: [f32; 3]| [x + w / 2.0 + v[0] * s, y + h / 2.0 - v[1] * s];
    let occupied: HashSet<[i32; 3]> = blocks.iter().map(|(p, _)| *p).collect();

    // Every face that can be seen: (depth, texture, corner, edge a, edge b, shade).
    let mut quads = Vec::new();
    // (normal, corner offset, a, b, slot, shade) per face of a unit cube.
    const FACES: [([i32; 3], [f32; 3], [f32; 3], [f32; 3], usize, f32); 6] = [
        ([0, 1, 0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0, 1.0),
        ([0, -1, 0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], 1, 0.5),
        ([0, 0, -1], [1.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], 2, 0.8),
        ([0, 0, 1], [0.0, 1.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], 3, 0.8),
        ([-1, 0, 0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0], 4, 0.6),
        ([1, 0, 0], [1.0, 1.0, 1.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0], 5, 0.6),
    ];
    for (p, id) in blocks {
        for cuboid in shape_of(c.resources(), id) {
            let (f, t) = (cuboid.from, cuboid.to);
            // A unit cube's corner → this box's (in sixteenths, turned), in the world.
            let at = |u: [f32; 3]| {
                let q = cuboid.turns.iter().fold([0, 1, 2].map(|i| f[i] + u[i] * (t[i] - f[i])), |q, turn| turn.apply(q));
                [0, 1, 2].map(|i| p[i] as f32 + q[i] / BLOCK_UNITS)
            };
            for (n, corner, a, b, slot, shade) in FACES {
                // On the block's side, against a neighbour: hidden (a turned box is never flush).
                let on_side = cuboid.turns.is_empty()
                    && (0..3).all(|i| n[i] == 0 || (n[i] > 0 && t[i] >= BLOCK_UNITS) || (n[i] < 0 && f[i] <= 0.0));
                if on_side && occupied.contains(&[p[0] + n[0], p[1] + n[1], p[2] + n[2]]) {
                    continue;
                }
                // Facing away from the viewer: unseen.
                let normal = cuboid.turns.iter().fold(n.map(|v| v as f32), |v, turn| turn.apply_dir(v));
                let nv = view([0, 1, 2].map(|i| centre[i] + normal[i]));
                if nv[2] <= 0.0 {
                    continue;
                }
                let Some(img) = cuboid.faces[slot].clone() else { continue };
                let o = at(corner);
                let oa = at([corner[0] + a[0], corner[1] + a[1], corner[2] + a[2]]);
                let ob = at([corner[0] + b[0], corner[1] + b[1], corner[2] + b[2]]);
                let (vo, va, vb) = (view(o), view(oa), view(ob));
                let mid = view([0, 1, 2].map(|i| (oa[i] + ob[i]) / 2.0));
                let (so, sa, sb) = (screen(vo), screen(va), screen(vb));
                quads.push((mid[2], Some(img), so, [sa[0] - so[0], sa[1] - so[1]], [sb[0] - so[0], sb[1] - so[1]], shade));
            }
        }
    }
    // Back to front.
    quads.sort_by(|a, b| a.0.total_cmp(&b.0));
    let missing = Arc::new(missing_image());
    for (_, img, o, a, b, shade) in quads {
        let img = img.unwrap_or_else(|| missing.clone());
        c.quad(&img, o, a, b, shade);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32) -> Image {
        Image { width: w, height: h, rgba: (0..w * h).flat_map(|i| [i as u8, 0, 0, 255]).collect() }
    }

    fn opaque(shape: Shape) -> usize {
        let res = Resources { mod_root: None, vanilla: None };
        let mut c = Canvas::new(32, 32, 1, &res);
        draw_with(&mut c, &[([0, 0, 0], "x".into())], [0.0, 0.0, 32.0, 32.0], 225.0, 30.0, |_, _| shape.clone());
        c.rgba.chunks_exact(4).filter(|p| p[3] > 0).count()
    }

    #[test]
    fn a_models_boxes_are_drawn_their_size_with_their_uv() {
        let img = Arc::new(solid(16, 16));
        let elements: Vec<Value> = serde_json::from_str(
            r##"[{ "from": [5, 5, 5], "to": [11, 11, 11], "faces": {
                "up": { "uv": [5, 5, 11, 11], "texture": "#all" }, "north": { "texture": "#all" } } }]"##,
        )
        .unwrap();
        let shape = shape_of(&elements, &mut |v: &str| (v == "#all").then(|| img.clone()));
        assert_eq!(shape.len(), 1);
        let up = shape[0].faces[0].as_ref().unwrap();
        assert_eq!((up.width, up.height), (6, 6), "the uv's part");
        assert_eq!(up.pixel(0, 0), img.pixel(5, 5));
        let north = shape[0].faces[2].as_ref().unwrap();
        assert_eq!((north.width, north.height), (6, 6), "no uv: the box's own part");
        assert!(shape[0].faces[1].is_none(), "a face the model leaves out isn't drawn");

        let small = opaque(shape);
        let full = opaque(cube(std::array::from_fn(|_| Some(img.clone()))));
        assert!(small > 0 && small * 4 < full, "a cable's core, not a whole block: {small} vs {full}");
    }

    #[test]
    fn parses_block_lines() {
        let b = parse("0 0 0 minecraft:stone\n1 2 -1 minecraft:furnace[facing=north]\nbad");
        assert_eq!(b, [([0, 0, 0], "minecraft:stone".to_string()), ([1, 2, -1], "minecraft:furnace[facing=north]".to_string())]);
    }

    #[test]
    fn a_state_meets_multipart_conditions() {
        let (id, props) = split_state("minecraft:brewing_stand[has_bottle_0=true,has_bottle_1=false]");
        assert_eq!(id, "minecraft:brewing_stand");
        let when = |j: &str| serde_json::from_str::<Value>(j).unwrap();
        assert!(meets(&when(r#"{ "has_bottle_0": "true" }"#), &props));
        assert!(!meets(&when(r#"{ "has_bottle_1": "true" }"#), &props));
        assert!(meets(&when(r#"{ "has_bottle_1": "true|false" }"#), &props));
        assert!(meets(&when(r#"{ "OR": [{ "has_bottle_2": "true" }, { "has_bottle_0": "true" }] }"#), &props));
        assert!(!meets(&when(r#"{ "has_bottle_2": "false" }"#), &props), "a property the state doesn't name");
    }

    #[test]
    fn turns_turn_about_their_origin() {
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4);
        // A quarter turn about y through the centre: east goes north.
        let y = Turn { origin: CENTRE, axis: 1, degrees: 90.0, rescale: false };
        assert!(close(y.apply([16.0, 8.0, 8.0]), [8.0, 8.0, 0.0]), "{:?}", y.apply([16.0, 8.0, 8.0]));
        assert!(close(y.apply(CENTRE), CENTRE));
        // Rescaled at 45°: the diagonal is stretched back to the block.
        let r = Turn { origin: CENTRE, axis: 1, degrees: 45.0, rescale: true };
        let d = r.apply([16.0, 8.0, 8.0]);
        assert!(((d[0] - 8.0).powi(2) + (d[2] - 8.0).powi(2)).sqrt() > 8.0 * 1.4);
    }
}

