//! `draw2d_blocks` in the designer: blocks in 3D, as the game draws them in
//! a GUI (an orthographic view, turned and tilted), each block a cube with
//! its model's boxes (`elements`: a cable's core, a slab's half) with
//! their textures — the faces you'd see, back to front, the top lit, the
//! sides shaded like Minecraft's. A model without elements is a cube.
//! A block state picks its blockstate's variant (turned by its `x`/`y`)
//! or every multipart part whose `when` it meets; elements keep their own
//! rotations.

use std::collections::{HashMap, HashSet};
use yog_ids::direction::{DOWN, EAST, NORTH, SOUTH, UP, WEST};
use yog_ids::texture_slot::{ALL, BOTTOM, END, FRONT, SIDE, TEXTURE, TOP};
use yog_sync::Arc;

use yog_data::Value;

use super::assets::{Image, Resources, decode_png, missing_image};
use super::raster::Canvas;
use yog_gfx::Rect;

/// A cube's textures per face: up, down, north, south, west, east.
pub(crate) type Faces = [Option<Arc<Image>>; 6];

/// One box of a model, in sixteenths of a block: per face (up, down,
/// north, south, west, east) its texture, already cut to the face's uv
/// (`None`: the box has no such face), and the turns it's drawn with, in
/// order (its element's, then its variant's).
#[derive(Clone)]
pub(crate) struct Cuboid {
    pub(crate) from: [f32; 3],
    pub(crate) to: [f32; 3],
    pub(crate) faces: Faces,
    pub(crate) turns: Vec<Turn>,
}

/// A turn of a box about `axis` (0 x, 1 y, 2 z) through `origin`
/// (sixteenths of a block), `degrees` counter-clockwise looking down the
/// axis; `rescale` stretches the turned box's other axes back to the block
/// (an element's `rescale`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Turn {
    pub(crate) origin: [f32; 3],
    pub(crate) axis: usize,
    pub(crate) degrees: f32,
    pub(crate) rescale: bool,
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
        Turn {
            origin: [0.0; 3],
            rescale: false,
            ..*self
        }
        .apply(v)
    }
}

/// A block as drawn: its model's boxes.
pub(crate) type Shape = Vec<Cuboid>;

/// A whole-block cube with `faces` (a missing one: the missing texture).
/// A unit cube's face: normal, corner offset, edges a and b, texture slot, shade.
type CubeFace = ([i32; 3], [f32; 3], [f32; 3], [f32; 3], usize, f32);

pub(crate) fn cube(faces: Faces) -> Shape {
    let faces = faces.map(|f| Some(f.unwrap_or_else(|| Arc::new(missing_image()))));
    vec![Cuboid {
        from: [0.0; 3],
        to: [BLOCK_UNITS; 3],
        faces,
        turns: Vec::new(),
    }]
}

const FACE_NAMES: [&str; 6] = [UP, DOWN, NORTH, SOUTH, WEST, EAST];

/// The texture variables the game's block models draw each face with (in
/// [`FACE_NAMES`] order), most particular first: a cube's `up`, a
/// column's `end`, a `cube_all`'s `all`, a one-texture model's `texture`.
pub(crate) const FACE_TEXTURES: [&[&str]; 6] = [
    &[UP, TOP, END, ALL, TEXTURE],
    &[DOWN, BOTTOM, END, ALL, TEXTURE],
    &[NORTH, FRONT, SIDE, ALL, TEXTURE],
    &[SOUTH, SIDE, ALL, TEXTURE],
    &[WEST, SIDE, ALL, TEXTURE],
    &[EAST, SIDE, ALL, TEXTURE],
];

/// What a face falls back to when its model has none of [`FACE_TEXTURES`].
const PARTICLE: &str = yog_ids::model::PARTICLE;

/// The part of a face's texture its box shows when the model gives no `uv`.
fn default_uv(slot: usize, f: [f32; 3], t: [f32; 3]) -> [f32; 4] {
    match slot {
        0 => [f[0], f[2], t[0], t[2]],
        1 => [f[0], BLOCK_UNITS - t[2], t[0], BLOCK_UNITS - f[2]],
        2 => [
            BLOCK_UNITS - t[0],
            BLOCK_UNITS - t[1],
            BLOCK_UNITS - f[0],
            BLOCK_UNITS - f[1],
        ],
        3 => [f[0], BLOCK_UNITS - t[1], t[0], BLOCK_UNITS - f[1]],
        4 => [f[2], BLOCK_UNITS - t[1], t[2], BLOCK_UNITS - f[1]],
        _ => [
            BLOCK_UNITS - t[2],
            BLOCK_UNITS - t[1],
            BLOCK_UNITS - f[2],
            BLOCK_UNITS - f[1],
        ],
    }
}

