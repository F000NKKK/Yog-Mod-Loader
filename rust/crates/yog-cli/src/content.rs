//! `.yblock` / `.yitem` content files — a mod's blocks and items: their
//! properties, 3D model, block states and item form, in one JSON file each,
//! edited in Yog-IDLE's 3D model editor — and `.yrecipe` files, one recipe
//! each (crafting, shapeless, smelting), edited in its recipe editor. See
//! Yog-IDLE's `docs/blocks-items.md` for the formats.
//!
//! `yog build` reads `content/**/*.yblock` and `content/**/*.yitem` and
//!
//! - writes Minecraft's JSON for them: block models, blockstates, item
//!   models, 1.21.4+ item definitions (`assets/<ns>/items/`), block loot
//!   tables, the `mineable/<tool>` and `needs_<level>_tool` block tags;
//! - puts their names and tooltips into the lang files (a `@<Set>.<name>`
//!   text of a `.yres` file under the key Minecraft looks up, a literal into
//!   `en_us`);
//! - bundles textures given as paths inside `resources/`;
//! - generates `.yog-build/generated/yog-content.g.rs` — `Blocks`/`Items`
//!   with an id constant and a `BlockDef`/`ItemDef` builder per entry and
//!   a `register` function — pulled in by `yog_api::content!();`, which
//!   also registers them (unless an entry says `"register": "manual"`) and
//!   every recipe.

mod import;
pub mod json;
mod files;
mod recipe;
mod refs;

pub use import::import_command;

use crate::yres;
use yog_data::Path as At;
use json::num_value;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Where content files live, under the mod root.
pub const CONTENT_DIR: &str = "content";
/// The generated Rust code, under the mod root.
pub const GENERATED_FILE: &str = ".yog-build/generated/yog-content.g.rs";

/// A generated file: path inside the `.yog` and its bytes.
pub type Output = (String, Vec<u8>);

/// What a content file defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// `.yblock` — a block and its item form.
    Block,
    /// `.yitem` — an item.
    Item,
    /// `.yrecipe` — a recipe.
    Recipe,
    /// `.ybook` — a guide book (`yog_book::Book` as JSON).
    Book,
    /// `.ylayout` — how books show a kind of recipe (`yog_book::layout`).
    Layout,
    /// `.ydef` — a definition of some kind: a ladder, a level on one.
    Def,
}

impl Kind {
    pub fn of(path: &Path) -> Option<Kind> {
        match path.extension()?.to_str()? {
            "yblock" => Some(Kind::Block),
            "yitem" => Some(Kind::Item),
            "yrecipe" => Some(Kind::Recipe),
            "ybook" => Some(Kind::Book),
            "ylayout" => Some(Kind::Layout),
            "ydef" => Some(Kind::Def),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Kind::Block => "block",
            Kind::Item => "item",
            Kind::Recipe => "recipe",
            Kind::Book => "book",
            Kind::Layout => "layout",
            Kind::Def => "definition",
        }
    }
}

/// Everything `yog build` makes of a mod's content files.
#[derive(Debug, Default)]
pub struct Compiled {
    /// Models, blockstates, item definitions, loot tables, textures.
    pub outputs: Vec<Output>,
    /// Lang entries: `(namespace, locale)` → key → text, merged into the
    /// lang files by [`merge_outputs`].
    lang: BTreeMap<(String, String), BTreeMap<String, String>>,
    /// `.yres` text keys to copy under another key: (from, to).
    lang_aliases: Vec<(String, String)>,
    /// Block tag path (`data/minecraft/tags/block/...json`) → entries.
    tags: BTreeMap<String, BTreeSet<String>>,
    /// The generated Rust code; `None` when the mod has no content files.
    pub source: Option<String>,
    pub warnings: Vec<String>,
    pub blocks: usize,
    pub items: usize,
    pub recipes: usize,
    pub books: usize,
}

// ── Vocabulary ───────────────────────────────────────────────────────────────

pub const DIRECTIONS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
pub const DISPLAY_SLOTS: [&str; 8] = [
    "thirdperson_righthand",
    "thirdperson_lefthand",
    "firstperson_righthand",
    "firstperson_lefthand",
    "gui",
    "head",
    "ground",
    "fixed",
];
/// The sound groups the loaders map (`BlockDef::sound`).
pub const SOUNDS: [&str; 10] = [
    "stone",
    "wood",
    "grass",
    "gravel",
    "sand",
    "snow",
    "metal",
    "glass",
    "wool",
    "nether_brick",
];
pub const RENDER_LAYERS: [&str; 4] = ["solid", "cutout", "cutout_mipped", "translucent"];
pub const RECIPE_TYPES: [&str; 8] =
    ["shaped", "shapeless", "smelting", "blasting", "smoking", "campfire_cooking", "stonecutting", "smithing"];

/// A model template — `"parent": "cube_all"` — and the texture slots it takes.
pub struct Template {
    pub name: &'static str,
    /// The vanilla model it compiles to.
    pub parent: &'static str,
    /// Its texture slots; for item sprites, `layer0` (more layers allowed).
    pub slots: &'static [&'static str],
    /// An item sprite template (`generated`, `handheld`) — item models only.
    pub sprite: bool,
    /// The parent model defines `particle` itself.
    pub has_particle: bool,
}

pub const TEMPLATES: &[Template] = &[
    Template {
        name: "cube_all",
        parent: "minecraft:block/cube_all",
        slots: &["all"],
        sprite: false,
        has_particle: true,
    },
    Template {
        name: "cube_column",
        parent: "minecraft:block/cube_column",
        slots: &["end", "side"],
        sprite: false,
        has_particle: true,
    },
    Template {
        name: "cube_bottom_top",
        parent: "minecraft:block/cube_bottom_top",
        slots: &["top", "bottom", "side"],
        sprite: false,
        has_particle: true,
    },
    Template {
        name: "cube",
        parent: "minecraft:block/cube",
        slots: &["down", "up", "north", "south", "west", "east"],
        sprite: false,
        has_particle: false,
    },
    Template {
        name: "orientable",
        parent: "minecraft:block/orientable",
        slots: &["front", "side", "top"],
        sprite: false,
        has_particle: true,
    },
    Template {
        name: "cross",
        parent: "minecraft:block/cross",
        slots: &["cross"],
        sprite: false,
        has_particle: true,
    },
    Template {
        name: "generated",
        parent: "minecraft:item/generated",
        slots: &["layer0"],
        sprite: true,
        has_particle: true,
    },
    Template {
        name: "handheld",
        parent: "minecraft:item/handheld",
        slots: &["layer0"],
        sprite: true,
        has_particle: true,
    },
];

pub fn template(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.name == name)
}

/// `[a-z0-9_.-]+` — a Minecraft namespace.
pub fn valid_namespace(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-')
        })
}

/// `[a-z0-9_./-]+` — a Minecraft resource path.
pub fn valid_path(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('/')
        && !s.ends_with('/')
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-' | b'/')
        })
}

/// `ns:path` (or `path`, in `minecraft`) as a valid resource id.
pub fn valid_id(s: &str) -> bool {
    match s.split_once(':') {
        Some((ns, path)) => valid_namespace(ns) && valid_path(path),
        None => valid_path(s),
    }
}

// ── Finding ──────────────────────────────────────────────────────────────────

/// Every `content/**/*.yblock|*.yitem|*.yrecipe|*.ybook` of the mod at `root`, sorted.
pub fn find(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if Kind::of(&path).is_some() {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join(CONTENT_DIR), &mut out);
    out.sort();
    out
}

fn display(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

// ── Compiling ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct BlockProps {
    hardness: Option<f64>,
    resistance: Option<f64>,
    sound: Option<String>,
    light: u8,
    /// The tool tags it joins (`minecraft:mineable/pickaxe`, a mod's own).
    tools: Vec<String>,
    /// The tool level it needs (`minecraft:iron`, a mod's `.ydef` level) and
    /// the line that says so.
    tool_level: Option<(String, usize)>,
    requires_tool: bool,
    no_collision: bool,
    slipperiness: Option<f64>,
    /// `None` = full cube.
    shape: Option<[f64; 6]>,
    connect_groups: Vec<String>,
    connects: bool,
    /// The inventory screen it opens (a `.yui` `<Screen>` with `<Slot>`s).
    inventory: Option<String>,
    /// Gives off redstone power, set by the mod per position.
    redstone: bool,
}

#[derive(Debug, Clone, Default)]
struct ItemProps {
    stack_size: Option<u8>,
    durability: u32,
    fire_resistant: bool,
    fuel_ticks: u32,
    /// (nutrition, saturation, always_edible)
    food: Option<(u32, f64, bool)>,
}

/// One checked content file — what the Rust code needs.
#[derive(Debug, Clone)]
struct Entry {
    kind: Kind,
    id: String,
    file: String,
    auto: bool,
    block: BlockProps,
    item: ItemProps,
}

/// Per-file checking state.
struct Ctx<'a> {
    root: &'a Path,
    refs: &'a yres::Refs,
    file: String,
    ns: String,
    errors: &'a mut Vec<String>,
    warnings: &'a mut Vec<String>,
    /// Bundled texture path → its source file.
    textures: &'a mut BTreeMap<String, PathBuf>,
    /// `block.inventory` screen ids named so far: (file, line, id) —
    /// checked against the mod's screens once every file is read.
    inventories: &'a mut Vec<(String, usize, String)>,
    /// References a file makes (a block's drops): (file, line, path, ref) —
    /// checked once every file is read.
    deferred: &'a mut Vec<(String, usize, String, yog_data::Ref)>,
}

impl Ctx<'_> {
    fn err(&mut self, line: usize, msg: impl std::fmt::Display) {
        self.errors.push(format!("{}:{line}: {msg}", self.file));
    }

    fn warn(&mut self, line: usize, msg: impl std::fmt::Display) {
        self.warnings.push(format!("{}:{line}: {msg}", self.file));
    }

    /// A problem with the value at `path`.
    fn err_at(&mut self, loc: Loc, path: &At, msg: impl std::fmt::Display) {
        let line = loc(path);
        self.err(line, format!("{path}: {msg}"));
    }

    fn warn_at(&mut self, loc: Loc, path: &At, msg: impl std::fmt::Display) {
        let line = loc(path);
        self.warn(line, format!("{path}: {msg}"));
    }

    /// Reports what reading a file through its schema found.
    fn schema_errors(&mut self, errors: yog_data::Errors) {
        for e in errors.0 {
            let l = e.at.map_or(1, |a| a.line as usize);
            if e.path.is_root() {
                self.err(l, e.message)
            } else {
                self.err(l, format!("{}: {}", e.path, e.message))
            }
        }
    }

    /// A texture → the model's form: `#slot`, a `.yres` image's id, a
    /// `.png` of `resources/` (bundled), or a texture id (`ns:block/x`,
    /// `block/stone` = `minecraft:block/stone`).
    fn texture(&mut self, loc: Loc, path: &At, t: &files::Texture) -> Option<String> {
        let s = match t {
            files::Texture::Slot(slot) => return Some(format!("#{slot}")),
            files::Texture::Res(r) => {
                let name = format!("{}.{}", r.set, r.key);
                return match self.refs.images.get(&name) {
                    Some(id) => Some(id.clone()),
                    None => {
                        self.err_at(loc, path, format!("`@{name}`: no image `{name}` in resources/*.yres"));
                        None
                    }
                };
            }
            files::Texture::Plain(s) => s.trim().to_string(),
        };
        if let Some(stem) = s.strip_suffix(".png") {
            let rel = stem.trim_start_matches("./").trim_start_matches('/');
            let rel = rel.strip_prefix("resources/").unwrap_or(rel);
            if rel.split('/').any(|p| p == "..") || !valid_path(rel) {
                self.err_at(loc, path, format!("`{s}`: a path inside resources/ — lower case a-z, 0-9, `_`, `-`, `.`, `/`"));
                return None;
            }
            let file = self.root.join(yres::RESOURCES_DIR).join(format!("{rel}.png"));
            if !file.is_file() {
                self.err_at(loc, path, format!("resources/{rel}.png doesn't exist"));
                return None;
            }
            let target = format!("assets/{}/textures/{rel}.png", self.ns);
            if let Some(other) = self.textures.insert(target.clone(), file.clone())
                && other != file
            {
                let shown = display(self.root, &other);
                self.err_at(loc, path, format!("{target} also comes from {shown}"));
            }
            let meta = self.root.join(yres::RESOURCES_DIR).join(format!("{rel}.png.mcmeta"));
            if meta.is_file() {
                self.textures.insert(format!("{target}.mcmeta"), meta);
            }
            return Some(format!("{}:{rel}", self.ns));
        }
        if !valid_id(&s) {
            self.err_at(loc, path, format!("`{s}` isn't a texture — `#slot`, `@Set.image`, `path/in/resources.png` or a texture id like `minecraft:block/stone`"));
            return None;
        }
        let id = if s.contains(':') { s } else { format!("minecraft:{s}") };
        let (ns, tex) = id.split_once(':').unwrap_or_default();
        if ns == self.ns {
            let file = self.root.join("assets").join(ns).join("textures").join(format!("{tex}.png"));
            let bundled = self.textures.contains_key(&format!("assets/{ns}/textures/{tex}.png")) || self.refs.images.values().any(|v| *v == id);
            if !file.is_file() && !bundled {
                self.warn_at(loc, path, format!("texture {id} isn't in assets/{ns}/textures/ or resources/"));
            }
        }
        Some(id)
    }
}