/// `img`'s part `uv` (sixteenths of it; reversed: mirrored).
fn cut(img: &Image, [u0, v0, u1, v1]: [f32; 4]) -> Image {
    let (sx, sy) = (
        img.width as f32 / BLOCK_UNITS,
        img.width.min(img.height) as f32 / BLOCK_UNITS,
    );
    let (x0, x1) = ((u0 * sx).round() as i32, (u1 * sx).round() as i32);
    let (y0, y1) = ((v0 * sy).round() as i32, (v1 * sy).round() as i32);
    let (w, h) = (
        (x1 - x0).unsigned_abs().max(1),
        (y1 - y0).unsigned_abs().max(1),
    );
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let px = if x1 >= x0 { x0 + x } else { x0 - 1 - x };
            let py = if y1 >= y0 { y0 + y } else { y0 - 1 - y };
            let (px, py) = (
                px.clamp(0, img.width as i32 - 1) as u32,
                py.clamp(0, img.height as i32 - 1) as u32,
            );
            rgba.extend_from_slice(&img.pixel(px, py).to_rgba8());
        }
    }
    Image {
        width: w,
        height: h,
        rgba,
    }
}

/// A model's `elements` as boxes; `texture("#var" or "var")` gives a face's texture.
pub(crate) fn shape_of(
    elements: &[Value],
    texture: &mut dyn FnMut(&str) -> Option<Arc<Image>>,
) -> Shape {
    let triple = |v: Option<&Value>| -> Option<[f32; 3]> {
        let a = v?.as_list()?;
        Some([
            a.first()?.as_f64()? as f32,
            a.get(1)?.as_f64()? as f32,
            a.get(2)?.as_f64()? as f32,
        ])
    };
    elements
        .iter()
        .filter_map(|e| {
            let (from, to) = (
                triple(e.get(yog_ids::model::FROM))?,
                triple(e.get(yog_ids::model::TO))?,
            );
            let (f, t) = (
                [0, 1, 2].map(|i| from[i].min(to[i])),
                [0, 1, 2].map(|i| from[i].max(to[i])),
            );
            let faces = e.get(yog_ids::model::FACES)?;
            let faces: Faces = std::array::from_fn(|slot| {
                let face = faces.get(FACE_NAMES[slot])?;
                let img = face
                    .get(yog_ids::model::TEXTURE)
                    .and_then(Value::as_str)
                    .and_then(&mut *texture)
                    .unwrap_or_else(|| Arc::new(missing_image()));
                let uv = face
                    .get(yog_ids::model::UV)
                    .and_then(Value::as_list)
                    .and_then(|a| {
                        Some([
                            a.first()?.as_f64()? as f32,
                            a.get(1)?.as_f64()? as f32,
                            a.get(2)?.as_f64()? as f32,
                            a.get(3)?.as_f64()? as f32,
                        ])
                    })
                    .unwrap_or_else(|| default_uv(slot, f, t));
                Some(Arc::new(cut(&img, uv)))
            });
            Some(Cuboid {
                from: f,
                to: t,
                faces,
                turns: element_turn(e).into_iter().collect(),
            })
        })
        .collect()
}