/// Where a path of the file being read is: its line.
type Loc<'a> = &'a dyn Fn(&At) -> usize;

/// A checked model: Minecraft's JSON, its bounding box.
struct Model {
    json: Value,
    /// Elements' bounding box in pixels, when it has elements.
    bounds: Option<[f64; 6]>,
}

fn nums(ns: &[f64]) -> Value {
    Value::Array(ns.iter().map(|n| num_value(*n)).collect())
}

/// A model → Minecraft's model JSON. `item` = an item model (sprite templates allowed).
fn model(cx: &mut Ctx, loc: Loc, at: &At, m: &files::Model, item: bool) -> Option<Model> {
    let mut out = Map::new();
    let mut tpl: Option<&Template> = None;
    let mut raw_parent = false;
    if let Some(name) = &m.parent {
        if let Some(t) = template(name) {
            if t.sprite && !item {
                cx.err_at(loc, &at.key("parent"), format!("{name:?} is an item sprite — block models use cube_all, cube_column, ..."));
            }
            out.insert("parent".into(), Value::String(t.parent.into()));
            tpl = Some(t);
        } else if name.contains(':') && valid_id(name) {
            out.insert("parent".into(), Value::String(name.clone()));
            raw_parent = true;
        } else {
            let names: Vec<&str> = TEMPLATES.iter().filter(|t| item || !t.sprite).map(|t| t.name).collect();
            cx.err_at(loc, &at.key("parent"), format!("{name:?} isn't a template ({}) or a model id like `minecraft:block/stairs`", names.join(", ")));
            return None;
        }
    }

    // Textures.
    let mut slots: Vec<String> = Vec::new();
    let mut textures = Map::new();
    for (slot, tex) in &m.textures.0 {
        let here = at.key("textures").key(slot);
        if slot.is_empty() || slot.starts_with('#') {
            cx.err_at(loc, &here, "slot names are plain words (`all`, `side`, `0`)");
            continue;
        }
        if let Some(t) = cx.texture(loc, &here, tex) {
            textures.insert(slot.clone(), Value::String(t));
            slots.push(slot.clone());
        }
    }
    let defined = |slot: &str, textures: &Map<String, Value>| textures.contains_key(slot);
    if let Some(t) = tpl {
        if t.sprite {
            if !defined("layer0", &textures) {
                cx.err_at(loc, at, format!("parent {:?} needs textures.layer0", t.name));
            }
            for slot in &slots {
                let n = slot.strip_prefix("layer").and_then(|n| n.parse::<usize>().ok());
                match n {
                    Some(n) if n == 0 || defined(&format!("layer{}", n - 1), &textures) => {}
                    Some(_) => cx.err_at(loc, &at.key("textures").key(slot), "without the layer before it"),
                    None if slot == "particle" => {}
                    None => cx.warn_at(loc, &at.key("textures").key(slot), "isn't used by an item sprite (layer0, layer1, ...)"),
                }
            }
        } else {
            for slot in t.slots {
                if !defined(slot, &textures) {
                    cx.err_at(loc, at, format!("parent {:?} needs textures.{slot}", t.name));
                }
            }
        }
    }

    // Elements.
    let mut bounds: Option<[f64; 6]> = None;
    if let Some(els) = &m.elements {
        if let Some(t) = tpl {
            cx.err_at(loc, &at.key("elements"), format!("elements replace the shape of parent {:?} — remove one of them", t.name));
        }
        let mut elements = Vec::new();
        for (i, el) in els.iter().enumerate() {
            if let Some(json) = element(cx, loc, &at.key("elements").index(i), el, &textures, raw_parent) {
                let b = bounds.get_or_insert([16.0, 16.0, 16.0, 0.0, 0.0, 0.0]);
                for a in 0..3 {
                    b[a] = b[a].min(el.from[a].min(el.to[a]));
                    b[a + 3] = b[a + 3].max(el.from[a].max(el.to[a]));
                }
                elements.push(json);
            }
        }
        out.insert("elements".into(), Value::Array(elements));
    } else if tpl.is_none() && !raw_parent {
        cx.err_at(loc, at, "no parent and no elements — give a template (parent: \"cube_all\") or elements");
    }

    // `particle` — the break/run particles. Default: the first texture —
    // also under a raw parent when the model brings its own elements (a
    // parent like `minecraft:block/block` names no particle texture).
    if !textures.contains_key("particle")
        && !tpl.is_some_and(|t| t.has_particle)
        && (!raw_parent || m.elements.is_some())
        && let Some(first) = slots.first()
    {
        let v = textures[first].clone();
        textures.insert("particle".into(), v);
    }
    if !textures.is_empty() {
        out.insert("textures".into(), Value::Object(textures));
    }
    if let Some(d) = &m.display {
        let mut display = Map::new();
        for (place, t) in d.iter() {
            let mut o = Map::new();
            for (key, v) in [("rotation", t.rotation), ("translation", t.translation), ("scale", t.scale)] {
                if let Some(v) = v {
                    o.insert(key.into(), nums(&v));
                }
            }
            display.insert(place.into(), Value::Object(o));
        }
        out.insert("display".into(), Value::Object(display));
    }
    if let Some(b) = m.ambient_occlusion {
        out.insert("ambientocclusion".into(), Value::Bool(b));
    }
    if let Some(g) = &m.gui_light {
        out.insert("gui_light".into(), Value::String(files::word(g)));
    }
    Some(Model { json: Value::Object(out), bounds })
}

/// One element → its JSON.
fn element(cx: &mut Ctx, loc: Loc, at: &At, el: &files::Element, textures: &Map<String, Value>, raw_parent: bool) -> Option<Value> {
    let mut out = Map::new();
    out.insert("from".into(), nums(&el.from));
    out.insert("to".into(), nums(&el.to));
    if let Some(r) = &el.rotation {
        if (r.angle / 22.5).fract() != 0.0 {
            cx.err_at(loc, &at.key("rotation").key("angle"), format!("{} — Minecraft rotates elements by -45, -22.5, 0, 22.5 or 45 degrees", r.angle));
        }
        let mut rot = Map::new();
        rot.insert("origin".into(), nums(&r.origin.unwrap_or([8.0, 8.0, 8.0])));
        rot.insert("axis".into(), Value::String(files::word(&r.axis)));
        rot.insert("angle".into(), num_value(r.angle));
        if r.rescale {
            rot.insert("rescale".into(), Value::Bool(true));
        }
        out.insert("rotation".into(), Value::Object(rot));
    }
    if el.shade == Some(false) {
        out.insert("shade".into(), Value::Bool(false));
    }
    let mut faces = Map::new();
    match &el.faces {
        None => cx.warn_at(loc, at, "no faces — it's invisible"),
        Some(f) => {
            for (dir, face) in f.iter() {
                let here = at.key("faces").key(dir);
                let mut fo = Map::new();
                let slot = face.texture.strip_prefix('#').unwrap_or(&face.texture);
                if !textures.contains_key(slot) && !raw_parent {
                    cx.err_at(loc, &here.key("texture"), format!("`#{slot}`: no such slot in textures"));
                }
                fo.insert("texture".into(), Value::String(format!("#{slot}")));
                if let Some(uv) = face.uv {
                    fo.insert("uv".into(), nums(&uv));
                }
                match face.rotation {
                    None | Some(0) => {}
                    Some(n @ (90 | 180 | 270)) => {
                        fo.insert("rotation".into(), Value::from(n));
                    }
                    Some(_) => cx.err_at(loc, &here.key("rotation"), "must be 0, 90, 180 or 270"),
                }
                if let Some(c) = &face.cullface {
                    fo.insert("cullface".into(), Value::String(files::word(c)));
                }
                if let Some(t) = face.tint
                    && t >= 0
                {
                    fo.insert("tintindex".into(), Value::from(t));
                }
                faces.insert(dir.into(), Value::Object(fo));
            }
        }
    }
    out.insert("faces".into(), Value::Object(faces));
    Some(Value::Object(out))
}

/// A file's id: as written, else the file's name; the mod's namespace by default.
fn file_id(cx: &mut Ctx, line: usize, id: Option<&str>, path: &Path, mod_ns: Option<&str>) -> Option<(String, String)> {
    let raw = id.map(str::to_string).unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    let (ns, p) = match raw.split_once(':') {
        Some((ns, p)) => (ns.to_string(), p.to_string()),
        None => match mod_ns {
            Some(ns) => (ns.to_string(), raw.clone()),
            None => {
                cx.err(line, "the mod id isn't a valid Minecraft namespace (a-z, 0-9, `_`, `-`, `.`), so give the namespace: id: \"mynamespace:name\"");
                return None;
            }
        },
    };
    if !valid_namespace(&ns) || !valid_path(&p) {
        cx.err(line, format!("id `{ns}:{p}` isn't a valid Minecraft id — lower case a-z, 0-9, `_`, `-`, `.` (and `/` in the path)"));
        return None;
    }
    Some((ns, p))
}

/// The JSON of one block state: `{"model": ..., "x": .., "y": .., ...}`.
fn variant(model: &str, x: f64, y: f64, uvlock: bool, weight: Option<f64>) -> Value {
    let mut v = Map::new();
    v.insert("model".into(), Value::String(model.into()));
    if x != 0.0 {
        v.insert("x".into(), num_value(x));
    }
    if y != 0.0 {
        v.insert("y".into(), num_value(y));
    }
    if uvlock {
        v.insert("uvlock".into(), Value::Bool(true));
    }
    if let Some(w) = weight {
        v.insert("weight".into(), num_value(w));
    }
    Value::Object(v)
}

fn pretty(v: &Value) -> Vec<u8> {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s.into_bytes()
}

/// Checks one `.yitem` or `.yblock` (`content::files`) and writes what the
/// game reads: models, block states, item definitions, loot, lang, tags.
fn compile_file(
    cx: &mut Ctx,
    text: &str,
    kind: Kind,
    path: &Path,
    mod_ns: Option<&str>,
    out: &mut Compiled,
    generated: &mut BTreeMap<String, String>,
) -> Option<Entry> {
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let loc_fn = |p: &At| doc.locate(text, p).line as usize;
    let loc: Loc = &loc_fn;
    let root = At::root();
    struct Parts {
        format: Option<u32>,
        id: Option<String>,
        name: Option<yog_data::Text>,
        tooltip: Option<yog_data::Text>,
        register: Option<files::Register>,
        model: files::Model,
        item_model: Option<files::Model>,
        block: Option<files::BlockProps>,
        states: Option<files::States>,
        item: Option<files::ItemProps>,
    }
    let (parts, refs) = match kind {
        Kind::Item => match yog_data::schema::from_doc::<files::ItemFile>(text, &doc) {
            Ok(r) => {
                let f = r.value;
                (Parts { format: f.format, id: f.id, name: f.name, tooltip: f.tooltip, register: f.register, model: f.model, item_model: None, block: None, states: None, item: f.item }, r.refs)
            }
            Err(e) => {
                cx.schema_errors(e);
                return None;
            }
        },
        Kind::Block => match yog_data::schema::from_doc::<files::BlockFile>(text, &doc) {
            Ok(r) => {
                let f = r.value;
                (Parts { format: f.format, id: f.id, name: f.name, tooltip: f.tooltip, register: f.register, model: f.model, item_model: f.item_model, block: f.block, states: f.states, item: None }, r.refs)
            }
            Err(e) => {
                cx.schema_errors(e);
                return None;
            }
        },
        Kind::Recipe | Kind::Book | Kind::Layout | Kind::Def => unreachable!("only blocks and items get here"),
    };
    // What it names (drops) is checked once every file is read.
    for f in &refs {
        cx.deferred.push((cx.file.clone(), loc(&f.path), f.path.to_string(), f.value.clone()));
    }
    if let Some(f) = parts.format
        && f != 1
    {
        cx.err_at(loc, &root.key("format"), "format must be 1 (this yog understands format 1)");
    }
    let (ns, p) = file_id(cx, loc(&root.key("id")), parts.id.as_deref(), path, mod_ns)?;
    cx.ns = ns.clone();
    let id = format!("{ns}:{p}");
    let auto = parts.register.unwrap_or_default() == files::Register::Auto;

    // Name and tooltip → lang, under the keys the game looks up.
    let key = format!("{}.{ns}.{}", kind.word(), p.replace('/', "."));
    for (field, text, target) in [("name", &parts.name, key.clone()), ("tooltip", &parts.tooltip, format!("{key}.desc"))] {
        let Some(t) = text else { continue };
        let from = match t {
            yog_data::Text::Literal(s) => {
                out.lang.entry((ns.clone(), yres::NEUTRAL_LOCALE.to_string())).or_default().insert(target, s.clone());
                continue;
            }
            yog_data::Text::Lang(k) => k.clone(),
            yog_data::Text::NameOf(r) => {
                let (rns, rp) = r.id.split_once(':').unwrap_or(("minecraft", &r.id));
                format!("{}.{rns}.{}", r.kind, rp.replace('/', "."))
            }
            yog_data::Text::Res(r) => match cx.refs.texts.get(&format!("{}.{}", r.set, r.key)) {
                Some((k, _)) => k.clone(),
                None => {
                    cx.err_at(loc, &root.key(field), format!("`@{}.{}`: no text `{}.{}` in resources/*.yres", r.set, r.key, r.set, r.key));
                    continue;
                }
            },
        };
        if from != target {
            out.lang_aliases.push((from, target));
        }
    }

    let mut emit = |cx: &mut Ctx, rel: String, value: &Value| {
        let full = format!("assets/{ns}/{rel}");
        if cx.root.join(&full).is_file() {
            cx.err(1, format!("{full} is written by this file — delete the hand-written one (or bring it in with `yog import model`)"));
        }
        if let Some(other) = generated.insert(full.clone(), cx.file.clone()) {
            cx.err(1, format!("{full} is also written by {other}"));
        }
        out.outputs.push((full, pretty(value)));
    };

    let mut entry = Entry { kind, id: id.clone(), file: cx.file.clone(), auto, block: BlockProps::default(), item: ItemProps::default() };

    if kind == Kind::Item {
        if let Some(it) = &parts.item {
            let props = &mut entry.item;
            props.stack_size = it.stack_size;
            props.durability = it.durability.unwrap_or(0);
            props.fire_resistant = it.fire_resistant;
            props.fuel_ticks = it.fuel_ticks.unwrap_or(0);
            props.food = it.food.as_ref().map(|f| (f.nutrition, f.saturation, f.always_edible));
            if props.durability > 0 && props.stack_size.is_some_and(|s| s > 1) {
                cx.warn_at(loc, &root.key("item").key("stack_size"), "ignored: an item with durability stacks to 1");
            }
        }
        let m = model(cx, loc, &root.key("model"), &parts.model, true)?;
        emit(cx, format!("models/item/{p}.json"), &m.json);
        emit(cx, format!("items/{p}.json"), &json!({ "model": { "type": "minecraft:model", "model": format!("{ns}:item/{p}") } }));
        return Some(entry);
    }

    // A block.
    let b = parts.block.clone().unwrap_or_default();
    let bat = root.key("block");
    let props = &mut entry.block;
    props.hardness = b.hardness;
    props.resistance = b.resistance;
    props.sound = b.sound.as_ref().map(files::word);
    props.light = b.light.unwrap_or(0);
    props.tools = b.tool.iter().map(|t| t.0.clone()).collect();
    props.tool_level = b.tool_level.clone().map(|l| (l, loc(&bat.key("tool_level"))));
    props.requires_tool = b.requires_tool;
    props.no_collision = b.collision == Some(false);
    props.slipperiness = b.slipperiness;
    props.redstone = b.redstone;
    props.connect_groups = b.connect_groups.clone();
    let shape_auto = matches!(b.shape, None | Some(files::BlockShape::Auto));
    if let Some(files::BlockShape::Box(bx)) = b.shape {
        props.shape = Some(bx);
    }
    if let Some(inv) = &b.inventory {
        if valid_id(inv) && inv.contains(':') {
            props.inventory = Some(inv.clone());
            let at = (cx.file.clone(), loc(&bat.key("inventory")), inv.clone());
            cx.inventories.push(at);
        } else {
            cx.err_at(loc, &bat.key("inventory"), format!("`{inv}`: the id of an inventory screen — a .yui <Screen id=\"mymod:name\"> with <Slot>s"));
        }
    }
    if props.tool_level.is_some() && props.tools.is_empty() {
        cx.warn_at(loc, &bat, "tool_level without tool — any tool of that level mines it");
    }
    let drops = match &b.drops {
        None | Some(files::Drops::Itself) => drops_table(&id, None),
        Some(files::Drops::Nothing) => Value::Null,
        Some(files::Drops::Item(d)) => drops_table(d.item.as_deref().unwrap_or(&id), d.count),
    };
    let render_layer = b.render_layer.as_ref().map(files::word);

    let mut m = model(cx, loc, &root.key("model"), &parts.model, false)?;
    if let (Some(layer), Value::Object(o)) = (&render_layer, &mut m.json) {
        // Forge/NeoForge read it from the model; Fabric ignores it.
        o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
    }
    if shape_auto {
        entry.block.shape = m
            .bounds
            .filter(|b| *b != [0.0, 0.0, 0.0, 16.0, 16.0, 16.0])
            .map(|b| [b[0].max(0.0), b[1].max(0.0), b[2].max(0.0), b[3].min(16.0), b[4].min(16.0), b[5].min(16.0)]);
    }
    let main = format!("{ns}:block/{p}");
    emit(cx, format!("models/block/{p}.json"), &m.json);

    // Block states.
    let mut rotations: Vec<Value> = Vec::new();
    let mut arm: Option<(String, bool)> = None;
    if let Some(s) = &parts.states {
        let sat = root.key("states");
        if let Some(rs) = &s.rotations {
            if rs.is_empty() {
                cx.err_at(loc, &sat.key("rotations"), "a non-empty list of { x: 0, y: 90, weight: 1, uvlock: false }");
            }
            for (i, r) in rs.iter().enumerate() {
                let here = sat.key("rotations").index(i);
                let mut angle = |k: &str, v: Option<u32>| match v {
                    None => 0.0,
                    Some(n @ (0 | 90 | 180 | 270)) => n as f64,
                    Some(_) => {
                        cx.err_at(loc, &here.key(k), "must be 0, 90, 180 or 270");
                        0.0
                    }
                };
                let (x, y) = (angle("x", r.x), angle("y", r.y));
                rotations.push(variant(&main, x, y, r.uvlock, r.weight));
            }
        }
        if let Some(c) = &s.connect {
            if let Some(am) = model(cx, loc, &sat.key("connect").key("arm"), &c.arm, false) {
                let mut json = am.json;
                if let (Some(layer), Value::Object(o)) = (&render_layer, &mut json) {
                    o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
                }
                emit(cx, format!("models/block/{p}_arm.json"), &json);
                arm = Some((format!("{ns}:block/{p}_arm"), c.uvlock));
            }
            entry.block.connects = true;
            if entry.block.connect_groups.is_empty() {
                entry.block.connect_groups.push(id.clone());
            }
        }
    }
    let base: Value = match rotations.len() {
        0 => variant(&main, 0.0, 0.0, false, None),
        1 => rotations.remove(0),
        _ => Value::Array(rotations),
    };
    let blockstate = match &arm {
        None => json!({ "variants": { "": base } }),
        Some((arm, uvlock)) => {
            let mut parts = vec![json!({ "apply": base })];
            for (dir, x, y) in [("north", 0.0, 0.0), ("east", 0.0, 90.0), ("south", 0.0, 180.0), ("west", 0.0, 270.0), ("up", 270.0, 0.0), ("down", 90.0, 0.0)] {
                parts.push(json!({ "when": { dir: "true" }, "apply": variant(arm, x, y, *uvlock, None) }));
            }
            json!({ "multipart": parts })
        }
    };
    emit(cx, format!("blockstates/{p}.json"), &blockstate);

    // The item form: the block model, or its own.
    let item_model = match &parts.item_model {
        None => Some(json!({ "parent": main })),
        Some(im) => model(cx, loc, &root.key("item_model"), im, true).map(|m| m.json),
    };
    if let Some(im) = item_model {
        emit(cx, format!("models/item/{p}.json"), &im);
    }
    emit(cx, format!("items/{p}.json"), &json!({ "model": { "type": "minecraft:model", "model": format!("{ns}:item/{p}") } }));

    if !drops.is_null() {
        let full = format!("data/{ns}/loot_table/blocks/{p}.json");
        let plural = format!("data/{ns}/loot_tables/blocks/{p}.json");
        if cx.root.join(&full).is_file() || cx.root.join(&plural).is_file() {
            cx.err(1, format!("{full} is written by this file (block.drops) — delete the hand-written one, or set drops: \"none\""));
        }
        out.outputs.push((full, pretty(&drops)));
    }
    // Tools and tiers are block tags: the block joins each.
    // (The tool level's tags come once every `.ydef` is read: `tool_level_tags`.)
    for tag in &entry.block.tools {
        let (tns, tpath) = tag.split_once(':').unwrap_or(("minecraft", tag));
        out.tags.entry(format!("data/{tns}/tags/block/{tpath}.json")).or_default().insert(id.clone());
    }
    Some(entry)
}

/// The loot table dropping `item` (`count` of it), when it survives the explosion.
fn drops_table(item: &str, count: Option<files::DropCount>) -> Value {
    let mut entry = json!({ "type": "minecraft:item", "name": item });
    if let Some(c) = count {
        let count = match c {
            files::DropCount::Exactly(n) => num_value(n),
            files::DropCount::Between(min, max) => json!({ "type": "minecraft:uniform", "min": num_value(min), "max": num_value(max) }),
        };
        entry["functions"] = json!([{ "function": "minecraft:set_count", "count": count }]);
    }
    json!({
        "type": "minecraft:block",
        "pools": [{
            "rolls": 1,
            "entries": [entry],
            "conditions": [{ "condition": "minecraft:survives_explosion" }]
        }]
    })
}

// ── Recipes ──────────────────────────────────────────────────────────────────

/// What a recipe takes.
#[derive(Debug, Clone)]
enum RecipeKind {
    /// A crafting grid pattern: rows of key characters (space = empty);
    /// each key any of several items.
    Shaped { pattern: Vec<String>, key: Vec<(char, Vec<String>)> },
    /// Crafting ingredients in any arrangement.
    Shapeless(Vec<Vec<String>>),
    /// Cooking (`method`: `Smelting`, `Blasting`, `Smoking`, `Campfire`).
    Cooking { method: &'static str, input: Vec<String>, experience: f64, cook_time: u32 },
    Stonecutting(Vec<String>),
    Smithing { template: Vec<String>, base: Vec<String>, addition: Vec<String> },
    /// The mod's own kind of recipe (`mymod:infusion`), for its machines
    /// and books — not the game's: named slots, each a list of ingredients
    /// (an ingredient any of several items), and values (`time`, `energy`…).
    Custom { kind: String, slots: Vec<(String, Vec<Vec<String>>)>, values: serde_json::Map<String, Value> },
}

/// One checked `.ybook`.
struct BookFile {
    id: String,
    /// The book as the runtime reads it, references resolved.
    json: String,
}

/// Checks one `.ybook` (`yog_book::Book`'s schema), whose references
/// (`@item:minecraft:stick`, see `yog_book::refs`) must name what exists
/// — in this mod, on every Minecraft version, or among the other mods'
/// ids `yog.toml` declares — and come out as plain ids. Texts may be
/// `@<Set>.<name>` (a `.yres` text), `@item:<id>` / `@block:<id>` /
/// `@entity:<id>` (its name) or `@@…` (a literal `@`); a book with text
/// references is shown in the player's language. An `id` without a
/// namespace gets the mod's.
fn compile_book(
    cx: &mut Ctx,
    text: &str,
    mod_ns: Option<&str>,
    known: &refs::Known,
    layouts: &[yog_book::layout::RecipeLayout],
    recipe_kinds: &HashMap<String, String>,
) -> Option<BookFile> {
    use refs::RefKind;
    use yog_data::schema::{Resolve, Text};
    use yog_data::{Path, Ref, Res};

    struct Check<'a> {
        known: &'a refs::Known,
        texts: &'a HashMap<String, (String, String)>,
        entries: HashSet<String>,
        categories: HashSet<String>,
        text_refs: bool,
    }

    impl Resolve for Check<'_> {
        fn reference(&mut self, r: &Ref, _: &Path) -> Result<String, String> {
            let own = match r.kind.as_str() {
                "entry" => Some(&self.entries),
                "category" => Some(&self.categories),
                _ => None,
            };
            match (own, RefKind::from_word(&r.kind)) {
                (Some(set), _) if set.contains(&r.id) => Ok(r.id.clone()),
                (Some(_), _) => Err(format!("no {} {} in this book", r.kind, r.id)),
                (None, Some(kind)) => self.known.check(kind, &r.id).map(|()| r.id.clone()),
                (None, None) => Err(format!("`@{}:` names nothing a book shows", r.kind)),
            }
        }

        fn text(&mut self, t: &Text, _: &Path) -> Result<String, String> {
            match t {
                Text::Literal(s) => Ok(s.clone()),
                // A name: the game's translation of the item, block or entity.
                Text::NameOf(r) => {
                    if !matches!(r.kind.as_str(), "item" | "block" | "entity") {
                        return Err(format!("`@{}:{}` has no name to show", r.kind, r.id));
                    }
                    self.known.check(RefKind::from_word(&r.kind).unwrap(), &r.id)?;
                    let (ns, p) = yog_book::refs::bare(&r.id).split_once(':').unwrap_or(("minecraft", &r.id));
                    self.text_refs = true;
                    Ok(format!("{}.{ns}.{}", r.kind, p.replace('/', ".")))
                }
                // Any translation key, the game's or a mod's.
                Text::Lang(key) => {
                    self.text_refs = true;
                    Ok(key.clone())
                }
                Text::Res(r) => match self.texts.get(&format!("{}.{}", r.set, r.key)) {
                    Some((key, _)) => {
                        self.text_refs = true;
                        Ok(key.clone())
                    }
                    None => Err(format!("`@{}.{}`: no text `{}.{}` in resources/*.yres", r.set, r.key, r.set, r.key)),
                },
            }
        }