/// An element's `rotation`: `{ origin, axis: "x"|"y"|"z", angle, rescale }`.
fn element_turn(e: &Value) -> Option<Turn> {
    let r = e.get(yog_ids::model::ROTATION)?;
    let o = r.get(yog_ids::model::ORIGIN)?.as_list()?;
    let origin = [
        o.first()?.as_f64()? as f32,
        o.get(1)?.as_f64()? as f32,
        o.get(2)?.as_f64()? as f32,
    ];
    let axis = ["x", "y", "z"]
        .iter()
        .position(|a| r.get(yog_ids::model::AXIS).and_then(Value::as_str) == Some(*a))?;
    let degrees = r.get(yog_ids::model::ANGLE)?.as_f64()? as f32;
    Some(Turn {
        origin,
        axis,
        degrees,
        rescale: r
            .get(yog_ids::model::RESCALE)
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// The first `elements` up a model's parents.
fn model_elements(res: &Resources, model: &str, depth: usize) -> Option<Vec<Value>> {
    if depth > 8 {
        return None;
    }
    let (ns, path) = yog_ids::split(model);
    let path = path
        .strip_prefix("block/")
        .map_or(format!("block/{path}"), |p| format!("block/{p}"));
    let json = crate::design::data::read_bytes(&res.read(ns, &yog_ids::model_file(&path))?)?;
    if let Some(e) = json.get(yog_ids::model::ELEMENTS).and_then(Value::as_list) {
        return Some(e.to_vec());
    }
    model_elements(res, json.get(yog_ids::model::PARENT)?.as_str()?, depth + 1)
}

/// The texture variables of a block model, its parents' included.
fn model_textures(res: &Resources, model: &str, depth: usize) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if depth > 8 {
        return out;
    }
    let (ns, path) = yog_ids::split(model);
    let path = path
        .strip_prefix("block/")
        .map_or(format!("block/{path}"), |p| format!("block/{p}"));
    let Some(json) = res
        .read(ns, &yog_ids::model_file(&path))
        .and_then(|b| crate::design::data::read_bytes(&b))
    else {
        return out;
    };
    if let Some(parent) = json.get(yog_ids::model::PARENT).and_then(Value::as_str) {
        out = model_textures(res, parent, depth + 1);
    }
    if let Some(t) = json.get(yog_ids::model::TEXTURES).and_then(Value::as_map) {
        for (k, v) in t {
            if let Some(v) = v.as_str() {
                out.insert(k.to_string(), v.to_string());
            }
        }
    }
    out
}

/// A block state's id and its properties: `minecraft:furnace[facing=north]`.
fn split_state(state: &str) -> (&str, HashMap<&str, &str>) {
    let (id, props) = state.split_once('[').unwrap_or((state, ""));
    let props = props
        .trim_end_matches(']')
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    (id, props)
}

/// Whether `props` meet a multipart `when` (every property one of its
/// `a|b` values; `OR`/`AND`: any/all of a list). A property the state
/// doesn't name meets nothing.
fn meets(when: &Value, props: &HashMap<&str, &str>) -> bool {
    let Some(map) = when.as_map() else {
        return true;
    };
    if let Some(any) = map.get(yog_ids::model::OR).and_then(Value::as_list) {
        return any.iter().any(|w| meets(w, props));
    }
    if let Some(all) = map.get(yog_ids::model::AND).and_then(Value::as_list) {
        return all.iter().all(|w| meets(w, props));
    }
    map.iter().all(|(k, v)| {
        let want = v
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| yog_data::text::to_string(v));
        props
            .get(k)
            .is_some_and(|have| want.split('|').any(|w| w == *have))
    })
}

/// A blockstate's `apply`/variant: its model and its turns (`x` then `y`,
/// about the block's centre; the game turns models clockwise looking down
/// an axis).
fn applied(v: &Value) -> Option<(String, Vec<Turn>)> {
    let v = match v.as_list() {
        Some(variants) => variants.first()?,
        None => v,
    };
    let model = v.get(yog_ids::model::MODEL)?.as_str()?.to_string();
    let mut turns = Vec::new();
    for (axis, key) in [(0, "x"), (1, "y")] {
        let deg = v.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        if deg != 0.0 {
            turns.push(Turn {
                origin: CENTRE,
                axis,
                degrees: -deg,
                rescale: false,
            });
        }
    }
    Some((model, turns))
}

/// The models block state `state` shows: its variant (the one whose
/// properties it has; the first otherwise), or each multipart part it meets.
fn block_models(res: &Resources, state: &str) -> Vec<(String, Vec<Turn>)> {
    let (id, props) = split_state(state);
    let (ns, path) = yog_ids::split(id);
    let Some(json) = res
        .read(ns, &yog_ids::blockstate_file(path))
        .and_then(|b| crate::design::data::read_bytes(&b))
    else {
        return Vec::new();
    };
    if let Some(variants) = json.get(yog_ids::model::VARIANTS).and_then(Value::as_map) {
        let fits = |key: &str| {
            key.split(',').filter(|kv| !kv.is_empty()).all(|kv| {
                kv.split_once('=')
                    .is_some_and(|(k, v)| props.get(k) == Some(&v))
            })
        };
        let chosen = variants
            .iter()
            .find(|(k, _)| fits(k))
            .or_else(|| variants.iter().next());
        return chosen.and_then(|(_, v)| applied(v)).into_iter().collect();
    }
    json.get(yog_ids::model::MULTIPART)
        .and_then(Value::as_list)
        .into_iter()
        .flatten()
        .filter(|part| {
            part.get(yog_ids::model::WHEN)
                .is_none_or(|w| meets(w, &props))
        })
        .filter_map(|part| applied(part.get(yog_ids::model::APPLY)?))
        .collect()
}

/// Whether `id` is a block (the game's, or the mod's `.yblock`).
pub(crate) fn is_block(res: &Resources, id: &str) -> bool {
    let id = split_state(id).0;
    let (ns, path) = yog_ids::split(id);
    res.read(ns, &yog_ids::blockstate_file(path)).is_some()
        || res
            .mod_root
            .as_deref()
            .is_some_and(|root| crate::design::content::content_block_faces(root, id).is_some())
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
            decoded
                .entry(var.to_string())
                .or_insert_with(|| textures.get(var).and_then(|b| decode_png(b)).map(Arc::new))
                .clone()
        };
        let s = shape_of(&elements, &mut texture);
        if !s.is_empty() {
            return Some(s);
        }
    }
    crate::design::content::content_block_faces(root, id)
        .map(|faces| cube(faces.map(|b| b.and_then(|b| decode_png(&b)).map(Arc::new))))
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
            let at = [
                p.next()?.parse().ok()?,
                p.next()?.parse().ok()?,
                p.next()?.parse().ok()?,
            ];
            Some((at, p.next()?.to_string()))
        })
        .collect()
}