        fn words(&mut self, r: &Res) -> Option<String> {
            self.texts.get(&format!("{}.{}", r.set, r.key)).map(|(_, t)| t.clone())
        }
    }

    // Entries and categories may name each other in any order: their ids first.
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let ids = |key: &str| -> HashSet<String> {
        doc.value.get(key).and_then(|v| v.as_list()).unwrap_or_default().iter().filter_map(|e| e.get("id")?.as_str().map(str::to_string)).collect()
    };
    let mut check = Check { known, texts: &cx.refs.texts, entries: ids("entries"), categories: ids("categories"), text_refs: false };
    let mut book = match yog_data::from_doc_with::<yog_book::Book>(text, &doc, &mut check) {
        Ok(read) => read.value,
        Err(errors) => {
            for e in errors.0 {
                let line = e.at.map_or(1, |a| a.line as usize);
                if e.path.is_root() {
                    cx.err(line, e.message);
                } else {
                    cx.err(line, format!("{}: {}", e.path, e.message));
                }
            }
            return None;
        }
    };
    if !book.id.contains(':') {
        book.id = format!("{}:{}", mod_ns.unwrap_or("minecraft"), book.id);
    }
    // References to entries and categories always have a namespace, so their ids must too.
    let ns = mod_ns.unwrap_or("minecraft");
    let unnamespaced: Vec<String> = (book.categories.iter().enumerate().map(|(i, c)| (format!("categories[{i}].id"), &c.id)))
        .chain(book.entries.iter().enumerate().map(|(i, e)| (format!("entries[{i}].id"), &e.id)))
        .filter(|(_, id)| !id.contains(':'))
        .map(|(path, id)| format!("{path}: `{id}`: write its namespace (`{ns}:{id}`)"))
        .collect();
    if !unnamespaced.is_empty() {
        for e in unnamespaced {
            cx.err(1, e);
        }
        return None;
    }
    if !valid_id(&book.id) {
        cx.err(1, format!("book id `{}`: not a valid id", book.id));
        return None;
    }
    book.i18n |= check.text_refs;
    // Every recipe a page shows needs a layout for its kind (the game's are built in).
    let mut ok = true;
    for (e, entry) in book.entries.iter().enumerate() {
        for (p, page) in entry.pages.iter().enumerate() {
            if let yog_book::BookPage::Recipe { recipe, .. } = page
                && let Some(kind) = recipe_kinds.get(recipe)
                && yog_book::layout::find(layouts, kind).is_none()
            {
                let path = yog_data::Path::root().key("entries").index(e).key("pages").index(p).key("recipe");
                let file = kind.split_once(':').map_or(kind.as_str(), |(_, k)| k);
                cx.err(
                    doc.locate(text, &path).line as usize,
                    format!("{path}: no layout for recipe kind {kind} — add content/{file}.ylayout (see yog_book::layout)"),
                );
                ok = false;
            }
        }
    }
    if !ok {
        return None;
    }
    book.layouts = layouts.to_vec();
    Some(BookFile { id: book.id.clone(), json: book.to_json() })
}

/// Checks one `.ylayout` (`yog_book::layout::RecipeLayout`): the items it
/// draws exist, the kinds it lays out are the game's or the mod's own.
fn compile_layout(cx: &mut Ctx, text: &str, mod_ns: Option<&str>, known: &refs::Known) -> Option<yog_book::layout::RecipeLayout> {
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let read = match yog_data::schema::from_doc::<yog_book::layout::RecipeLayout>(text, &doc) {
        Ok(r) => r,
        Err(errors) => {
            for e in errors.0 {
                let l = e.at.map_or(1, |a| a.line as usize);
                if e.path.is_root() { cx.err(l, e.message) } else { cx.err(l, format!("{}: {}", e.path, e.message)) }
            }
            return None;
        }
    };
    let mut ok = true;
    for f in &read.refs {
        if let Some(kind) = refs::RefKind::from_word(&f.value.kind)
            && let Err(e) = known.check(kind, &f.value.id)
        {
            cx.err(doc.locate(text, &f.path).line as usize, format!("{}: {e}", f.path));
            ok = false;
        }
    }
    let l = read.value;
    for (i, k) in l.kinds.iter().enumerate() {
        let game = recipe::TYPES.contains(&k.as_str());
        let own = mod_ns.is_some_and(|m| k.split_once(':').is_some_and(|(n, p)| n == m && valid_path(p)));
        if !game && !own {
            let path = yog_data::Path::root().key("kinds").index(i);
            cx.err(doc.locate(text, &path).line as usize, format!("{path}: `{k}`: one of {}, or the mod's own kind `{}:<kind>`", recipe::TYPES.join(", "), mod_ns.unwrap_or("mymod")));
            ok = false;
        }
    }
    if l.kinds.is_empty() {
        cx.err(1, "kinds: the recipe kinds it lays out, at least one");
        ok = false;
    }
    ok.then_some(l)
}

/// One checked `.yrecipe`.
#[derive(Debug, Clone)]
struct Recipe {
    id: String,
    /// Its `type`: `shaped`, `smelting`, …, or the mod's own kind.
    ty: String,
    kind: RecipeKind,
    result: String,
    count: u32,
    group: Option<String>,
    category: Option<String>,
}

impl Recipe {
    /// The `registry.add_recipe(...)` call registering it.
    fn registration(&self) -> String {
        let list = |items: &[String]| {
            let quoted: Vec<String> = items.iter().map(|i| format!("{i:?}.into()")).collect();
            format!("vec![{}]", quoted.join(", "))
        };
        let kind = match &self.kind {
            RecipeKind::Shaped { pattern, key } => {
                let rows: Vec<String> = pattern.iter().map(|r| format!("{r:?}.into()")).collect();
                let keys: Vec<String> = key.iter().map(|(c, items)| format!("({c:?}, {})", list(items))).collect();
                format!("Shaped {{ pattern: vec![{}], key: vec![{}] }}", rows.join(", "), keys.join(", "))
            }
            RecipeKind::Shapeless(ingredients) => {
                let all: Vec<String> = ingredients.iter().map(|i| list(i)).collect();
                format!("Shapeless {{ ingredients: vec![{}] }}", all.join(", "))
            }
            RecipeKind::Cooking { method, input, experience, cook_time } => format!(
                "Cooking {{ method: ::yog_api::Cooking::{method}, ingredient: {}, experience: {}_f32, cook_time: {cook_time} }}",
                list(input),
                float(*experience)
            ),
            RecipeKind::Stonecutting(input) => format!("Stonecutting {{ ingredient: {} }}", list(input)),
            RecipeKind::Custom { kind, slots, values } => {
                // Yog's recipe JSON as is: the host keeps it out of the game's data.
                let mut o = Map::new();
                o.insert("type".into(), kind.clone().into());
                o.insert("slots".into(), Value::Object(slots.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
                if !values.is_empty() {
                    o.insert("values".into(), Value::Object(values.clone()));
                }
                o.insert("result".into(), self.result.clone().into());
                if self.count != 1 {
                    o.insert("count".into(), self.count.into());
                }
                if let Some(g) = &self.group {
                    o.insert("group".into(), g.clone().into());
                }
                return format!("registry.add_recipe_json({:?}, {:?});", self.id, Value::Object(o).to_string());
            }
            RecipeKind::Smithing { template, base, addition } => format!(
                "Smithing {{ template: {}, base: {}, addition: {} }}",
                list(template),
                list(base),
                list(addition)
            ),
        };
        let opt = |o: &Option<String>| o.as_ref().map_or("None".to_string(), |v| format!("Some({v:?}.into())"));
        format!(
            "registry.add_recipe(::yog_api::Recipe {{ id: {:?}.into(), kind: ::yog_api::RecipeKind::{kind}, result: {:?}.into(), count: {}, group: {}, category: {} }});",
            self.id,
            self.result,
            self.count,
            opt(&self.group),
            opt(&self.category)
        )
    }
}

/// Checks one `.yrecipe` (see `content::recipe`): its schema, then what it
/// names — the mod's content, every Minecraft version's, other mods'
/// declared ids (`known`).
fn compile_recipe(cx: &mut Ctx, text: &str, path: &Path, mod_ns: Option<&str>, known: &refs::Known) -> Option<Recipe> {
    use recipe::{Body, RecipeFile};
    use yog_data::Path as At;

    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let line = |p: &At| doc.locate(text, p).line as usize;
    let read = match yog_data::schema::from_doc::<RecipeFile>(text, &doc) {
        Ok(r) => r,
        Err(errors) => {
            for e in errors.0 {
                let l = e.at.map_or(1, |a| a.line as usize);
                if e.path.is_root() { cx.err(l, e.message) } else { cx.err(l, format!("{}: {}", e.path, e.message)) }
            }
            return None;
        }
    };
    let mut ok = true;
    for f in &read.refs {
        if let Some(kind) = refs::RefKind::from_word(&f.value.kind)
            && let Err(e) = known.check(kind, &f.value.id)
        {
            cx.err(line(&f.path), format!("{}: {e}", f.path));
            ok = false;
        }
    }
    let RecipeFile { common: c, body } = read.value;
    let ty = match &body {
        Body::Shaped(_) => "shaped".to_string(),
        Body::Shapeless(_) => "shapeless".to_string(),
        Body::Cooking(t, _) | Body::Custom(t, _) => t.clone(),
        Body::Stonecutting(_) => "stonecutting".to_string(),
        Body::Smithing(_) => "smithing".to_string(),
    };
    let root = At::root();
    if let Some(f) = c.format
        && f != 1
    {
        cx.err(line(&root.key("format")), "format must be 1 (this yog understands format 1)");
        ok = false;
    }
    // Its id: as written, else the file's name; the mod's namespace by default.
    let raw = c.id.clone().unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    let id_line = line(&root.key("id"));
    let (ns, p) = match raw.split_once(':') {
        Some((ns, p)) => (ns.to_string(), p.to_string()),
        None => match mod_ns {
            Some(ns) => (ns.to_string(), raw.clone()),
            None => {
                cx.err(id_line, "the mod id isn't a valid Minecraft namespace (a-z, 0-9, `_`, `-`, `.`), so give the namespace: \"id\": \"mynamespace:name\"");
                return None;
            }
        },
    };
    if !valid_namespace(&ns) || !valid_path(&p) {
        cx.err(id_line, format!("id `{ns}:{p}` isn't a valid Minecraft id — lower case a-z, 0-9, `_`, `-`, `.` (and `/` in the path)"));
        return None;
    }
    if !(1..=64).contains(&c.count) {
        cx.err(line(&root.key("count")), format!("count: {} is out of range (1..=64)", c.count));
        ok = false;
    }
    let kind = match body {
        Body::Shaped(s) => {
            let width = s.pattern.first().map_or(0, |r| r.chars().count());
            if s.pattern.is_empty() || s.pattern.len() > 3 || width == 0 || width > 3 || s.pattern.iter().any(|r| r.chars().count() != width) {
                cx.err(line(&root.key("pattern")), "pattern must be 1–3 rows of the same 1–3 characters each");
                return None;
            }
            let mut key = Vec::new();
            for (k, ing) in &s.key.0 {
                let at = line(&root.key("key").key(k));
                let mut chars = k.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) if ch != ' ' => {
                        if !s.pattern.iter().any(|r| r.contains(ch)) {
                            cx.warn(at, format!("key {k:?} isn't used by the pattern"));
                        }
                        key.push((ch, ing.ids()));
                    }
                    _ => {
                        cx.err(at, format!("key {k:?}: one character other than space"));
                        ok = false;
                    }
                }
            }
            for ch in s.pattern.iter().flat_map(|r| r.chars()).filter(|c| *c != ' ') {
                if !s.key.0.iter().any(|(k, _)| k.chars().eq([ch])) {
                    cx.err(line(&root.key("pattern")), format!("pattern uses {ch:?}, which key doesn't define"));
                    return None;
                }
            }
            RecipeKind::Shaped { pattern: s.pattern, key }
        }
        Body::Shapeless(s) => {
            if s.ingredients.is_empty() || s.ingredients.len() > 9 {
                cx.err(line(&root.key("ingredients")), "ingredients must be 1–9 items or tags");
                return None;
            }
            RecipeKind::Shapeless(s.ingredients.iter().map(|i| i.ids()).collect())
        }
        Body::Cooking(ty, k) => {
            let (method, default_time) = match ty.as_str() {
                "smelting" => ("Smelting", 200),
                "blasting" => ("Blasting", 100),
                "smoking" => ("Smoking", 100),
                _ => ("Campfire", 600),
            };
            if !(0.0..=1000.0).contains(&k.experience) {
                cx.err(line(&root.key("experience")), "experience: 0–1000");
                ok = false;
            }
            let cook_time = k.cook_time.unwrap_or(default_time);
            if !(1..=32767).contains(&cook_time) {
                cx.err(line(&root.key("cook_time")), "cook_time: 1–32767 ticks");
                ok = false;
            }
            RecipeKind::Cooking { method, input: k.ingredient.ids(), experience: k.experience, cook_time }
        }
        Body::Stonecutting(k) => RecipeKind::Stonecutting(k.ingredient.ids()),
        Body::Smithing(k) => RecipeKind::Smithing { template: k.template.ids(), base: k.base.ids(), addition: k.addition.ids() },
        Body::Custom(kind, k) => {
            // The mod's own kind of recipe: `<mod id>:<kind>`.
            let own = mod_ns.is_some_and(|m| kind.split_once(':').is_some_and(|(n, p)| n == m && valid_path(p)));
            if !own {
                cx.err(
                    line(&root.key("type")),
                    format!("type `{kind}`: one of {}, or the mod's own kind `{}:<kind>`", recipe::TYPES.join(", "), mod_ns.unwrap_or("mymod")),
                );
                return None;
            }
            if c.category.is_some() {
                cx.err(line(&root.key("category")), format!("category is for the game's recipe types; {kind} takes \"slots\" and \"values\""));
                ok = false;
            }
            let slots = k.slots.0.iter().map(|(name, ings)| (name.clone(), ings.iter().map(|i| i.ids()).collect())).collect();
            let values = k.values.0.iter().map(|(name, v)| (name.clone(), serde_json::from_str(&yog_data::text::to_string(v)).unwrap_or(Value::Null))).collect();
            RecipeKind::Custom { kind, slots, values }
        }
    };
    ok.then(|| Recipe {
        id: format!("{ns}:{p}"),
        ty,
        kind,
        result: c.result.id,
        count: c.count,
        group: c.group,
        category: c.category,
    })
}

/// Every ladder, its levels bottom to top: the game's tool ladder and the mod's `.ydef`s.
struct Ladders {
    order: BTreeMap<String, Vec<String>>,
}

impl Ladders {
    /// A level's ladder and place on it.
    fn level(&self, id: &str) -> Option<(&str, usize)> {
        self.order.iter().find_map(|(l, levels)| levels.iter().position(|x| x == id).map(|i| (l.as_str(), i)))
    }
}

/// Reads the `.ydef`s: ladders, and levels placed on them (`above`, in any order of files).
fn ladders(defs: &[(String, String)], mod_ns: Option<&str>, errors: &mut Vec<String>) -> Ladders {
    let mut order: BTreeMap<String, Vec<String>> = BTreeMap::new();
    order.insert(files::TOOL_LADDER.into(), files::TOOL_LEVELS.iter().map(|l| format!("minecraft:{l}")).collect());
    let ns = mod_ns.unwrap_or("minecraft");
    let full = |id: &str| if id.contains(':') { id.to_string() } else { format!("{ns}:{id}") };
    // (file, line, id, ladder, above)
    let mut levels: Vec<(String, usize, String, String, Option<String>)> = Vec::new();
    for (file, text) in defs {
        let doc = match yog_data::text::parse(text) {
            Ok(d) => d,
            Err(e) => {
                errors.push(format!("{file}:{}: {}", e.at.line, e.message));
                continue;
            }
        };
        match yog_data::schema::from_doc::<files::Def>(text, &doc) {
            Ok(r) => match r.value {
                files::Def::Ladder { id } => {
                    if order.insert(full(&id), Vec::new()).is_some() {
                        errors.push(format!("{file}:1: ladder {} is defined twice", full(&id)));
                    }
                }
                files::Def::Level { id, ladder, above } => levels.push((file.clone(), doc.locate(text, &At::root().key("id")).line as usize, full(&id), ladder, above)),
            },
            Err(es) => {
                for e in es.0 {
                    let l = e.at.map_or(1, |a| a.line as usize);
                    errors.push(if e.path.is_root() { format!("{file}:{l}: {}", e.message) } else { format!("{file}:{l}: {}: {}", e.path, e.message) });
                }
            }
        }
    }
    // Place each level once what it's above is placed.
    loop {
        let before = levels.len();
        levels.retain(|(file, line, id, ladder, above)| {
            let Some(list) = order.get_mut(ladder) else {
                errors.push(format!("{file}:{line}: level {id}: no ladder {ladder}"));
                return false;
            };
            let at = match above {
                None => 0,
                Some(a) => match list.iter().position(|x| x == a) {
                    Some(i) => i + 1,
                    None => return true, // later
                },
            };
            if list.contains(id) {
                errors.push(format!("{file}:{line}: level {id} is defined twice"));
            } else {
                list.insert(at, id.clone());
            }
            false
        });
        if levels.is_empty() || levels.len() == before {
            break;
        }
    }
    for (file, line, id, ladder, above) in levels {
        let above = above.unwrap_or_default();
        errors.push(format!("{file}:{line}: level {id}: above {above}, which isn't a level of {ladder} (or the levels go round in a circle)"));
    }
    Ladders { order }
}

/// The tags that make block `id` need tool level `level`, on every version:
/// the game's `needs_*_tool` where they say it, and — for a level above
/// diamond — each version's own way (`incorrect_for_*_tool` on 1.21,
/// Fabric's `needs_tool_level_N` and Forge's `needs_netherite_tool` on 1.20).
fn tool_level_tags(id: &str, level: &str, ladders: &Ladders, out: &mut Compiled, err: &mut dyn FnMut(String), warn: &mut dyn FnMut(String)) {
    let Some((ladder, at)) = ladders.level(level) else { return };
    if ladder != files::TOOL_LADDER {
        err(format!("{level} is a level of {ladder}, not of the tool ladder ({})", files::TOOL_LADDER));
        return;
    }
    let tools = &ladders.order[files::TOOL_LADDER];
    let place = |l: &str| tools.iter().position(|x| x == &format!("minecraft:{l}")).unwrap_or(0);
    // The weakest of the game's tools that mines it.
    let able = ["wood", "stone", "iron", "diamond", "netherite"].into_iter().find(|v| place(v) >= at);
    let mut tag = |path: String| {
        out.tags.entry(path).or_default().insert(id.to_string());
    };
    match able {
        Some("wood") => {}
        Some(v @ ("stone" | "iron" | "diamond")) => tag(format!("data/minecraft/tags/block/needs_{v}_tool.json")),
        _ => {
            // Above diamond: diamond's tag everywhere, then each version's own.
            tag("data/minecraft/tags/block/needs_diamond_tool.json".into());
            tag("when/any/1.21/data/minecraft/tags/block/incorrect_for_diamond_tool.json".into());
            tag(format!("when/fabric/1.20/data/fabric/tags/block/needs_tool_level_{}.json", if able.is_some() { 4 } else { 5 }));
            for loader in ["forge", "neoforge"] {
                tag(format!("when/{loader}/1.20/data/forge/tags/block/needs_netherite_tool.json"));
            }
            if able.is_none() {
                tag("when/any/1.21/data/minecraft/tags/block/incorrect_for_netherite_tool.json".into());
                warn("above netherite: on Forge and NeoForge 1.20 a netherite tool still mines it (their tiers stop at netherite)".into());
            }
        }
    }
}

/// Ids of the `.yui` screens under `dir` that lay out an inventory (have
/// `<Slot>`s or a `<PlayerInventory>`).
fn inventory_screens(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            out.extend(inventory_screens(&path));
            continue;
        }
        if path.extension().is_none_or(|x| x != "yui") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if !text.contains("<Slot") && !text.contains("<PlayerInventory") {
            continue;
        }
        if let Some(screen) = text.find("<Screen")
            && let Some(at) = text[screen..].find(" id=\"")
        {
            let rest = &text[screen + at + 5..];
            if let Some(end) = rest.find('"') {
                out.insert(rest[..end].to_string());
            }
        }
    }
    out
}

/// Compiles the mod's content files (see the module docs). Any error fails
/// the whole thing, listing every problem.
pub fn compile(root: &Path, mod_id: &str, refs: &yres::Refs) -> Result<Compiled, String> {
    let mut out = Compiled::default();
    let files = find(root);
    if files.is_empty() {
        return Ok(out);
    }
    let mod_ns = valid_namespace(mod_id).then_some(mod_id);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut textures: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut generated: BTreeMap<String, String> = BTreeMap::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut recipes: Vec<Recipe> = Vec::new();
    let mut recipe_ids: HashMap<String, String> = HashMap::new();
    let mut books: Vec<BookFile> = Vec::new();
    let mut book_ids: HashMap<String, String> = HashMap::new();
    let mut inventories: Vec<(String, usize, String)> = Vec::new();
    let mut pending_books: Vec<(String, String)> = Vec::new();
    let mut pending_recipes: Vec<(String, String, PathBuf)> = Vec::new();
    let mut pending_layouts: Vec<(String, String)> = Vec::new();
    let mut pending_defs: Vec<(String, String)> = Vec::new();
    let mut deferred: Vec<(String, usize, String, yog_data::Ref)> = Vec::new();
    for path in &files {
        let shown = display(root, path);
        let Some(kind) = Kind::of(path) else { continue };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{shown}: {e}"));
                continue;
            }
        };
        if kind == Kind::Book {
            // Checked once every block, item and recipe is known.
            pending_books.push((shown.clone(), text));
            continue;
        }
        if kind == Kind::Recipe {
            // Checked once every block and item is known.
            pending_recipes.push((shown.clone(), text, path.clone()));
            continue;
        }
        if kind == Kind::Layout {
            pending_layouts.push((shown.clone(), text));
            continue;
        }
        if kind == Kind::Def {
            pending_defs.push((shown.clone(), text));
            continue;
        }
        let mut cx = Ctx {
            root,
            refs,
            file: shown.clone(),
            ns: mod_ns.unwrap_or("minecraft").to_string(),
            errors: &mut errors,
            warnings: &mut warnings,
            textures: &mut textures,
            inventories: &mut inventories,
            deferred: &mut deferred,
        };
        if let Some(entry) = compile_file(&mut cx, &text, kind, path, mod_ns, &mut out, &mut generated)
        {
            if let Some(other) = ids.insert(entry.id.clone(), shown.clone()) {
                errors.push(format!(
                    "{shown}: id {} is also defined by {other}",
                    entry.id
                ));
            }
            entries.push(entry);
        }
    }
    // Books name the mod's content, vanilla's and other mods' declared ids.
    let externals = crate::manifest::YogToml::read(&root.join("yog.toml")).map(|t| t.externals).unwrap_or_default();
    let mut known = refs::Known::new(mod_ns.unwrap_or("minecraft"), &externals);
    for e in &entries {
        known.add(if e.kind == Kind::Block { refs::RefKind::Block } else { refs::RefKind::Item }, e.id.clone());
    }
    for (shown, text, path) in &pending_recipes {
        let mut cx = Ctx {
            root,
            refs,
            file: shown.clone(),
            ns: mod_ns.unwrap_or("minecraft").to_string(),
            errors: &mut errors,
            warnings: &mut warnings,
            textures: &mut textures,
            inventories: &mut inventories,
            deferred: &mut deferred,
        };
        if let Some(r) = compile_recipe(&mut cx, text, path, mod_ns, &known) {
            if let Some(other) = recipe_ids.insert(r.id.clone(), shown.clone()) {
                errors.push(format!("{shown}: recipe {} is also defined by {other}", r.id));
            }
            recipes.push(r);
        }
    }
    for r in &recipes {
        known.add(refs::RefKind::Recipe, r.id.clone());
    }
    // Definitions: the ladders and their levels.
    let ladders = ladders(&pending_defs, mod_ns, &mut errors);
    // What blocks and items name (drops, levels): known now.
    for (file, line, path, r) in &deferred {
        let found = match r.kind.as_str() {
            "level" => ladders.level(&r.id).map(|_| ()).ok_or_else(|| format!("no level {} — the game's tool levels or a .ydef's", r.id)),
            "ladder" => ladders.order.contains_key(&r.id).then_some(()).ok_or_else(|| format!("no ladder {}", r.id)),
            kind => match refs::RefKind::from_word(kind) {
                Some(k) => known.check(k, &r.id),
                None => Ok(()),
            },
        };
        if let Err(e) = found {
            errors.push(format!("{file}:{line}: {path}: {e}"));
        }
    }
    for e in &entries {
        if let Some((level, line)) = &e.block.tool_level {
            tool_level_tags(&e.id, level, &ladders, &mut out, &mut |m| errors.push(format!("{}:{line}: block.tool_level: {m}", e.file)), &mut |m| warnings.push(format!("{}:{line}: block.tool_level: {m}", e.file)));
        }
    }
    // Recipe layouts: what they name checked; a kind laid out twice is a mistake.
    let mut layouts: Vec<yog_book::layout::RecipeLayout> = Vec::new();
    let mut laid_out: HashMap<String, String> = HashMap::new();
    for (shown, text) in &pending_layouts {
        let mut cx = Ctx {
            root,
            refs,
            file: shown.clone(),
            ns: mod_ns.unwrap_or("minecraft").to_string(),
            errors: &mut errors,
            warnings: &mut warnings,
            textures: &mut textures,
            inventories: &mut inventories,
            deferred: &mut deferred,
        };
        if let Some(l) = compile_layout(&mut cx, text, mod_ns, &known) {
            for k in &l.kinds {
                if let Some(other) = laid_out.insert(k.clone(), shown.clone()) {
                    errors.push(format!("{shown}: recipe kind {k} is also laid out by {other}"));
                }
            }
            layouts.push(l);
        }
    }
    let recipe_kinds: HashMap<String, String> = recipes.iter().map(|r| (r.id.clone(), r.ty.clone())).collect();
    for (shown, text) in &pending_books {
        let mut cx = Ctx {
            root,
            refs,
            file: shown.clone(),
            ns: mod_ns.unwrap_or("minecraft").to_string(),
            errors: &mut errors,
            warnings: &mut warnings,
            textures: &mut textures,
            inventories: &mut inventories,
            deferred: &mut deferred,
        };
        if let Some(b) = compile_book(&mut cx, text, mod_ns, &known, &layouts, &recipe_kinds) {
            if let Some(other) = book_ids.insert(b.id.clone(), shown.clone()) {
                errors.push(format!("{shown}: book {} is also defined by {other}", b.id));
            }
            books.push(b);
        }
    }
    // What recipes name from this mod must be its blocks and items (vanilla
    // and other mods' ids can't be checked here).
    if let Some(ns) = mod_ns {
        let screens = inventory_screens(&root.join("src"));
        for (file, line, id) in &inventories {
            if id.split_once(':').is_some_and(|(n, _)| n == ns) && !screens.contains(id) {
                errors.push(format!(
                    "{file}:{line}: block.inventory `{id}`: no .yui <Screen id=\"{id}\"> with <Slot>s in src/"
                ));
            }
        }
    }
    for (target, source) in &textures {
        match std::fs::read(source) {
            Ok(bytes) => out.outputs.push((target.clone(), bytes)),
            Err(e) => errors.push(format!("{}: {e}", display(root, source))),
        }
    }
    out.warnings = warnings;
    if !errors.is_empty() {
        return Err(format!(
            "invalid blocks/items in content/:\n  {}",
            errors.join("\n  ")
        ));
    }
    out.blocks = entries.iter().filter(|e| e.kind == Kind::Block).count();
    out.items = entries.len() - out.blocks;
    out.recipes = recipes.len();
    out.books = books.len();
    if (entries.iter().any(|e| e.auto) || !recipes.is_empty() || !books.is_empty())
        && !mentions_content_macro(&root.join("src"))
    {
        out.warnings.push(format!(
            "{} has blocks/items, but src/ never calls `yog_api::content!();` — they get models but aren't registered",
            CONTENT_DIR
        ));
    }
    out.source = Some(rust_source(&entries, &recipes, &books));
    Ok(out)
}