pub(crate) fn draw(c: &mut Canvas, list: &str, rect: Rect, yaw: f32, pitch: f32) {
    let mut cache = HashMap::new();
    draw_with(c, &parse(list), rect, (yaw, pitch), |res, id| {
        shape(res, id, &mut cache)
    });
}

/// One cube with its own `faces` (a mob's head, say), as [`draw`] draws a block.
pub(crate) fn draw_cube(c: &mut Canvas, faces: Faces, rect: Rect, yaw: f32, pitch: f32) {
    draw_with(
        c,
        &[([0, 0, 0], String::new())],
        rect,
        (yaw, pitch),
        |_, _| cube(faces.clone()),
    );
}

/// The game's shading of a block's faces by the way they look: up, down,
/// north and south, west and east.
const SHADE_UP: f32 = 1.0;
const SHADE_DOWN: f32 = 0.5;
const SHADE_NORTH_SOUTH: f32 = 0.8;
const SHADE_WEST_EAST: f32 = 0.6;
/// As the hosts draw them: the blocks as big as fit, this many times their largest side.
const FIT: f32 = 1.7;

/// (normal, corner offset, a, b, slot, shade) per face of a unit cube.
const FACES: [CubeFace; 6] = [
    (
        [0, 1, 0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        0,
        SHADE_UP,
    ),
    (
        [0, -1, 0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        1,
        SHADE_DOWN,
    ),
    (
        [0, 0, -1],
        [1.0, 1.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        2,
        SHADE_NORTH_SOUTH,
    ),
    (
        [0, 0, 1],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        3,
        SHADE_NORTH_SOUTH,
    ),
    (
        [-1, 0, 0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, -1.0, 0.0],
        4,
        SHADE_WEST_EAST,
    ),
    (
        [1, 0, 0],
        [1.0, 1.0, 1.0],
        [0.0, 0.0, -1.0],
        [0.0, -1.0, 0.0],
        5,
        SHADE_WEST_EAST,
    ),
];

/// How the blocks are seen: their centre, the turn (yaw's and pitch's sine
/// and cosine), the box drawn in and the scale.
struct Camera {
    centre: [f32; 3],
    yaw: (f32, f32),
    pitch: (f32, f32),
    rect: Rect,
    scale: f32,
}

impl Camera {
    /// The blocks' bounds fitted into `rect`, turned by `yaw` and `pitch` (degrees).
    fn fit(blocks: &[([i32; 3], String)], rect: Rect, yaw: f32, pitch: f32) -> Camera {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for (p, _) in blocks {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        let size = (0..3).map(|i| hi[i] - lo[i]).max().unwrap_or(0) as f32 + 1.0;
        Camera {
            centre: [0, 1, 2].map(|i| (lo[i] + hi[i] + 1) as f32 / 2.0),
            yaw: yaw.to_radians().sin_cos(),
            pitch: pitch.to_radians().sin_cos(),
            rect,
            scale: rect.w.min(rect.h) / (size * FIT),
        }
    }

    /// World → view: about Y by yaw, then about X by pitch; x right, y up, z toward the viewer.
    fn view(&self, p: [f32; 3]) -> [f32; 3] {
        let ((sy, cy), (sp, cp)) = (self.yaw, self.pitch);
        let [px, py, pz] = [0, 1, 2].map(|i| p[i] - self.centre[i]);
        let x1 = px * cy + pz * sy;
        let z1 = -px * sy + pz * cy;
        [x1, py * cp - z1 * sp, py * sp + z1 * cp]
    }

    /// A view point on the canvas.
    fn screen(&self, v: [f32; 3]) -> [f32; 2] {
        let Rect { x, y, w, h } = self.rect;
        [
            x + w / 2.0 + v[0] * self.scale,
            y + h / 2.0 - v[1] * self.scale,
        ]
    }
}

/// A face to draw: (depth, texture, corner, edge a, edge b, shade).
type Quad = (f32, Option<Arc<Image>>, [f32; 2], [f32; 2], [f32; 2], f32);

fn draw_with(
    c: &mut Canvas,
    blocks: &[([i32; 3], String)],
    rect: Rect,
    (yaw, pitch): (f32, f32),
    mut shape_of: impl FnMut(&Resources, &str) -> Shape,
) {
    if blocks.is_empty() || rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let cam = Camera::fit(blocks, rect, yaw, pitch);
    let occupied: HashSet<[i32; 3]> = blocks.iter().map(|(p, _)| *p).collect();
    let mut quads = Vec::new();
    for (p, id) in blocks {
        for cuboid in shape_of(c.resources(), id) {
            seen_faces(&cam, *p, &cuboid, &occupied, &mut quads);
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

/// The faces of `cuboid` (of the block at `p`) that can be seen: not against
/// a neighbour, not facing away.
fn seen_faces(
    cam: &Camera,
    p: [i32; 3],
    cuboid: &Cuboid,
    occupied: &HashSet<[i32; 3]>,
    quads: &mut Vec<Quad>,
) {
    let (f, t) = (cuboid.from, cuboid.to);
    // A unit cube's corner → this box's (in sixteenths, turned), in the world.
    let at = |u: [f32; 3]| {
        let q = cuboid
            .turns
            .iter()
            .fold([0, 1, 2].map(|i| f[i] + u[i] * (t[i] - f[i])), |q, turn| {
                turn.apply(q)
            });
        [0, 1, 2].map(|i| p[i] as f32 + q[i] / BLOCK_UNITS)
    };
    for (n, corner, a, b, slot, shade) in FACES {
        // On the block's side, against a neighbour: hidden (a turned box is never flush).
        let on_side = cuboid.turns.is_empty()
            && (0..3).all(|i| {
                n[i] == 0 || (n[i] > 0 && t[i] >= BLOCK_UNITS) || (n[i] < 0 && f[i] <= 0.0)
            });
        if on_side && occupied.contains(&[p[0] + n[0], p[1] + n[1], p[2] + n[2]]) {
            continue;
        }
        // Facing away from the viewer: unseen.
        let normal = cuboid
            .turns
            .iter()
            .fold(n.map(|v| v as f32), |v, turn| turn.apply_dir(v));
        if cam.view([0, 1, 2].map(|i| cam.centre[i] + normal[i]))[2] <= 0.0 {
            continue;
        }
        let Some(img) = cuboid.faces[slot].clone() else {
            continue;
        };
        let o = at(corner);
        let oa = at([0, 1, 2].map(|i| corner[i] + a[i]));
        let ob = at([0, 1, 2].map(|i| corner[i] + b[i]));
        let mid = cam.view([0, 1, 2].map(|i| (oa[i] + ob[i]) / 2.0));
        let (so, sa, sb) = (
            cam.screen(cam.view(o)),
            cam.screen(cam.view(oa)),
            cam.screen(cam.view(ob)),
        );
        quads.push((
            mid[2],
            Some(img),
            so,
            [sa[0] - so[0], sa[1] - so[1]],
            [sb[0] - so[0], sb[1] - so[1]],
            shade,
        ));
    }
}

#[cfg(test)]
mod tests;