/// Whether any `src/**/*.rs` has `content!(` — the include of the generated code.
fn mentions_content_macro(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        if p.is_dir() {
            mentions_content_macro(&p)
        } else {
            p.extension().is_some_and(|x| x == "rs")
                && std::fs::read_to_string(&p).is_ok_and(|t| t.contains("content!("))
        }
    })
}

/// `ruby_ore`, `ores/ruby` → `ores_ruby`; a Rust keyword gets `r#`.
fn rust_ident(path: &str) -> String {
    let mut s: String = path
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        s.insert(0, '_');
    }
    s
}

fn float(n: f64) -> String {
    let s = format!("{n:?}");
    if s.contains('.') || s.contains('e') {
        s
    } else {
        format!("{s}.0")
    }
}

fn rust_source(entries: &[Entry], recipes: &[Recipe], books: &[BookFile]) -> String {
    let mut code = String::from(
        "// <auto-generated>\n//     Generated by `yog build` from content/**/*.yblock and *.yitem — don't edit,\n//     it's rewritten on every build. Included by `yog_api::content!()`.\n// </auto-generated>\n",
    );
    for (kind, set, def) in [
        (Kind::Block, "Blocks", "BlockDef"),
        (Kind::Item, "Items", "ItemDef"),
    ] {
        let mine: Vec<&Entry> = entries.iter().filter(|e| e.kind == kind).collect();
        let _ = writeln!(
            code,
            "\n/// The {} of `content/`: the id and a [`::yog_api::{def}`] of each.\n#[allow(dead_code)]\npub struct {set};\n\n#[allow(dead_code, clippy::all)]\nimpl {set} {{",
            if kind == Kind::Block {
                "blocks"
            } else {
                "items"
            }
        );
        let mut used: HashMap<String, usize> = HashMap::new();
        for e in mine {
            let path = e.id.split_once(':').map_or(e.id.as_str(), |(_, p)| p);
            let mut ident = rust_ident(path);
            let n = used.entry(ident.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let _ = write!(ident, "_{n}");
            }
            let upper = ident.to_ascii_uppercase();
            let ident = if is_keyword(&ident) {
                format!("r#{ident}")
            } else {
                ident
            };
            let _ = writeln!(
                code,
                "    /// `{}` — `{}`.\n    pub const {upper}: &'static str = {:?};",
                e.id, e.file, e.id
            );
            let mut b = format!("::yog_api::{def}::new(Self::{upper})");
            match kind {
                Kind::Recipe | Kind::Book | Kind::Layout | Kind::Def => unreachable!("only blocks and items get here"),
                Kind::Block => {
                    let p = &e.block;
                    if p.hardness.is_some() || p.resistance.is_some() {
                        let _ = write!(
                            b,
                            "\n            .strength({}, {})",
                            float(p.hardness.unwrap_or(1.5)),
                            float(p.resistance.unwrap_or(6.0))
                        );
                    }
                    if let Some(s) = &p.sound {
                        let _ = write!(b, "\n            .sound({s:?})");
                    }
                    if p.light > 0 {
                        let _ = write!(b, "\n            .light_level({})", p.light);
                    }
                    if p.requires_tool {
                        b.push_str("\n            .requires_tool()");
                    }
                    if p.no_collision {
                        b.push_str("\n            .no_collision()");
                    }
                    if let Some(s) = p.slipperiness {
                        let _ = write!(b, "\n            .slipperiness({})", float(s));
                    }
                    if let Some(s) = p.shape {
                        let args: Vec<String> = s.iter().map(|n| float(*n)).collect();
                        let _ = write!(b, "\n            .shape({})", args.join(", "));
                    }
                    if p.connects {
                        b.push_str("\n            .connects_to_neighbors()");
                    }
                    if !p.connect_groups.is_empty() {
                        let groups: Vec<String> =
                            p.connect_groups.iter().map(|g| format!("{g:?}")).collect();
                        let _ =
                            write!(b, "\n            .connect_groups(&[{}])", groups.join(", "));
                    }
                    if let Some(inv) = &p.inventory {
                        let _ = write!(b, "\n            .inventory({inv:?})");
                    }
                    if p.redstone {
                        b.push_str("\n            .redstone_source()");
                    }
                }
                Kind::Item => {
                    let p = &e.item;
                    if let Some(s) = p.stack_size {
                        let _ = write!(b, "\n            .max_stack({s})");
                    }
                    if p.durability > 0 {
                        let _ = write!(b, "\n            .max_damage({})", p.durability);
                    }
                    if p.fire_resistant {
                        b.push_str("\n            .fire_resistant()");
                    }
                    if p.fuel_ticks > 0 {
                        let _ = write!(b, "\n            .fuel({})", p.fuel_ticks);
                    }
                    if let Some((n, s, always)) = p.food {
                        let _ = write!(
                            b,
                            "\n            .food(::yog_api::FoodDef::new({n}, {}){})",
                            float(s),
                            if always { ".can_always_eat()" } else { "" }
                        );
                    }
                }
            }
            let _ = writeln!(
                code,
                "    /// The definition from `{}` (name and tooltip come from the lang files).\n    pub fn {ident}() -> ::yog_api::{def} {{\n        {b}\n    }}",
                e.file
            );
        }
        code.push_str("}\n");
    }

    code.push_str("\n/// Registers every block and item of `content/` whose file doesn't say\n/// `\"register\": \"manual\"` — `yog_api::content!()` calls it after\n/// `Mod::register`.\n#[allow(unused_variables)]\npub fn register(registry: &mut ::yog_api::Registry) {\n");
    let mut used: HashMap<(Kind, String), usize> = HashMap::new();
    for e in entries {
        let path = e.id.split_once(':').map_or(e.id.as_str(), |(_, p)| p);
        let mut ident = rust_ident(path);
        let n = used.entry((e.kind, ident.clone())).or_insert(0);
        *n += 1;
        if *n > 1 {
            let _ = write!(ident, "_{n}");
        }
        let ident = if is_keyword(&ident) {
            format!("r#{ident}")
        } else {
            ident
        };
        if !e.auto {
            continue;
        }
        let _ = match e.kind {
            Kind::Block => writeln!(code, "    registry.register_block(Blocks::{ident}());"),
            Kind::Item => writeln!(code, "    registry.register_item(Items::{ident}());"),
            Kind::Recipe | Kind::Book | Kind::Layout | Kind::Def => unreachable!("only blocks and items get here"),
        };
    }
    for r in recipes {
        let _ = writeln!(code, "    {}", r.registration());
    }
    for b in books {
        let _ = writeln!(code, "    registry.register_book_json({:?}, {:?});", b.id, b.json);
    }
    code.push_str("}\n");
    if entries.iter().any(|e| e.auto) || !recipes.is_empty() || !books.is_empty() {
        code.push_str(
            "\n// Runs when the mod's library loads: `export_mod!` calls `register` after\n// `Mod::register`.\n#[::yog_api::ctor::ctor(unsafe, crate_path = ::yog_api::ctor)]\nfn __yog_content_auto_register() {\n    ::yog_api::__yog_content_registrars()\n        .lock()\n        .unwrap_or_else(|e| e.into_inner())\n        .push(register);\n}\n",
        );
    }
    code
}

fn is_keyword(s: &str) -> bool {
    const KW: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
        "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final",
        "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
    ];
    KW.contains(&s)
}

/// Writes `compiled.source` to [`GENERATED_FILE`] (only when it changed);
/// deletes it when the mod has no content files.
pub fn write_source(root: &Path, compiled: &Compiled) -> Result<(), String> {
    let out = root.join(GENERATED_FILE);
    let Some(code) = &compiled.source else {
        let _ = std::fs::remove_file(&out);
        return Ok(());
    };
    crate::write_if_changed(&out, code)?;
    Ok(())
}

/// Outputs in order, keyed by path: [`OutputSet::put`] replaces a path's
/// output (moving it to the end) without rescanning the list.
#[derive(Default)]
pub struct OutputSet {
    items: Vec<Option<Output>>,
    at: HashMap<String, usize>,
}

impl OutputSet {
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        let i = *self.at.get(path)?;
        self.items[i].as_ref().map(|(_, b)| b.as_slice())
    }

    pub fn contains(&self, path: &str) -> bool {
        self.at.contains_key(path)
    }

    pub fn paths(&self) -> impl Iterator<Item = &String> {
        self.at.keys()
    }

    /// Sets `path`'s output, dropping the one it had.
    pub fn put(&mut self, path: String, bytes: Vec<u8>) {
        if let Some(i) = self.at.insert(path.clone(), self.items.len()) {
            self.items[i] = None;
        }
        self.items.push(Some((path, bytes)));
    }

    pub fn into_vec(self) -> Vec<Output> {
        self.items.into_iter().flatten().collect()
    }
}

impl From<Vec<Output>> for OutputSet {
    fn from(outputs: Vec<Output>) -> Self {
        let mut set = OutputSet::default();
        for (path, bytes) in outputs {
            set.put(path, bytes);
        }
        set
    }
}

/// A JSON object output at `path`: the one already in `outputs`, else the
/// mod's own file there, else empty.
fn json_output(outputs: &OutputSet, root: &Path, path: &str) -> Result<Map<String, Value>, String> {
    let read;
    let bytes = match outputs.get(path) {
        Some(b) => b,
        None => match std::fs::read(root.join(path)) {
            Ok(b) => {
                read = b;
                &read
            }
            Err(_) => return Ok(Map::new()),
        },
    };
    serde_json::from_slice::<Map<String, Value>>(bytes).map_err(|e| format!("{path}: {e}"))
}

/// Adds the content's outputs to `outputs` (the `.yres` ones, say): lang
/// entries and tags merged into what's there (or into the mod's own files),
/// the rest appended.
pub fn merge_outputs(
    root: &Path,
    outputs: &mut Vec<Output>,
    compiled: Compiled,
) -> Result<(), String> {
    let mut set = OutputSet::from(std::mem::take(outputs));
    let mut errors = Vec::new();
    // `.yres` texts under the keys the game looks up: in every lang file
    // that has the text.
    if !compiled.lang_aliases.is_empty() {
        let mut lang_files: BTreeSet<String> =
            set.paths().filter(|p| is_lang_path(p)).cloned().collect();
        if let Ok(dirs) = std::fs::read_dir(root.join("assets")) {
            for ns in dirs.flatten() {
                if let Ok(files) = std::fs::read_dir(ns.path().join("lang")) {
                    for f in files.flatten() {
                        if let Ok(rel) = f.path().strip_prefix(root) {
                            let rel = rel.to_string_lossy().replace('\\', "/");
                            if is_lang_path(&rel) {
                                lang_files.insert(rel);
                            }
                        }
                    }
                }
            }
        }
        for path in lang_files {
            let mut map = match json_output(&set, root, &path) {
                Ok(m) => m,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            let mut changed = false;
            for (from, to) in &compiled.lang_aliases {
                if let Some(text) = map.get(from).cloned()
                    && !map.contains_key(to)
                {
                    map.insert(to.clone(), text);
                    changed = true;
                }
            }
            if changed {
                set.put(path, pretty(&Value::Object(map)));
            }
        }
    }
    for ((ns, locale), entries) in &compiled.lang {
        let path = format!("assets/{ns}/lang/{locale}.json");
        let mut map = match json_output(&set, root, &path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        for (k, v) in entries {
            map.entry(k.clone())
                .or_insert_with(|| Value::String(v.clone()));
        }
        set.put(path, pretty(&Value::Object(map)));
    }
    for (path, ids) in &compiled.tags {
        // The mod's own tag file may use the pre-1.21 spelling; the loader
        // maps both to one file, so merge into whichever it has.
        let plural = path.replacen("/tags/block/", "/tags/blocks/", 1);
        let path = if !set.contains(path) && root.join(&plural).is_file() {
            plural
        } else {
            path.clone()
        };
        let mut map = match json_output(&set, root, &path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let values = map
            .entry("values")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(list) = values {
            for id in ids {
                if !list.iter().any(|v| v.as_str() == Some(id)) {
                    list.push(Value::String(id.clone()));
                }
            }
        }
        map.entry("replace").or_insert(Value::Bool(false));
        set.put(path, pretty(&Value::Object(map)));
    }
    for (path, bytes) in compiled.outputs {
        set.put(path, bytes);
    }
    *outputs = set.into_vec();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "merging blocks/items into the mod's files:\n  {}",
            errors.join("\n  ")
        ))
    }
}

fn is_lang_path(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    parts.len() == 4 && parts[0] == "assets" && parts[2] == "lang" && parts[3].ends_with(".json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "yog-content-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        dir
    }

    fn output(c: &Compiled, path: &str) -> Value {
        let (_, b) = c
            .outputs
            .iter()
            .find(|(p, _)| p == path)
            .unwrap_or_else(|| {
                panic!(
                    "no {path} in {:?}",
                    c.outputs.iter().map(|(p, _)| p).collect::<Vec<_>>()
                )
            });
        serde_json::from_slice(b).unwrap()
    }

    #[test]
    fn compiles_a_cube_block() {
        let root = project(&[
            ("resources/block/ruby_ore.png", "png"),
            ("content/ruby.yitem", r#"{ model: { parent: "generated", textures: { layer0: "minecraft:item/emerald" } } }"#),
            (
                "content/blocks/ruby_ore.yblock",
                r#"{
  "name": "Ruby Ore",
  "block": { "hardness": 3, "resistance": 3, "sound": "stone", "tool": "pickaxe", "tool_level": "iron", "requires_tool": true,
             "drops": { "item": "@item:mymod:ruby", "count": [1, 3] } },
  "model": { "parent": "cube_all", "textures": { "all": "block/ruby_ore.png" } }
}"#,
            ),
        ]);
        let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
        assert_eq!(c.blocks, 1);
        assert_eq!(
            output(&c, "assets/mymod/models/block/ruby_ore.json"),
            json!({ "parent": "minecraft:block/cube_all", "textures": { "all": "mymod:block/ruby_ore" } })
        );
        assert_eq!(
            output(&c, "assets/mymod/blockstates/ruby_ore.json"),
            json!({ "variants": { "": { "model": "mymod:block/ruby_ore" } } })
        );
        assert_eq!(
            output(&c, "assets/mymod/models/item/ruby_ore.json"),
            json!({ "parent": "mymod:block/ruby_ore" })
        );
        assert_eq!(
            output(&c, "assets/mymod/items/ruby_ore.json")["model"]["model"],
            json!("mymod:item/ruby_ore")
        );
        let loot = output(&c, "data/mymod/loot_table/blocks/ruby_ore.json");
        assert_eq!(loot["pools"][0]["entries"][0]["name"], json!("mymod:ruby"));
        assert_eq!(
            loot["pools"][0]["entries"][0]["functions"][0]["count"]["max"],
            json!(3)
        );
        assert!(
            c.outputs
                .iter()
                .any(|(p, _)| p == "assets/mymod/textures/block/ruby_ore.png")
        );
        let src = c.source.as_ref().unwrap();
        assert!(
            src.contains("pub const RUBY_ORE: &'static str = \"mymod:ruby_ore\";"),
            "{src}"
        );
        assert!(src.contains(".strength(3.0, 3.0)"), "{src}");
        assert!(src.contains(".requires_tool()"), "{src}");
        assert!(
            src.contains("registry.register_block(Blocks::ruby_ore());"),
            "{src}"
        );

        let mut outputs = Vec::new();
        merge_outputs(&root, &mut outputs, c).unwrap();
        let find = |p: &str| -> Value {
            serde_json::from_slice(&outputs.iter().find(|(q, _)| q == p).unwrap().1).unwrap()
        };
        assert_eq!(
            find("assets/mymod/lang/en_us.json")["block.mymod.ruby_ore"],
            json!("Ruby Ore")
        );
        assert_eq!(
            find("data/minecraft/tags/block/mineable/pickaxe.json")["values"],
            json!(["mymod:ruby_ore"])
        );
        assert_eq!(
            find("data/minecraft/tags/block/needs_iron_tool.json")["values"],
            json!(["mymod:ruby_ore"])
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn raw_parent_with_elements_gets_a_particle() {
        let root = project(&[(
            "content/cable.yblock",
            r##"{ "model": { "parent": "minecraft:block/block", "textures": { "all": "minecraft:block/stone" },
                 "elements": [ { "from": [5, 5, 5], "to": [11, 11, 11], "faces": { "up": { "texture": "#all" } } } ] } }"##,
        )]);
        let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
        let m = output(&c, "assets/mymod/models/block/cable.json");
        assert_eq!(m["textures"]["particle"], json!("minecraft:block/stone"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compiles_elements_shape_and_connect() {
        let root = project(&[(
            "content/pipe.yblock",
            r##"{
  "block": { "render_layer": "cutout" },
  "model": {
    "textures": { "0": "minecraft:block/stone" },
    "elements": [ { "from": [6, 6, 6], "to": [10, 10, 10],
                    "rotation": { "axis": "y", "angle": 22.5 },
                    "faces": { "north": { "texture": "#0", "uv": [0, 0, 4, 4], "cullface": "north" } } } ]
  },
  "states": { "connect": { "arm": { "textures": { "0": "minecraft:block/stone" },
     "elements": [ { "from": [6, 6, 0], "to": [10, 10, 6], "faces": { "up": { "texture": "#0" } } } ] } } }
}"##,
        )]);
        let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
        let m = output(&c, "assets/mymod/models/block/pipe.json");
        assert_eq!(m["textures"]["particle"], json!("minecraft:block/stone"));
        assert_eq!(m["render_type"], json!("minecraft:cutout"));
        assert_eq!(m["elements"][0]["rotation"]["origin"], json!([8, 8, 8]));
        let bs = output(&c, "assets/mymod/blockstates/pipe.json");
        assert_eq!(bs["multipart"].as_array().unwrap().len(), 7);
        assert_eq!(bs["multipart"][5]["apply"]["x"], json!(270));
        let src = c.source.unwrap();
        assert!(
            src.contains(".shape(6.0, 6.0, 6.0, 10.0, 10.0, 10.0)"),
            "{src}"
        );
        assert!(src.contains(".connects_to_neighbors()"), "{src}");
        assert!(src.contains(".connect_groups(&[\"mymod:pipe\"])"), "{src}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn tools_are_tags_and_levels_are_definitions_a_mod_can_add() {
        let cube = r#"model: { parent: "cube_all", textures: { all: "minecraft:block/stone" } }"#;
        let mythril = r#"{ kind: "level", id: "mythril", ladder: "@ladder:minecraft:tool", above: "@level:minecraft:diamond" }"#;
        let block = format!(r#"{{ {cube}, block: {{ tool: ["pickaxe", "@tag:block:mymod:mineable/hammer"], tool_level: "@level:mymod:mythril" }} }}"#);
        let c = compile(&project(&[("content/ore.yblock", &block), ("content/mythril.ydef", mythril)]), "mymod", &yres::Refs::default()).unwrap();
        for tag in [
            "data/minecraft/tags/block/mineable/pickaxe.json",
            "data/mymod/tags/block/mineable/hammer.json",
            // Above diamond, on every version its own way.
            "data/minecraft/tags/block/needs_diamond_tool.json",
            "when/any/1.21/data/minecraft/tags/block/incorrect_for_diamond_tool.json",
            "when/fabric/1.20/data/fabric/tags/block/needs_tool_level_4.json",
            "when/forge/1.20/data/forge/tags/block/needs_netherite_tool.json",
        ] {
            assert!(c.tags.get(tag).is_some_and(|ids| ids.contains("mymod:ore")), "{tag}: {:?}", c.tags);
        }
        // The game's levels by name.
        let iron = format!(r#"{{ {cube}, block: {{ tool: "pickaxe", tool_level: "iron" }} }}"#);
        let c = compile(&project(&[("content/ore.yblock", &iron)]), "mymod", &yres::Refs::default()).unwrap();
        assert!(c.tags["data/minecraft/tags/block/needs_iron_tool.json"].contains("mymod:ore"));
        assert_eq!(c.tags.len(), 2);
        // Mistakes.
        let bad = format!(r#"{{ {cube}, block: {{ tool: "hammer", tool_level: "mythril" }} }}"#);
        let e = compile(&project(&[("content/ore.yblock", &bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("block.tool: `hammer`: one of pickaxe, axe, shovel, hoe — or a tag of the mod's own tools"), "{e}");
        assert!(e.contains("block.tool_level: `mythril`: one of wood, stone, iron, diamond, netherite — or a level of the tool ladder"), "{e}");
        let volts = r#"{ kind: "ladder", id: "voltage" }"#;
        let lv = r#"{ kind: "level", id: "lv", ladder: "@ladder:mymod:voltage" }"#;
        let loop_ = r#"{ kind: "level", id: "hv", ladder: "@ladder:mymod:voltage", above: "@level:mymod:nope" }"#;
        let wrong = format!(r#"{{ {cube}, block: {{ tool_level: "@level:mymod:lv", drops: {{ item: "@item:mymod:nope" }} }} }}"#);
        let e = compile(&project(&[("content/v.ydef", volts), ("content/lv.ydef", lv), ("content/hv.ydef", loop_), ("content/ore.yblock", &wrong)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("level mymod:hv: above mymod:nope, which isn't a level of mymod:voltage"), "{e}");
        assert!(e.contains("block.tool_level: mymod:lv is a level of mymod:voltage, not of the tool ladder"), "{e}");
        assert!(e.contains("block.drops.item: no item mymod:nope in this mod"), "{e}");
    }

    #[test]
    fn reports_errors_with_lines() {
        // The schema first: every structural problem, with its line and path.
        let root = project(&[(
            "content/bad.yblock",
            "{\n  \"model\": {\n    \"textures\": { \"0\": \"minecraft:block/stone\" },\n    \"elements\": [ {\n      \"from\": [0, 0, 0], \"to\": [16, 16, 99],\n      \"rotation\": { \"axis\": \"q\", \"angle\": 30 },\n      \"faces\": { \"nort\": { \"texture\": \"#1\" } }\n    } ]\n  },\n  \"bogus\": 1\n}",
        )]);
        let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
        for want in [
            "content/bad.yblock:5: model.elements[0].to: 99 is out of range (-16..=32)",
            "content/bad.yblock:6: model.elements[0].rotation.axis: unknown `q` — did you mean `x`?",
            "content/bad.yblock:7: model.elements[0].faces.nort: unknown field `nort` — did you mean `north`?",
            "content/bad.yblock:10: bogus: unknown field `bogus`",
        ] {
            assert!(e.contains(want), "missing {want}\n{e}");
        }
        let _ = std::fs::remove_dir_all(root);
        // Then what it means: rotations Minecraft can't do, slots that aren't there.
        let root = project(&[(
            "content/bad.yblock",
            "{\n  model: {\n    textures: { \"0\": \"minecraft:block/stone\" },\n    elements: [ {\n      from: [0, 0, 0], to: [16, 16, 16],\n      rotation: { axis: \"x\", angle: 30 },\n      faces: { north: { texture: \"#1\" } }\n    } ]\n  },\n}",
        )]);
        let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
        for want in [
            "content/bad.yblock:6: model.elements[0].rotation.angle: 30 — Minecraft rotates elements by -45, -22.5, 0, 22.5 or 45 degrees",
            "content/bad.yblock:7: model.elements[0].faces.north.texture: `#1`: no such slot in textures",
        ] {
            assert!(e.contains(want), "missing {want}\n{e}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compiles_recipes() {
        let root = project(&[
            ("content/ruby.yitem", r#"{ "model": { "parent": "generated", "textures": { "layer0": "minecraft:item/emerald" } } }"#),
            (
                "content/recipes/ruby_block.yrecipe",
                r##"{ "type": "shaped", "pattern": ["RR", "RR"], "key": { "R": "@item:mymod:ruby" }, "result": "@item:minecraft:emerald_block" }"##,
            ),
            (
                "content/recipes/ruby.yrecipe",
                r##"{ "type": "shapeless", "ingredients": ["@item:minecraft:emerald", "@tag:item:c:dusts"], "result": "@item:mymod:ruby", "count": 2 }"##,
            ),
            (
                "content/recipes/smelt_ruby.yrecipe",
                r##"{ "type": "smelting", "ingredient": "@tag:item:c:ores", "result": "@item:mymod:ruby", "experience": 0.7, "$comment": "metadata goes", "_editor": { "x": 1 } }"##,
            ),
            (
                "content/recipes/cut.yrecipe",
                r##"{ "type": "stonecutting", "ingredient": ["@item:minecraft:stone", "@item:minecraft:andesite"], "result": "@item:mymod:ruby", "group": "rubies" }"##,
            ),
        ]);
        let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
        assert_eq!(c.recipes, 4);
        let src = c.source.clone().unwrap();
        for want in [
            r#"kind: ::yog_api::RecipeKind::Shaped { pattern: vec!["RR".into(), "RR".into()], key: vec![('R', vec!["mymod:ruby".into()])] }, result: "minecraft:emerald_block".into(), count: 1"#,
            r##"Shapeless { ingredients: vec![vec!["minecraft:emerald".into()], vec!["#c:dusts".into()]] }, result: "mymod:ruby".into(), count: 2"##,
            r##"Cooking { method: ::yog_api::Cooking::Smelting, ingredient: vec!["#c:ores".into()], experience: 0.7_f32, cook_time: 200 }"##,
            r##"Stonecutting { ingredient: vec!["minecraft:stone".into(), "minecraft:andesite".into()] }"##,
            r##"group: Some("rubies".into())"##,
        ] {
            assert!(src.contains(want), "missing {want}\n{src}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn recipe_mistakes() {
        let bad = |recipe: &str| {
            let root = project(&[("content/r.yrecipe", recipe)]);
            let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
            let _ = std::fs::remove_dir_all(root);
            e
        };
        let e = bad(r#"{ "type": "shaped", "pattern": ["AB"], "key": { "A": "@item:minecraft:stick" }, "result": "@item:minecraft:stick" }"#);
        assert!(e.contains("pattern uses 'B'"), "{e}");
        let e = bad(r#"{ "type": "shapeless", "ingredients": ["stick"], "result": "@item:minecraft:stick" }"#);
        assert!(e.contains("ingredients[0]: `stick`: name it by a reference — `@item:stick`"), "{e}");
        let e = bad(r##"{ "type": "shapeless", "ingredients": ["#c:sand"], "result": "@item:minecraft:glass" }"##);
        assert!(e.contains("`#c:sand`: write a tag as `@tag:item:c:sand`"), "{e}");
        let e = bad(r##"{ "type": "smelting", "ingredient": "@item:minecraft:sand", "result": "@tag:item:c:glass" }"##);
        assert!(e.contains("result: expected a reference (`@item:`), found a tag"), "{e}");
        let e = bad(r#"{ "type": "shapeless", "ingredients": ["@item:minecraft:sand"], "result": "@item:mymod:nothing" }"#);
        assert!(e.contains("result: no item mymod:nothing in this mod"), "{e}");
        let e = bad(r#"{ "type": "shapeless", "ingredients": ["@item:minecraft:nope"], "result": "@item:minecraft:glass" }"#);
        assert!(e.contains("ingredients[0]: no item minecraft:nope in Minecraft"), "{e}");
        let e = bad(r#"{ "type": "shapeless", "pattern": ["A"], "ingredients": ["@item:minecraft:sand"], "result": "@item:minecraft:glass" }"#);
        assert!(e.contains("unknown field `pattern`"), "{e}");
        let e = bad(r#"{ "type": "smeltin", "ingredient": "@item:minecraft:sand", "result": "@item:minecraft:glass" }"#);
        assert!(e.contains("unknown `smeltin` — did you mean `smelting`?"), "{e}");
    }

    #[test]
    fn block_inventory_names_a_screen() {
        let block = r#"{ "block": { "inventory": "mymod:bench" }, "model": { "parent": "cube_all", "textures": { "all": "minecraft:block/stone" } } }"#;
        let root = project(&[
            ("content/bench.yblock", block),
            ("src/bench.yui", r#"<Screen id="mymod:bench" width="176" height="166" x:ViewModel="Vm"><Slot index="0"/></Screen>"#),
        ]);
        let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
        assert!(c.source.unwrap().contains(r#".inventory("mymod:bench")"#));
        let _ = std::fs::remove_dir_all(root);

        let root = project(&[("content/bench.yblock", block)]);
        let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("no .yui <Screen id=\"mymod:bench\"> with <Slot>s"), "{e}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compiles_a_book() {
        let book = r#"{
            "id": "guide",
            "name": "@Book.title",
            "categories": [{ "id": "mymod:basics", "name": "Basics", "icon": "@item:minecraft:book" }],
            "entries": [{
                "id": "mymod:ruby", "name": "@item:mymod:ruby", "category": "@category:mymod:basics",
                "pages": [
                    { "type": "text", "text": "@@not a ref, {link=@entry:mymod:ruby|see}" },
                    { "type": "spotlight", "item": "@item:mymod:ruby" },
                    { "type": "entity", "entity": "@entity:minecraft:allay" }
                ]
            }]
        }"#;
        let ruby = r#"{ "name": "Ruby", "model": { "parent": "generated", "textures": { "layer0": "minecraft:item/emerald" } } }"#;
        let root = project(&[("content/guide.ybook", book), ("content/ruby.yitem", ruby)]);
        let mut refs = yres::Refs::default();
        refs.texts.insert("Book.title".into(), ("mymod.book.title".into(), "Guide".into()));
        let out = compile(&root, "mymod", &refs).unwrap();
        assert_eq!(out.books, 1);
        let src = out.source.unwrap();
        assert!(src.contains("registry.register_book_json(\"mymod:guide\""), "{src}");
        assert!(src.contains("mymod.book.title") && src.contains("item.mymod.ruby"), "{src}");
        assert!(src.contains("not a ref") && src.contains("i18n"), "{src}");
        assert!(src.contains(r#"\"item\":\"mymod:ruby\""#) && src.contains(r#"\"category\":\"mymod:basics\""#), "references become ids: {src}");

        // Bare ids, missing content and version-only ids are errors, each on its line.
        let bad = r#"{ "id": "x",
            "categories": [{ "id": "c", "name": "C" }],
            "entries": [{ "id": "e", "name": "E", "category": "c",
                "pages": [
                    { "type": "spotlight", "item": "minecraft:stick" },
                    { "type": "spotlight", "item": "@item:minecraft:short_grass" },
                    { "type": "recipe", "recipe": "@recipe:mymod:nope" },
                    { "type": "entity", "entity": "@entity:create:train" }
                ] }] }"#;
        let e = compile(&project(&[("content/bad.ybook", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("bad.ybook:3: entries[0].category: `c`: name it by a reference — `@category:c`"), "{e}");
        assert!(e.contains("bad.ybook:5:") && e.contains("`@item:minecraft:stick`"), "{e}");
        assert!(e.contains("not in 1.20.1"), "{e}");
        assert!(e.contains("no recipe mymod:nope in this mod"), "{e}");
        assert!(e.contains("[external.create]"), "{e}");

        let bad = project(&[("content/bad.ybook", r#"{ "id": "x", "entries": [{ "pages": [{ "type": "nope" }] }] }"#)]);
        let e = compile(&bad, "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("entries[0].pages[0].type: unknown `nope`"), "{e}");
        let bad = project(&[("content/bad.ybook", r#"{ "id": "x", "name": "X", "categories": [{ "id": "c", "name": "C" }] }"#)]);
        let e = compile(&bad, "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("categories[0].id: `c`: write its namespace (`mymod:c`)"), "{e}");
    }

    #[test]
    fn books_lay_out_the_mods_own_recipe_kinds() {
        let recipe = r#"{ type: "mymod:infusion", slots: { core: ["@item:minecraft:diamond"] }, result: "@item:minecraft:emerald" }"#;
        let book = r#"{ id: "guide", name: "G", categories: [{ id: "mymod:c", name: "C" }],
            entries: [{ id: "mymod:e", name: "E", category: "@category:mymod:c",
                pages: [{ type: "recipe", recipe: "@recipe:mymod:infusion" }] }] }"#;
        let layout = r#"{ kinds: ["mymod:infusion"], size: [96, 24], parts: [
            { type: "slots", slot: "core", at: [[4, 4]], frame: true },
            { type: "item", item: "@item:minecraft:beacon", at: [40, 4] },
            { type: "result", at: [76, 4], frame: true },
        ] }"#;
        let files = [("content/infusion.yrecipe", recipe), ("content/guide.ybook", book)];
        let e = compile(&project(&files), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("entries[0].pages[0].recipe: no layout for recipe kind mymod:infusion — add content/infusion.ylayout"), "{e}");
        let c = compile(&project(&[files[0], files[1], ("content/infusion.ylayout", layout)]), "mymod", &yres::Refs::default()).unwrap();
        assert!(c.source.unwrap().contains(r#"\"layouts\":[{\"kinds\":[\"mymod:infusion\"]"#), "the book carries the mod's layouts");
        let bad = r#"{ kinds: ["create:mixing"], size: [1, 1], parts: [{ type: "item", item: "@item:minecraft:nope", at: [0, 0] }] }"#;
        let e = compile(&project(&[("content/bad.ylayout", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("kinds[0]: `create:mixing`: one of shaped") && e.contains("parts[0].item: no item minecraft:nope in Minecraft"), "{e}");
    }

    #[test]
    fn compiles_a_recipe_of_the_mods_own_kind() {
        let r = r##"{ type: "mymod:infusion", slots: { core: ["@item:minecraft:diamond"], ring: [["@item:minecraft:coal", "@item:minecraft:charcoal"], "@tag:item:c:dusts"] },
                     values: { time: 200 }, result: "@item:minecraft:emerald", count: 2 }"##;
        let c = compile(&project(&[("content/infusion.yrecipe", r)]), "mymod", &yres::Refs::default()).unwrap();
        let src = c.source.unwrap();
        assert!(src.contains("registry.add_recipe_json(\"mymod:infusion\""), "{src}");
        assert!(src.contains(r#"\"type\":\"mymod:infusion\""#) && src.contains(r#"\"time\":200"#), "{src}");

        let bad = r#"{ "type": "mymod:infusion", "ingredient": "@item:minecraft:stone", "result": "@item:minecraft:emerald" }"#;
        let e = compile(&project(&[("content/bad.yrecipe", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("unknown field `ingredient`") && e.contains("missing `slots`"), "{e}");
        // Another mod's kind isn't the mod's own: not a Yog recipe type.
        let other = r#"{ "type": "create:mixing", "slots": {}, "result": "@item:minecraft:emerald" }"#;
        let e = compile(&project(&[("content/o.yrecipe", other)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("type `create:mixing`: one of shaped"), "{e}");
    }

    #[test]
    fn compiles_an_item_with_resource_refs() {
        let root = project(&[(
            "content/items/ruby.yitem",
            r#"{ "name": "@Items.ruby", "tooltip": "@Strings.ruby_tip",
                 "item": { "stack_size": 16, "food": { "nutrition": 4, "saturation": 0.3 } },
                 "model": { "parent": "generated", "textures": { "layer0": "@Textures.ruby" } } }"#,
        )]);
        let mut refs = yres::Refs::default();
        refs.texts.insert(
            "Items.ruby".into(),
            ("item.mymod.ruby".into(), "Ruby".into()),
        );
        refs.texts.insert(
            "Strings.ruby_tip".into(),
            ("mymod.ruby_tip".into(), "Shiny".into()),
        );
        refs.images
            .insert("Textures.ruby".into(), "mymod:item/ruby".into());
        let c = compile(&root, "mymod", &refs).unwrap();
        assert_eq!(
            output(&c, "assets/mymod/models/item/ruby.json"),
            json!({ "parent": "minecraft:item/generated", "textures": { "layer0": "mymod:item/ruby" } })
        );
        let src = c.source.clone().unwrap();
        assert!(src.contains(".max_stack(16)"), "{src}");
        assert!(
            src.contains(".food(::yog_api::FoodDef::new(4, 0.3))"),
            "{src}"
        );
        // The .yres lang file gets the tooltip under the key the game reads.
        let mut outputs = vec![(
            "assets/mymod/lang/ru_ru.json".to_string(),
            r#"{"item.mymod.ruby": "Рубин", "mymod.ruby_tip": "Блестит"}"#
                .as_bytes()
                .to_vec(),
        )];
        merge_outputs(&root, &mut outputs, c).unwrap();
        let ru: Value = serde_json::from_slice(&outputs[0].1).unwrap();
        assert_eq!(ru["item.mymod.ruby.desc"], json!("Блестит"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn namespace_needed_for_odd_mod_ids() {
        let root = project(&[(
            "content/x.yitem",
            r#"{ "model": { "parent": "generated", "textures": { "layer0": "minecraft:item/apple" } } }"#,
        )]);
        let e = compile(&root, "Example-Mod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("give the namespace"), "{e}");
        let _ = std::fs::remove_dir_all(root);
    }
}
