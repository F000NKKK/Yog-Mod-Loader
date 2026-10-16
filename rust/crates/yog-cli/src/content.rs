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
mod refs;
mod table;

pub use import::import_command;

use crate::yres;
use yog_data::Path as At;
use json::num_value;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use yog_data::Schema;

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
    /// `.ydef` — a definition of some kind: a ladder, a level on one, a
    /// sound group, a kind of recipe, a recipe layout.
    Def,
    /// `.ymodel` — a skeleton of bones and cubes (`yog_body`).
    Model,
    /// `.yanim` — animation clips (`yog_body`).
    Anim,
    /// `.yentity` — an entity type: an object, a living thing or a mob.
    Entity,
    /// `.yfont` — a font in Yog's vector-glyph format.
    Font,
}
impl Kind {
    pub fn of(path: &Path) -> Option<Kind> {
        match path.extension()?.to_str()? {
            "yblock" => Some(Kind::Block),
            "yitem" => Some(Kind::Item),
            "yrecipe" => Some(Kind::Recipe),
            "ybook" => Some(Kind::Book),
            "ydef" => Some(Kind::Def),
            "ymodel" => Some(Kind::Model),
            "yanim" => Some(Kind::Anim),
            "yentity" => Some(Kind::Entity),
            "yfont" => Some(Kind::Font),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Kind::Block => "block",
            Kind::Item => "item",
            Kind::Recipe => "recipe",
            Kind::Book => "book",
            Kind::Def => "definition",
            Kind::Model => "model",
            Kind::Anim => "animation",
            Kind::Entity => "entity",
            Kind::Font => "font",
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
    /// Tag path (`data/minecraft/tags/block/...json`) → its entries:
    /// id (`#ns:tag` for a tag) → required (`false`: kept only if present).
    tags: BTreeMap<String, BTreeMap<String, bool>>,
    /// The generated Rust code; `None` when the mod has no content files.
    pub source: Option<Source>,
    /// Data the code includes (`include_bytes!`), by path under
    /// `.yog-build/generated/`: the books, as Yog Data.
    pub data_files: Vec<(String, Vec<u8>)>,
    pub warnings: Vec<String>,
    /// [`yog_registry::REGISTRATIONS_FILE`]: what the mod registers at the
    /// game's start — packaged by dev builds only, for hot reload.
    pub registrations: String,
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

/// Every content file (`Kind`) under `content/` of the mod at `root`, sorted.
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
    /// `""`, `"horizontal"` or `"all"`.
    facing: String,
    /// `"toward"` or `"away"`.
    placement: String,
    attach: bool,
    waterlogged: bool,
    /// Its own properties: name, values.
    properties: Vec<(String, Vec<String>)>,
    /// Yes/no properties: kept on by redstone; read by a comparator.
    powered: Option<String>,
    comparator: Option<String>,
    /// Hoppers and droppers can put items into it.
    accepts_items: bool,
    /// The inventory screen it opens (a `.yui` `<Screen>` with `<Slot>`s).
    inventory: Option<String>,
    /// Gives off redstone power, set by the mod per position.
    redstone: bool,
    /// Doesn't hide the faces behind it or stop light.
    no_occlusion: bool,
    /// One of the game's kinds of block (`stairs`…); `""`: Yog's own.
    kind: String,
    /// What a kind is made from, its set, a button's press and who sets it off.
    base: Option<String>,
    set: Option<String>,
    press_ticks: Option<u32>,
    mobs_only: bool,
}

#[derive(Debug, Clone, Default)]
struct ItemProps {
    stack_size: Option<u8>,
    durability: u32,
    fire_resistant: bool,
    fuel_ticks: u32,
    /// (nutrition, saturation, always_edible)
    food: Option<(u32, f64, bool)>,
    /// (kind, material, attack damage, attack speed)
    tool: Option<(String, String, f32, f32)>,
    /// (slot, material)
    armor: Option<(String, String)>,
    /// Worn in this slot without being armor.
    wearable: Option<String>,
    /// The hanging entity it places, and its size in blocks.
    hangs: Option<(String, Option<[u32; 2]>)>,
}

/// One checked content file — what the Rust code needs.
#[derive(Debug, Clone)]
struct Entry {
    kind: Kind,
    id: String,
    file: String,
    auto: bool,
    /// Mods it needs (`requires`): registered only when they're all loaded.
    requires: Vec<String>,
    block: BlockProps,
    item: ItemProps,
    potion: Option<yog_registry::PotionDef>,
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
    fn texture(&mut self, loc: Loc, path: &At, t: &yog_formats::content::Texture) -> Option<String> {
        let s = match t {
            yog_formats::content::Texture::Slot(slot) => return Some(format!("#{slot}")),
            yog_formats::content::Texture::Res(r) => {
                let name = format!("{}.{}", r.set, r.key);
                return match self.refs.images.get(&name) {
                    Some(id) => Some(id.clone()),
                    None => {
                        self.err_at(loc, path, format!("`@{name}`: no image `{name}` in resources/*.yres"));
                        None
                    }
                };
            }
            yog_formats::content::Texture::Plain(s) => s.trim().to_string(),
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
fn model(cx: &mut Ctx, loc: Loc, at: &At, m: &yog_formats::content::Model, item: bool) -> Option<Model> {
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
        // Its own shape and no parent: vanilla's block parent gives it how
        // it's held and shown in a slot — without one it's a flat face-on square.
        if m.parent.is_none() {
            out.insert("parent".into(), Value::String("minecraft:block/block".into()));
        }
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
        out.insert("gui_light".into(), Value::String(yog_formats::content::word(g)));
    }
    Some(Model { json: Value::Object(out), bounds })
}

/// One element → its JSON.
fn element(cx: &mut Ctx, loc: Loc, at: &At, el: &yog_formats::content::Element, textures: &Map<String, Value>, raw_parent: bool) -> Option<Value> {
    let mut out = Map::new();
    out.insert("from".into(), nums(&el.from));
    out.insert("to".into(), nums(&el.to));
    if let Some(r) = &el.rotation {
        if (r.angle / 22.5).fract() != 0.0 {
            cx.err_at(loc, &at.key("rotation").key("angle"), format!("{} — Minecraft rotates elements by -45, -22.5, 0, 22.5 or 45 degrees", r.angle));
        }
        let mut rot = Map::new();
        rot.insert("origin".into(), nums(&r.origin.unwrap_or([8.0, 8.0, 8.0])));
        rot.insert("axis".into(), Value::String(yog_formats::content::word(&r.axis)));
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
                    fo.insert("cullface".into(), Value::String(yog_formats::content::word(c)));
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
/// How a state turns its model (x, then y, in degrees): `facing` is where
/// its front (the model's north side) looks; `face`, where an attached
/// block (its model lying on the floor) hangs.
fn turn(facing: Option<&str>, face: Option<&str>) -> (f64, f64) {
    let yaw = |f: &str| match f {
        "east" => 90.0,
        "south" => 180.0,
        "west" => 270.0,
        _ => 0.0,
    };
    match (face, facing) {
        (Some("wall"), Some(f)) => (90.0, yaw(f)),
        (Some("ceiling"), Some(f)) => (180.0, (yaw(f) + 180.0) % 360.0),
        (_, Some("up")) => (270.0, 0.0),
        (_, Some("down")) => (90.0, 0.0),
        (_, Some(f)) => (0.0, yaw(f)),
        _ => (0.0, 0.0),
    }
}

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
        register: Option<yog_formats::content::Register>,
        model: Option<yog_formats::content::Model>,
        item_model: Option<yog_formats::content::Model>,
        block: Option<yog_formats::content::BlockProps>,
        states: Option<yog_formats::content::States>,
        item: Option<yog_formats::content::ItemProps>,
        /// A block's item form.
        block_item: Option<yog_formats::content::BlockItemProps>,
        requires: Vec<String>,
        tags: Vec<String>,
        /// One of the game's kinds of block.
        kind: Option<String>,
    }
    let (parts, refs, tag_refs) = match kind {
        Kind::Item => match yog_data::schema::from_doc::<yog_formats::content::ItemFile>(text, &doc) {
            Ok(r) => {
                let f = r.value;
                (Parts { format: f.format, id: f.id, name: f.name, tooltip: f.tooltip, register: f.register, model: f.model, item_model: None, block: None, states: None, item: f.item, block_item: None, requires: f.requires, tags: f.tags, kind: None }, r.refs, r.tags)
            }
            Err(e) => {
                cx.schema_errors(e);
                return None;
            }
        },
        Kind::Block => match yog_data::schema::from_doc::<yog_formats::content::BlockFile>(text, &doc) {
            Ok(r) => {
                let f = r.value;
                (Parts { format: f.format, id: f.id, name: f.name, tooltip: f.tooltip, register: f.register, model: Some(f.model), item_model: f.item_model, block: f.block, states: f.states, item: None, block_item: f.item, requires: f.requires, tags: f.tags, kind: f.kind }, r.refs, r.tags)
            }
            Err(e) => {
                cx.schema_errors(e);
                return None;
            }
        },
        Kind::Recipe | Kind::Book | Kind::Def | Kind::Model | Kind::Anim | Kind::Entity | Kind::Font => unreachable!("only blocks and items get here"),
    };
    // What it names (drops) is checked once every file is read.
    for f in &refs {
        cx.deferred.push((cx.file.clone(), loc(&f.path), f.path.to_string(), f.value.clone()));
    }
    for f in &tag_refs {
        cx.deferred.push((cx.file.clone(), loc(&f.path), f.path.to_string(), yog_data::Ref::new(format!("{TAG_REF}{}", f.value.kind), f.value.id.clone())));
    }
    // Its tags (checked like the rest): a block's are block or item tags.
    for (i, tag) in parts.tags.iter().enumerate() {
        let at = root.key("tags").index(i);
        let of = if kind == Kind::Block { "block|item" } else { "item" };
        cx.deferred.push((cx.file.clone(), loc(&at), at.to_string(), yog_data::Ref::new(format!("{TAG_REF}{of}"), full_id(tag.trim_start_matches('#'), mod_ns.unwrap_or("minecraft")))));
    }
    if let Some(f) = parts.format
        && f != 1
    {
        cx.err_at(loc, &root.key("format"), "format must be 1 (this yog understands format 1)");
    }
    let (ns, p) = file_id(cx, loc(&root.key("id")), parts.id.as_deref(), path, mod_ns)?;
    cx.ns = ns.clone();
    let id = format!("{ns}:{p}");
    let auto = parts.register.unwrap_or_default() == yog_formats::content::Register::Auto;

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

    let potion = parts.item.as_ref().and_then(|it| it.potion.as_ref()).map(|p| yog_registry::PotionDef {
        id: id.clone(),
        name: p.name.clone(),
        effects: p.effects.clone(),
        brewed: Vec::new(),
    });
    let mut entry = Entry {
        kind,
        id: id.clone(),
        file: cx.file.clone(),
        auto,
        requires: parts.requires.clone(),
        block: BlockProps::default(),
        item: ItemProps::default(),
        potion,
    };

    // Its tags: a block's block tag, and the item tag of the same name for its item form.
    for tag in &parts.tags {
        let tag = full_id(tag.trim_start_matches('#'), &cx.ns);
        let (tns, tpath) = tag.split_once(':').unwrap_or(("minecraft", &tag));
        if kind == Kind::Block {
            out.tags.entry(format!("data/{tns}/tags/block/{tpath}.json")).or_default().insert(id.clone(), true);
        }
        out.tags.entry(format!("data/{tns}/tags/item/{tpath}.json")).or_default().insert(id.clone(), true);
    }

    // A block's item form: its stack, fire and fuel.
    if let Some(it) = &parts.block_item {
        entry.item.stack_size = it.stack_size;
        entry.item.fire_resistant = it.fire_resistant;
        entry.item.fuel_ticks = it.fuel_ticks.unwrap_or(0);
    }

    if kind == Kind::Item {
        if let Some(it) = &parts.item {
            let props = &mut entry.item;
            props.stack_size = it.stack_size;
            props.durability = it.durability.unwrap_or(0);
            props.fire_resistant = it.fire_resistant;
            props.fuel_ticks = it.fuel_ticks.unwrap_or(0);
            props.food = it.food.as_ref().map(|f| (f.nutrition, f.saturation, f.always_edible));
            props.tool = it.tool.as_ref().map(|t| {
                let (d, s) = t.attack();
                (t.kind.clone(), t.material.clone(), d, s)
            });
            props.armor = it.armor.as_ref().map(|a| (yog_formats::content::word(&a.slot), a.material.clone()));
            props.wearable = it.wearable.as_ref().map(yog_formats::content::word);
            props.hangs = it.hangs.as_ref().map(|h| (h.entity.clone(), h.size));
            if it.hangs.as_ref().and_then(|h| h.size).is_some_and(|[w, h]| w == 0 || h == 0) {
                cx.err_at(loc, &root.key("item").key("hangs").key("size"), "a hanging thing is at least a block wide and high");
            }
            if props.wearable.is_some() && props.armor.is_some() {
                cx.err_at(loc, &root.key("item").key("wearable"), "armor is worn already — leave wearable out");
            }
            if props.tool.is_some() && props.armor.is_some() {
                cx.err_at(loc, &root.key("item").key("armor"), "an item is a tool or armor, not both");
            }
            if props.durability > 0 && props.stack_size.is_some_and(|s| s > 1) {
                cx.warn_at(loc, &root.key("item").key("stack_size"), "ignored: an item with durability stacks to 1");
            }
        }
        if entry.potion.is_some() {
            return Some(entry);
        }
        let Some(item_model) = parts.model.as_ref() else {
            cx.err_at(loc, &root.key("model"), "an item without potion effects needs a model");
            return None;
        };
        let m = model(cx, loc, &root.key("model"), item_model, true)?;
        emit(cx, format!("models/item/{p}.json"), &m.json);
        emit(cx, format!("items/{p}.json"), &json!({ "model": { "type": "minecraft:model", "model": format!("{ns}:item/{p}") } }));
        return Some(entry);
    }

    // A block.
    let b = parts.block.clone().unwrap_or_default();
    let bat = root.key("block");
    let kind_of = parts.kind.as_deref().and_then(yog_formats::content::BlockKind::built_in);
    let props = &mut entry.block;
    props.hardness = b.hardness;
    props.resistance = b.resistance;
    props.sound = b.sound.as_ref().map(yog_formats::content::word);
    props.light = b.light.unwrap_or(0);
    props.tools = b.tool.iter().map(|t| t.0.clone()).collect();
    props.tool_level = b.tool_level.clone().map(|l| (l, loc(&bat.key("tool_level"))));
    props.requires_tool = b.requires_tool;
    props.no_collision = !b.collision.or(kind_of.and_then(|k| k.collision)).unwrap_or(true);
    props.no_occlusion = !b.occludes.or(kind_of.and_then(|k| k.occludes)).unwrap_or(true);
    props.slipperiness = b.slipperiness;
    props.redstone = b.redstone;
    props.accepts_items = b.accepts_items;
    props.connect_groups = b.connect_groups.clone();
    let shape_auto = matches!(b.shape, None | Some(yog_formats::content::BlockShape::Auto));
    if let Some(yog_formats::content::BlockShape::Box(bx)) = b.shape {
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
    // A kind of the game's: what it's made of and its set; nothing of Yog's own blocks.
    if let Some(k) = kind_of {
        props.kind = k.name.clone();
        props.base = b.base.clone();
        props.set = b.set.clone();
        props.press_ticks = b.press_ticks;
        props.mobs_only = b.triggered_by == Some(yog_formats::content::TriggeredBy::Mobs);
        let own = [
            ("shape", b.shape.is_some()),
            ("connect_groups", !b.connect_groups.is_empty()),
            ("inventory", b.inventory.is_some()),
            ("redstone", b.redstone),
            ("powered", b.powered.is_some()),
            ("comparator", b.comparator.is_some()),
            ("accepts_items", b.accepts_items),
        ];
        for (field, _) in own.iter().filter(|(_, given)| *given) {
            cx.err_at(loc, &bat.key(field), format!("{field} doesn't go with kind {} (the game's block)", k.name));
        }
        if parts.states.is_some() {
            cx.err_at(loc, &root.key("states"), format!("kind {} has the game's states — leave states out", k.name));
        }
        if k.takes("base") && b.base.is_none() {
            cx.err_at(loc, &bat, format!("kind {} is made from a block — base: \"@block:mymod:planks\"", k.name));
        }
    }
    let kind_fields = [("base", b.base.is_some()), ("set", b.set.is_some()), ("press_ticks", b.press_ticks.is_some()), ("triggered_by", b.triggered_by.is_some())];
    for (field, _) in kind_fields.iter().filter(|(_, given)| *given) {
        match kind_of {
            Some(k) if k.takes(field) => {}
            Some(k) => cx.err_at(loc, &bat.key(field), format!("kind {} doesn't take {field}", k.name)),
            None => cx.err_at(loc, &bat.key(field), "only a block of a kind takes it (kind: \"stairs\", \"door\"…)"),
        }
    }
    let drops = match &b.drops {
        None | Some(yog_formats::content::Drops::Itself) => match kind_of {
            Some(k) => kind_drops(&id, k),
            None => drops_table(&id, None),
        },
        Some(yog_formats::content::Drops::Nothing) => Value::Null,
        Some(yog_formats::content::Drops::Item(d)) => drops_table(d.item.as_deref().unwrap_or(&id), d.count),
    };
    let render_layer = b.render_layer.as_ref().map(yog_formats::content::word);

    // Its models and states: a kind's, or its own.
    let item_parent = if let Some(k) = kind_of {
        kind_models(cx, loc, k, parts.model.as_ref().expect("blocks always have a model"), b.base.as_deref(), render_layer.as_deref(), &ns, &p, &mut emit)
    } else {
        let mut m = model(cx, loc, &root.key("model"), parts.model.as_ref().expect("blocks always have a model"), false)?;
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
            if s.facing.is_some() || s.attach || s.waterlogged || !s.properties.0.is_empty() {
                if s.rotations.is_some() || s.connect.is_some() {
                    cx.err_at(loc, &sat, "facing, attach, waterlogged and properties can't go with rotations or connect");
                }
                let b = &mut entry.block;
                use yog_formats::content::{Facing, Placement};
                if s.attach {
                    if s.facing == Some(Facing::All) {
                        cx.err_at(loc, &sat.key("facing"), "an attached block turns four ways — leave facing out");
                    }
                    b.attach = true;
                    b.facing = "horizontal".into();
                } else if let Some(f) = s.facing {
                    b.facing = if f == Facing::All { "all" } else { "horizontal" }.into();
                }
                if !b.facing.is_empty() {
                    b.placement = if s.placement == Some(Placement::Away) { "away" } else { "toward" }.into();
                } else if s.placement.is_some() {
                    cx.err_at(loc, &sat.key("placement"), "placement needs facing (or attach)");
                }
                b.waterlogged = s.waterlogged;
                for (name, values) in &s.properties.0 {
                    let here = sat.key("properties").key(name);
                    let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
                    if !word(name) {
                        cx.err_at(loc, &here, format!("`{name}`: a property's name is lowercase letters, digits and _"));
                    } else if ["facing", "face", "waterlogged"].contains(&name.as_str()) {
                        cx.err_at(loc, &here, format!("`{name}` comes from facing, attach or waterlogged — name yours otherwise"));
                    }
                    let mut seen = std::collections::HashSet::new();
                    if values.len() < 2 || !values.iter().all(|v| word(v) && seen.insert(v.as_str())) {
                        cx.err_at(loc, &here, "two or more different values, lowercase letters, digits and _");
                    }
                    b.properties.push((name.clone(), values.clone()));
                }
            } else if s.placement.is_some() || !s.models.0.is_empty() || !s.variants.0.is_empty() {
                cx.err_at(loc, &sat, "placement, models and variants need facing, attach, waterlogged or properties");
            }
        }
        // A block's own properties with every value — facing, face and
        // waterlogged as the host makes them.
        let states: Vec<(String, Vec<String>)> = {
            let b = &entry.block;
            let words = |ws: &[&str]| ws.iter().map(|w| w.to_string()).collect::<Vec<_>>();
            let mut all = Vec::new();
            match b.facing.as_str() {
                "all" => all.push(("facing".to_string(), words(&["north", "east", "south", "west", "up", "down"]))),
                "horizontal" => all.push(("facing".to_string(), words(&["north", "east", "south", "west"]))),
                _ => {}
            }
            if b.attach {
                all.push(("face".to_string(), words(&["floor", "wall", "ceiling"])));
            }
            if b.waterlogged {
                all.push(("waterlogged".to_string(), words(&["false", "true"])));
            }
            all.extend(b.properties.iter().cloned());
            all.sort_by(|a, b| a.0.cmp(&b.0));
            all
        };
        // Yes/no properties the host drives.
        let yes_no = |name: &str| states.iter().any(|(n, v)| n == name && *v == ["false", "true"]);
        for (key, value) in [("powered", &b.powered), ("comparator", &b.comparator)] {
            if let Some(name) = value {
                if yes_no(name) {
                    if key == "powered" {
                        entry.block.powered = Some(name.clone());
                    } else {
                        entry.block.comparator = Some(name.clone());
                    }
                } else {
                    cx.err_at(loc, &bat.key(key), format!("`{name}`: a yes/no property of states.properties ([\"false\", \"true\"])"));
                }
            }
        }
        // Its other models, and which state shows which.
        let mut rules: Vec<(Vec<(String, String)>, Vec<String>)> = Vec::new();
        if let Some(s) = &parts.states {
            let sat = root.key("states");
            let mut named = std::collections::HashMap::new();
            for (name, m) in &s.models.0 {
                if let Some(mut m) = model(cx, loc, &sat.key("models").key(name), m, false) {
                    if let (Some(layer), Value::Object(o)) = (&render_layer, &mut m.json) {
                        o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
                    }
                    emit(cx, format!("models/block/{p}/{name}.json"), &m.json);
                    named.insert(name.clone(), format!("{ns}:block/{p}/{name}"));
                }
            }
            for (key, names) in &s.variants.0 {
                let here = sat.key("variants").key(key);
                let mut when = Vec::new();
                for pair in key.split(',').filter(|p| !p.is_empty()) {
                    let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                    if states.iter().any(|(n, vs)| n == k && vs.iter().any(|x| x == v)) {
                        when.push((k.to_string(), v.to_string()));
                    } else {
                        cx.err_at(loc, &here, format!("`{pair}`: no such property and value"));
                    }
                }
                let mut models = Vec::new();
                for name in &names.0 {
                    match named.get(name) {
                        Some(m) => models.push(m.clone()),
                        None => cx.err_at(loc, &here, format!("`{name}`: not in states.models")),
                    }
                }
                if names.0.is_empty() {
                    cx.err_at(loc, &here, "a model name, or a list of them");
                } else if !models.is_empty() {
                    rules.push((when, models));
                }
            }
        }
        let base: Value = match rotations.len() {
            0 => variant(&main, 0.0, 0.0, false, None),
            1 => rotations.remove(0),
            _ => Value::Array(rotations),
        };
        let uvlock = parts.states.as_ref().is_some_and(|s| s.uvlock);
        let blockstate = match &arm {
            // Every state: its model, turned by facing and face.
            None if !states.is_empty() => {
                let mut variants = Map::new();
                let mut combo: Vec<usize> = vec![0; states.len()];
                loop {
                    let at: Vec<(&str, &str)> = states.iter().zip(&combo).map(|((n, vs), &i)| (n.as_str(), vs[i].as_str())).collect();
                    let value = |k: &str| at.iter().find(|(n, _)| *n == k).map(|(_, v)| *v);
                    let models = rules
                        .iter()
                        .find(|(when, _)| when.iter().all(|(k, v)| value(k) == Some(v.as_str())))
                        .map_or(vec![main.clone()], |(_, m)| m.clone());
                    let (x, y) = turn(value("facing"), value("face"));
                    let key: Vec<String> = at.iter().map(|(n, v)| format!("{n}={v}")).collect();
                    let picks: Vec<Value> = models.iter().map(|m| variant(m, x, y, uvlock, None)).collect();
                    let v = if picks.len() == 1 { picks.into_iter().next().unwrap() } else { Value::Array(picks) };
                    variants.insert(key.join(","), v);
                    // The next state, the last property fastest.
                    let mut i = states.len();
                    loop {
                        if i == 0 {
                            break;
                        }
                        i -= 1;
                        combo[i] += 1;
                        if combo[i] < states[i].1.len() {
                            break;
                        }
                        combo[i] = 0;
                    }
                    if combo.iter().all(|&c| c == 0) {
                        break;
                    }
                }
                json!({ "variants": variants })
            }
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
        Some(main)
    };

    // The item form: the block model, or its own.
    let item_model = match (&parts.item_model, item_parent) {
        (None, Some(parent)) => Some(json!({ "parent": parent })),
        (None, None) => {
            cx.err_at(loc, &root, format!("kind {} has no item model of its own — give item_model", parts.kind.as_deref().unwrap_or_default()));
            None
        }
        (Some(im), _) => model(cx, loc, &root.key("item_model"), im, true).map(|m| m.json),
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
        out.tags.entry(format!("data/{tns}/tags/block/{tpath}.json")).or_default().insert(id.clone(), true);
    }
    Some(entry)
}

/// The loot table dropping `item` (`count` of it), when it survives the explosion.
fn drops_table(item: &str, count: Option<yog_formats::content::DropCount>) -> Value {
    let mut entry = json!({ "type": "minecraft:item", "name": item });
    if let Some(c) = count {
        let count = match c {
            yog_formats::content::DropCount::Exactly(n) => num_value(n),
            yog_formats::content::DropCount::Between(min, max) => json!({ "type": "minecraft:uniform", "min": num_value(min), "max": num_value(max) }),
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

/// A block of one of the game's kinds: the kind's template models with the
/// file's textures, its states as the kind's, and the model its item shows
/// (none: the file gives it).
#[allow(clippy::too_many_arguments)]
fn kind_models(
    cx: &mut Ctx,
    loc: Loc,
    k: &yog_formats::content::BlockKind,
    m: &yog_formats::content::Model,
    base: Option<&str>,
    render_layer: Option<&str>,
    ns: &str,
    p: &str,
    emit: &mut dyn FnMut(&mut Ctx, String, &Value),
) -> Option<String> {
    let at = At::root().key("model");
    if m.parent.is_some() || m.elements.is_some() || m.display.is_some() || m.ambient_occlusion.is_some() || m.gui_light.is_some() {
        cx.err_at(loc, &at, format!("kind {} has the game's models — give only textures ({})", k.name, k.slots.join(", ")));
    }
    let mut textures = Map::new();
    for (slot, tex) in &m.textures.0 {
        let here = at.key("textures").key(slot);
        if slot != "particle" && !k.slots.contains(slot) {
            cx.warn_at(loc, &here, format!("kind {} has no slot `{slot}` ({})", k.name, k.slots.join(", ")));
        }
        if let Some(t) = cx.texture(loc, &here, tex) {
            textures.insert(slot.clone(), Value::String(t));
        }
    }
    for slot in k.slots.iter().filter(|s| !textures.contains_key(*s)) {
        cx.err_at(loc, &at, format!("kind {} needs textures.{slot}", k.name));
    }
    // `""` is the block's own model, `base` its base block's, another name `<block>_<name>`.
    let path = |name: &str| if name.is_empty() { p.to_string() } else { format!("{p}_{name}") };
    let id = |name: &str| match (name, base) {
        ("base", Some(b)) => {
            let (bns, bp) = b.split_once(':').unwrap_or(("minecraft", b));
            format!("{bns}:block/{bp}")
        }
        _ => format!("{ns}:block/{}", path(name)),
    };
    for (name, parent) in &k.models.0 {
        let mut json = json!({ "parent": parent, "textures": textures });
        if let (Some(layer), Value::Object(o)) = (render_layer, &mut json) {
            o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
        }
        emit(cx, format!("models/block/{}.json", path(name)), &json);
    }
    let shown = |km: &yog_formats::content::KindModel| variant(&id(&km.model), km.x.unwrap_or(0) as f64, km.y.unwrap_or(0) as f64, km.uvlock, None);
    let blockstate = if k.multipart.is_empty() {
        let variants: Map<String, Value> = k.variants.0.iter().map(|(state, km)| (state.clone(), shown(km))).collect();
        json!({ "variants": variants })
    } else {
        let parts: Vec<Value> = k
            .multipart
            .iter()
            .map(|part| {
                let mut o = Map::new();
                o.insert("apply".into(), shown(&part.apply));
                if let Some(when) = &part.when {
                    o.insert("when".into(), Value::Object(state_properties(when)));
                }
                Value::Object(o)
            })
            .collect();
        json!({ "multipart": parts })
    };
    emit(cx, format!("blockstates/{p}.json"), &blockstate);
    k.item.as_deref().map(id)
}

/// `half=lower,open=true` → `{"half": "lower", "open": "true"}`.
fn state_properties(state: &str) -> Map<String, Value> {
    state
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
        .collect()
}

/// What a block of a game's kind drops: itself — only in its `drops_when`
/// states (a door's bottom half), two in its `drops_two_when` (a double slab).
fn kind_drops(id: &str, k: &yog_formats::content::BlockKind) -> Value {
    let mut table = drops_table(id, None);
    let when = |state: &str| json!({ "condition": "minecraft:block_state_property", "block": id, "properties": state_properties(state) });
    let pool = &mut table["pools"][0];
    if let Some(state) = &k.drops_when
        && let Some(conditions) = pool["conditions"].as_array_mut()
    {
        conditions.insert(0, when(state));
    }
    if let Some(state) = &k.drops_two_when {
        // As the game's slab: two when double, and explosions take their share.
        pool.as_object_mut().map(|o| o.remove("conditions"));
        pool["entries"][0]["functions"] = json!([
            { "function": "minecraft:set_count", "count": 2, "conditions": [when(state)] },
            { "function": "minecraft:explosion_decay" }
        ]);
    }
    table
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
    /// The book as the runtime reads it (`yog_book::Book`'s schema), as
    /// binary Yog Data: the mod hands these bytes over as they are.
    data: Vec<u8>,
}

/// One checked `.yentity`.
struct EntityFile {
    id: String,
    file: String,
    /// Its model and clips, ids with their namespace.
    model: Option<String>,
    animations: Option<String>,
    /// `yog_registry::EntityDef`, as binary Yog Data.
    data: Vec<u8>,
}

impl EntityFile {
    fn path(&self) -> String {
        format!("entities/{}.ydata", self.id.replace([':', '/'], "__"))
    }
}

/// Checks a `.yentity`: its id (the file's name by default), its name into
/// the lang (`entity.<ns>.<path>`), the rest by `yog_registry::EntityDef`.
fn compile_entity(cx: &mut Ctx, text: &str, path: &Path, mod_ns: Option<&str>, out: &mut Compiled) -> Option<EntityFile> {
    use yog_data::Value;
    let mut value = match yog_data::text::parse_value(text) {
        Ok(v) => v,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let Value::Map(map) = &mut value else {
        cx.err(1, "an entity is an object: { \"kind\": \"mob\", \"width\": 0.6, \"height\": 1.8, … }");
        return None;
    };
    let given = map.get("id").and_then(Value::as_str).map(str::to_owned);
    let (ns, p) = file_id(cx, 1, given.as_deref(), path, mod_ns)?;
    let id = format!("{ns}:{p}");
    map.insert("id", Value::from(id.as_str()));
    // The name → lang, as a block's or an item's.
    if let Some(name) = map.remove("name") {
        let key = format!("entity.{ns}.{}", p.replace('/', "."));
        match yog_data::from_value::<yog_data::Text>(&name) {
            Ok(read) => match read.value {
                yog_data::Text::Literal(s) => {
                    out.lang.entry((ns.clone(), yres::NEUTRAL_LOCALE.to_string())).or_default().insert(key, s);
                }
                yog_data::Text::Lang(k) => out.lang_aliases.push((k, key)),
                yog_data::Text::NameOf(r) => {
                    let (rns, rp) = r.id.split_once(':').unwrap_or(("minecraft", &r.id));
                    out.lang_aliases.push((format!("{}.{rns}.{}", r.kind, rp.replace('/', ".")), key));
                }
                yog_data::Text::Res(r) => match cx.refs.texts.get(&format!("{}.{}", r.set, r.key)) {
                    Some((k, _)) => out.lang_aliases.push((k.clone(), key)),
                    None => cx.err(1, format!("name `@{}.{}`: no text `{}.{}` in resources/*.yres", r.set, r.key, r.set, r.key)),
                },
            },
            Err(problems) => {
                for pr in problems {
                    cx.err(1, format!("name: {}", pr.message));
                }
            }
        }
    }
    let def = match yog_data::from_value::<yog_registry::EntityDef>(&value) {
        Ok(read) => read.value,
        Err(problems) => {
            for pr in problems {
                cx.err(1, format!("{}: {}", pr.path, pr.message));
            }
            return None;
        }
    };
    let qualify = |s: &Option<String>| s.as_ref().map(|s| if s.contains(':') { s.clone() } else { format!("{ns}:{s}") });
    let mut def = def;
    def.model = qualify(&def.model);
    def.animations = qualify(&def.animations);
    Some(EntityFile {
        id,
        file: cx.file.clone(),
        model: def.model.clone(),
        animations: def.animations.clone(),
        data: yog_data::binary::encode(&yog_data::Schema::write(&def)),
    })
}

/// One checked `.ymodel` or `.yanim`.
struct BodyFile {
    model: bool,
    /// `<mod>:<path under content/ without the extension>`.
    id: String,
    /// As `yog_body` reads it: binary Yog Data.
    data: Vec<u8>,
}

/// One checked `.yfont`.
struct FontFile {
    /// `<mod>:<path under content/ without the extension>`.
    id: String,
    /// The built `.yfont`, as binary Yog Data.
    data: Vec<u8>,
}

impl FontFile {
    fn path(&self) -> String {
        format!("fonts/{}.yfont", self.id.replace([':', '/'], "__"))
    }
}

/// Checks a `.yfont` and stores it as binary Yog Data.
fn compile_font(shown: &str, text: &str, root: &Path, path: &Path, ns: &str, errors: &mut Vec<String>) -> Option<FontFile> {
    let font = match yog_data::from_text::<yog_font::Font>(text) {
        Ok(read) => read.value,
        Err(problems) => {
            for problem in problems.0 {
                errors.push(format!("{shown}: {problem}"));
            }
            return None;
        }
    };
    if let Err(problems) = font.validate() {
        errors.extend(problems.into_iter().map(|problem| format!("{shown}: {problem}")));
        return None;
    }
    let rel = path.strip_prefix(root.join(CONTENT_DIR)).unwrap_or(path).with_extension("");
    let id = format!("{ns}:{}", rel.to_string_lossy().replace('\\', "/"));
    Some(FontFile { id, data: font.to_bytes() })
}

impl BodyFile {
    /// Its file under `.yog-build/generated/`.
    fn path(&self) -> String {
        let dir = if self.model { "models" } else { "anims" };
        format!("body/{dir}/{}.ydata", self.id.replace([':', '/'], "__"))
    }
}

/// Checks a `.ymodel` (its schema, and that it makes a model: parents
/// before children, names once) or a `.yanim` (schema, times).
fn compile_body(shown: &str, text: &str, kind: Kind, root: &Path, path: &Path, ns: &str, errors: &mut Vec<String>) -> Option<BodyFile> {
    let rel = path.strip_prefix(root.join(CONTENT_DIR)).unwrap_or(path).with_extension("");
    let id = format!("{ns}:{}", rel.to_string_lossy().replace('\\', "/"));
    let report = |errors: &mut Vec<String>, e: yog_data::Errors| {
        for l in e.0 {
            errors.push(format!("{shown}:{l}"));
        }
    };
    let value = if kind == Kind::Model {
        let file = match yog_data::from_text::<yog_body::format::ModelFile>(text) {
            Ok(r) => r.value,
            Err(e) => {
                report(errors, e);
                return None;
            }
        };
        if let Err(e) = yog_body::Model::new(&file, ns) {
            errors.push(format!("{shown}: {e}"));
            return None;
        }
        yog_data::Schema::write(&file)
    } else {
        let file = match yog_data::from_text::<yog_body::format::AnimFile>(text) {
            Ok(r) => r.value,
            Err(e) => {
                report(errors, e);
                return None;
            }
        };
        if let Err(e) = yog_body::anim::Clips::new(&file) {
            errors.push(format!("{shown}: {e}"));
            return None;
        }
        yog_data::Schema::write(&file)
    };
    Some(BodyFile { model: kind == Kind::Model, id, data: yog_data::binary::encode(&value) })
}

/// A dimension's data file under `.yog-build/generated/`.
fn dimension_path(id: &str) -> String {
    format!("dimensions/{}.ydata", id.replace(':', "__"))
}

impl BookFile {
    /// Its file under `.yog-build/generated/`.
    fn path(&self) -> String {
        format!("books/{}.ydata", self.id.replace(':', "__"))
    }
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
    page_types: &[yog_book::page_type::PageType],
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
    // Pages of a mod's own kind: their fields checked by their type and
    // resolved like the rest of the book; the types they use go with it.
    let mut used: Vec<yog_book::page_type::PageType> = Vec::new();
    let mut fields_ok = true;
    for (e, entry) in book.entries.iter_mut().enumerate() {
        for (p, page) in entry.pages.iter_mut().enumerate() {
            let yog_book::BookPage::Custom(c) = page else { continue };
            use yog_book::page_type::PropKind as K;
            use yog_data::with::With;
            let at = Path::root().key("entries").index(e).key("pages").index(p);
            let line = |path: &Path| doc.locate(text, path).line as usize;
            let Some(t) = page_types.iter().find(|t| t.id == c.kind) else {
                if c.kind.split(':').next() == mod_ns {
                    cx.err(line(&at.key("type")), format!("{}.type: no page type {} — a .yui component with book_page=\"{}\"", at, c.kind, c.kind));
                    fields_ok = false;
                } else {
                    cx.warn(line(&at.key("type")), format!("{}.type: {} is another mod's page type — not checked here", at, c.kind));
                }
                continue;
            };
            for (k, _) in c.fields.iter() {
                if k != "anchor" && !t.properties.iter().any(|p| p.name == k) {
                    let names: Vec<&str> = t.properties.iter().map(|p| p.name.as_str()).collect();
                    cx.err(line(&at.key(k)), format!("{}: {} has no field `{k}` ({})", at.key(k), c.kind, names.join(", ")));
                    fields_ok = false;
                }
            }
            for prop in &t.properties {
                let path = at.key(&prop.name);
                let Some(v) = c.fields.get(&prop.name).cloned() else {
                    if prop.default.is_none() {
                        cx.err(line(&at), format!("{at}: missing `{}` ({})", prop.name, c.kind));
                        fields_ok = false;
                    }
                    continue;
                };
                let arena = yog_data::Bump::new();
                let r = v.borrow(&arena);
                let mut reader = yog_data::Reader::with_resolver(&mut check);
                let read = match prop.kind {
                    K::Text => yog_data::with::TextOf::read(&r, &mut reader).map(yog_data::Value::Str),
                    K::BookText => yog_book::refs::BookText::read(&r, &mut reader).map(yog_data::Value::Str),
                    K::Item => yog_data::with::RefTo::<(yog_data::kinds::Item, yog_data::kinds::Block)>::read(&r, &mut reader).map(yog_data::Value::Str),
                    K::Image => yog_book::refs::Icon::read(&r, &mut reader).map(yog_data::Value::Str),
                    K::Number => <f64 as yog_data::Schema>::read(&r, &mut reader).map(yog_data::Value::Float),
                    K::Bool => <bool as yog_data::Schema>::read(&r, &mut reader).map(yog_data::Value::Bool),
                    K::Data => Some(v.clone()),
                };
                let problems = std::mem::take(&mut reader.problems);
                drop(reader);
                for pr in problems {
                    cx.err(line(&path), format!("{path}: {}", pr.message));
                    fields_ok = false;
                }
                if let Some(out) = read {
                    c.fields.insert(prop.name.as_str(), out);
                }
            }
            if !used.contains(t) {
                used.push(t.clone());
            }
        }
    }
    if !fields_ok {
        return None;
    }
    book.page_types = used;
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
                let short = kind.split_once(':').map_or(kind.as_str(), |(_, k)| k);
                cx.err(
                    doc.locate(text, &path).line as usize,
                    format!("{path}: no layout for recipe kind {kind} — lay it out in a .yui: <Component recipe_layout=\"{short}\" width=\"…\" height=\"…\"> (see yog_book::layout)"),
                );
                ok = false;
            }
        }
    }
    if !ok {
        return None;
    }
    book.layouts = layouts.to_vec();
    Some(BookFile { id: book.id.clone(), data: yog_data::binary::encode(&yog_data::Schema::write(&book)) })
}

/// Checks one `.ydef` of kind `recipe_kind` or `recipe_layout`: the items
/// its layout draws exist, the kinds it lays out are the game's or the
/// mod's own. A kind of recipe, and the layout (of its kinds).
fn compile_layout(
    cx: &mut Ctx,
    text: &str,
    mod_ns: Option<&str>,
    known: &refs::Known,
    ores: &mut Vec<OreOut>,
    dimensions: &mut Vec<(String, Vec<u8>)>,
    tables: &mut Vec<table::Table>,
    loot: &mut Vec<(String, Vec<u8>)>,
    definitions: &mut Vec<(&'static str, String, Vec<u8>)>,
    advancements: &mut Vec<String>,
    out: &mut Compiled,
) -> Option<yog_formats::def::RecipeKindDef> {
    use yog_formats::def::Def;
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let read = match yog_data::schema::from_doc::<Def>(text, &doc) {
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
        // A tag's `optional` entries are other mods' — there or not.
        if matches!(read.value, Def::Tag(_)) && f.path.to_string().starts_with("optional") {
            continue;
        }
        if let Some(kind) = refs::RefKind::from_word(&f.value.kind)
            && let Err(e) = known.check(kind, &f.value.id)
        {
            cx.err(doc.locate(text, &f.path).line as usize, format!("{}: {e}", f.path));
            ok = false;
        }
    }
    for f in &read.tags {
        if matches!(read.value, Def::Tag(_)) && f.path.to_string().starts_with("optional") {
            continue;
        }
        if let Err(e) = known.check_tag(&f.value.kind, &f.value.id) {
            cx.err(doc.locate(text, &f.path).line as usize, format!("{}: {e}", f.path));
            ok = false;
        }
    }
    let ns = mod_ns.unwrap_or("minecraft");
    let kind = match read.value {
        Def::RecipeKind(mut k) => {
            if !k.id.contains(':') {
                k.id = format!("{ns}:{}", k.id);
            }
            if k.id.split_once(':').is_none_or(|(n, p)| n != ns || !valid_path(p)) {
                cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, format!("id: `{}`: the mod's own kind, `{ns}:<kind>`", k.id));
                ok = false;
            }
            k
        }
        Def::Ore(o) => {
            if ok {
                ores.push(ore(o, ns));
            }
            return None;
        }
        Def::Dimension(d) => {
            if ok {
                dimensions.push(dimension(d, ns));
            }
            return None;
        }
        Def::Tag(t) => {
            let line = doc.locate(text, &yog_data::Path::root().key("values")).line as usize;
            match tag_entries(&t, ns) {
                Ok((path, entries)) => out.tags.entry(path).or_default().extend(entries),
                Err(problems) => problems.into_iter().for_each(|p| cx.err(line, p)),
            }
            return None;
        }
        Def::DamageType(d) => {
            match damage_type(&d, ns) {
                Ok((file, tags)) => {
                    out.outputs.push(file);
                    for (path, id) in tags {
                        out.tags.entry(path).or_default().insert(id, true);
                    }
                }
                Err(e) => cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, e),
            }
            return None;
        }
        Def::Tree(t) => {
            match tree(&t, ns) {
                Ok(file) => {
                    if ok {
                        out.outputs.push(file);
                    }
                }
                Err(e) => cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, e),
            }
            return None;
        }
        Def::LootModifier(mut m) => {
            m.id = full_id(&m.id, ns);
            if m.id.split_once(':').is_none_or(|(n, p)| n != ns || !valid_path(p)) {
                cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, format!("id: `{}`: the mod's own, `{ns}:<name>`", m.id));
                ok = false;
            }
            if ok {
                use yog_data::Schema;
                loot.push((m.id.clone(), yog_data::binary::encode(&m.write())));
            }
            return None;
        }
        // A table of its own: an empty one in the datapack (so it exists for
        // `/loot` and other tables), filled by a modifier on it.
        Def::Advancement(a) => {
            let at = |key: &str| doc.locate(text, &yog_data::Path::root().key(key)).line as usize;
            match advancement(cx, &mut out.lang, &a, ns) {
                Ok(files) if ok => {
                    out.outputs.extend(files);
                    advancements.push(full_id(&a.id, ns));
                }
                Ok(_) => {}
                Err((key, e)) => cx.err(at(key), format!("{key}: {e}")),
            }
            return None;
        }
        // Registered with the game: the host reads them from the content.
        Def::Attribute(mut a) => {
            a.id = full_id(&a.id, ns);
            content_def(cx, &doc, text, ns, "attribute", a.id.clone(), &a, ok, definitions);
            return None;
        }
        Def::Effect(mut e) => {
            e.id = full_id(&e.id, ns);
            content_def(cx, &doc, text, ns, "effect", e.id.clone(), &e, ok, definitions);
            return None;
        }
        Def::LootTable(t) => {
            let id = full_id(&t.id, ns);
            let Some(path) = id.strip_prefix(&format!("{ns}:")).filter(|p| valid_path(p)) else {
                cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, format!("id: `{id}`: the mod's own, `{ns}:<name>`"));
                return None;
            };
            if ok {
                use yog_data::Schema;
                out.outputs.push((format!("data/{ns}/loot_table/{path}.json"), pretty(&json!({ "type": "minecraft:generic", "pools": [] }))));
                let fill = yog_registry::LootModifier { id: id.clone(), tables: vec![id.clone()], add: t.add, scale: Vec::new() };
                loot.push((id, yog_data::binary::encode(&fill.write())));
            }
            return None;
        }
        Def::Table(t) => {
            match table::check(&cx.file, t, cx.refs, known) {
                Ok(t) => tables.push(t),
                Err(errors) => {
                    for e in errors {
                        // `rows.wood.size: …` — at its line.
                        let path = e.split_once(':').map_or("", |(p, _)| p);
                        let at = path.split('.').fold(yog_data::Path::root(), |at, k| at.key(k));
                        cx.err(doc.locate(text, &at).line as usize, e);
                    }
                }
            }
            return None;
        }
        _ => return None,
    };
    ok.then_some(kind)
}

/// The builder calls (`.max_stack(16)`…) for an item's properties; empty
/// when it has none of its own.
fn item_props(p: &ItemProps) -> String {
    let mut b = String::new();
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
    if let Some((kind, material, d, s)) = &p.tool {
        let _ = write!(b, "\n            .tool({kind:?}, {material:?}, {d:?}, {s:?})");
    }
    if let Some(slot) = &p.wearable {
        let _ = write!(b, "\n            .wearable({slot:?})");
    }
    if let Some((entity, size)) = &p.hangs {
        let _ = write!(b, "\n            .hangs({entity:?}, {size:?})");
    }
    if let Some((slot, material)) = &p.armor {
        let _ = write!(b, "\n            .armor({slot:?}, {material:?})");
    }
    if let Some((n, s, always)) = p.food {
        let _ = write!(
            b,
            "\n            .food(::yog_api::FoodDef::new({n}, {}){})",
            float(s),
            if always { ".can_always_eat()" } else { "" }
        );
    }
    b
}

/// The translation key a text names (`@Lang.…`, `@lang:…`, an item's name),
/// or for words, `key` with the words put in the mod's lang.
fn text_key(cx: &mut Ctx, lang: &mut Lang, t: &yog_data::Text, ns: &str, key: String) -> Result<String, String> {
    Ok(match t {
        yog_data::Text::Literal(words) => {
            lang.entry((ns.to_string(), yres::NEUTRAL_LOCALE.to_string())).or_default().insert(key.clone(), words.clone());
            key
        }
        yog_data::Text::Lang(k) => k.clone(),
        yog_data::Text::NameOf(r) => {
            let (rns, rp) = r.id.split_once(':').unwrap_or(("minecraft", &r.id));
            format!("{}.{rns}.{}", r.kind, rp.replace('/', "."))
        }
        yog_data::Text::Res(r) => cx
            .refs
            .texts
            .get(&format!("{}.{}", r.set, r.key))
            .map(|(k, _)| k.clone())
            .ok_or_else(|| format!("`@{}.{}`: no text `{}.{}` in resources/*.yres", r.set, r.key, r.set, r.key))?,
    })
}

/// The mod's lang: (namespace, locale) → key → text.
type Lang = BTreeMap<(String, String), BTreeMap<String, String>>;

/// Minecraft versions before 1.20.5 write advancements the old way: an
/// icon's `item`, item predicates' `items` lists and `tag`s.
fn old_advancement_format(version: &str) -> bool {
    let mut p = version.split('.').map(|n| n.parse::<u32>().unwrap_or(0));
    let (major, minor, patch) = (p.next().unwrap_or(0), p.next().unwrap_or(0), p.next().unwrap_or(0));
    (major, minor, patch) < (1, 20, 5)
}

/// A `.ydef` advancement: its JSON in the newest format, and the old one as
/// an overlay for the versions that read that. `Err((field, why))`.
fn advancement(cx: &mut Ctx, lang: &mut Lang, a: &yog_formats::def::AdvancementDef, ns: &str) -> Result<Vec<Output>, (&'static str, String)> {
    use serde_json::json;
    let id = full_id(&a.id, ns);
    let path = id.strip_prefix(&format!("{ns}:")).filter(|p| valid_path(p)).ok_or(("id", format!("`{id}`: the mod's own, `{ns}:<name>`")))?.to_string();
    if !matches!(a.frame.as_str(), "task" | "goal" | "challenge") {
        return Err(("frame", format!("`{}`: task, goal or challenge", a.frame)));
    }
    let base_key = format!("advancement.{ns}.{}", path.replace('/', "."));
    let title = text_key(cx, lang, &a.title, ns, base_key.clone()).map_err(|e| ("title", e))?;
    let description = match &a.description {
        Some(d) => json!({ "translate": text_key(cx, lang, d, ns, format!("{base_key}.desc")).map_err(|e| ("description", e))? }),
        None => json!({ "text": "" }),
    };
    let make = |old: bool| -> Result<Value, (&'static str, String)> {
        let item_predicate = |item: &str| if old { json!({ "items": [item] }) } else { json!({ "items": item }) };
        let mut criteria = Map::new();
        let mut requirements = Vec::new();
        for (i, c) in a.criteria.iter().enumerate() {
            let kinds = c.granted as u8 + c.has_item.is_some() as u8 + c.used_item.is_some() as u8;
            if kinds != 1 {
                return Err(("criteria", format!("criteria[{i}]: one of granted, has_item, used_item")));
            }
            let name = format!("c{i}");
            let criterion = if c.granted {
                json!({ "trigger": "minecraft:impossible" })
            } else if let Some(item) = &c.has_item {
                let predicate = match item {
                    yog_data::OneOrTag::One(i) => item_predicate(&full_id(&i.id, "minecraft")),
                    yog_data::OneOrTag::Tag(t) if old => json!({ "tag": full_id(&t.id, "minecraft") }),
                    yog_data::OneOrTag::Tag(t) => json!({ "items": format!("#{}", full_id(&t.id, "minecraft")) }),
                };
                json!({ "trigger": "minecraft:inventory_changed", "conditions": { "items": [predicate] } })
            } else {
                let item = c.used_item.as_ref().map(|i| full_id(&i.id, "minecraft")).unwrap_or_default();
                json!({ "trigger": "minecraft:consume_item", "conditions": { "item": item_predicate(&item) } })
            };
            criteria.insert(name.clone(), criterion);
            requirements.push(json!([name]));
        }
        let mut display = json!({
            "icon": if old { json!({ "item": a.icon }) } else { json!({ "id": a.icon }) },
            "title": { "translate": title },
            "description": description,
            "frame": a.frame,
            "hidden": a.hidden,
            "show_toast": true,
            "announce_to_chat": true,
        });
        if let Some(bg) = &a.background {
            display["background"] = Value::String(bg.clone());
        }
        let mut v = json!({ "display": display, "criteria": criteria, "requirements": requirements });
        if let Some(parent) = &a.parent {
            v["parent"] = Value::String(full_id(parent, ns));
        }
        Ok(v)
    };
    let file = format!("data/{ns}/advancement/{path}.json");
    let mut out = vec![(file.clone(), pretty(&make(false)?))];
    for version in refs::versions().iter().filter(|v| old_advancement_format(v)) {
        out.push((format!("when/any/{version}/{file}"), pretty(&make(true)?)));
    }
    Ok(out)
}

/// A definition the game registers (`attribute`, `effect`, `potion`): the
/// mod's own id, kept as data for the code to register.
#[allow(clippy::too_many_arguments)]
fn content_def<T: yog_data::Schema>(
    cx: &mut Ctx,
    doc: &yog_data::text::Doc,
    text: &str,
    ns: &str,
    kind: &'static str,
    id: String,
    def: &T,
    ok: bool,
    definitions: &mut Vec<(&'static str, String, Vec<u8>)>,
) {
    if id.split_once(':').is_none_or(|(n, p)| n != ns || !valid_path(p)) {
        cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, format!("id: `{id}`: the mod's own, `{ns}:<name>`"));
        return;
    }
    if ok {
        definitions.push((kind, id, yog_data::binary::encode(&def.write())));
    }
}

/// A definition's file under `.yog-build/generated/`.
fn definition_path(kind: &str, id: &str) -> String {
    format!("{kind}/{}.ydata", id.replace([':', '/'], "__"))
}

/// A loot modifier's file under `.yog-build/generated/`.
fn loot_path(id: &str) -> String {
    format!("loot/{}.ydata", id.replace([':', '/'], "__"))
}

/// `ns:path` for an id the mod writes bare (`water_plants`).
fn full_id(id: &str, ns: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("{ns}:{id}") }
}

/// A deferred check of a tag of content `<kinds>`: `tag:item`, `tag:block|item`.
const TAG_REF: &str = "tag:";

/// A `.ydef` tag: where it's written and its entries (`optional` ones
/// kept only if present). What it names was checked with the file's refs.
fn tag_entries(t: &yog_formats::def::TagDef, ns: &str) -> Result<(String, BTreeMap<String, bool>), Vec<String>> {
    use yog_data::AnyRefOrTag;
    let id = full_id(&t.id, ns);
    let (tns, tpath) = id.split_once(':').unwrap_or(("minecraft", &id));
    let mut problems = Vec::new();
    let mut entries = BTreeMap::new();
    for (list, required) in [(&t.values, true), (&t.optional, false)] {
        for v in list {
            if v.kind() != t.of {
                problems.push(format!("`{v}` in a tag of {}", t.of));
                continue;
            }
            let (entry, required) = match v {
                AnyRefOrTag::One(r) => (full_id(&r.id, "minecraft"), required),
                // A tag only some versions or loaders have: where it isn't, the rest still counts.
                AnyRefOrTag::Tag(r) => {
                    let tag = full_id(&r.id, "minecraft");
                    let everywhere = !matches!(refs::tag_hosts(refs::tag_registry(&t.of), &tag), Some(Some(_)));
                    (format!("#{tag}"), required && everywhere)
                }
            };
            entries.insert(entry, required);
        }
    }
    if problems.is_empty() {
        Ok((format!("data/{tns}/tags/{}/{tpath}.json", refs::tag_registry(&t.of)), entries))
    } else {
        Err(problems)
    }
}

/// A `.ydef` tree: the game's `minecraft:tree` configured feature.
fn tree(t: &yog_formats::def::TreeDef, ns: &str) -> Result<Output, String> {
    use serde_json::json;
    let id = full_id(&t.id, ns);
    let (tns, tpath) = id.split_once(':').unwrap_or((ns, &id));
    if tns != ns || !valid_path(tpath) {
        return Err(format!("id: `{}`: the mod's own tree, `{ns}:<name>`", t.id));
    }
    // One block, or blocks by weight.
    let provider = |blocks: &[yog_formats::def::WeightedBlock]| match blocks {
        [one] => json!({ "type": "minecraft:simple_state_provider", "state": { "Name": one.block.id } }),
        many => json!({
            "type": "minecraft:weighted_state_provider",
            "entries": many.iter().map(|b| json!({ "data": { "Name": b.block.id }, "weight": b.weight })).collect::<Vec<_>>()
        }),
    };
    // The game's placers by their short name (`fancy`), a mod's by id.
    let placer = |shape: &str, what: &str| if shape.contains(':') { shape.to_string() } else { format!("minecraft:{shape}_{what}_placer") };
    if t.trunk.blocks.is_empty() || t.leaves.blocks.is_empty() {
        return Err("a tree needs trunk and leaves blocks".into());
    }
    let mut foliage = json!({ "type": placer(&t.leaves.shape, "foliage"), "radius": t.leaves.radius, "offset": t.leaves.offset });
    if let Some(h) = t.leaves.height {
        foliage["height"] = h.into();
    }
    let mut size = json!({ "type": "minecraft:two_layers_feature_size", "limit": 1, "lower_size": 0, "upper_size": 1 });
    if let Some(h) = t.min_clipped_height {
        size = json!({ "type": "minecraft:two_layers_feature_size", "limit": 0, "lower_size": 0, "upper_size": 0, "min_clipped_height": h });
    }
    let dirt = t.dirt.as_ref().map_or("minecraft:dirt", |b| b.id.as_str());
    let v = json!({
        "type": "minecraft:tree",
        "config": {
            "trunk_provider": provider(&t.trunk.blocks),
            "trunk_placer": { "type": placer(&t.trunk.shape, "trunk"), "base_height": t.trunk.base_height, "height_rand_a": t.trunk.random_a, "height_rand_b": t.trunk.random_b },
            "foliage_provider": provider(&t.leaves.blocks),
            "foliage_placer": foliage,
            "dirt_provider": { "type": "minecraft:simple_state_provider", "state": { "Name": dirt } },
            "minimum_size": size,
            "decorators": [],
            "ignore_vines": t.ignore_vines,
            "force_dirt": t.force_dirt
        }
    });
    Ok((format!("data/{tns}/worldgen/configured_feature/{tpath}.json"), pretty(&v)))
}

/// A `.ydef` damage type: its `damage_type/` file, and the damage type
/// tags it joins (path, its id).
fn damage_type(d: &yog_formats::def::DamageTypeDef, ns: &str) -> Result<(Output, Vec<(String, String)>), String> {
    let id = full_id(&d.id, ns);
    let (dns, dpath) = id.split_once(':').unwrap_or((ns, &id));
    if dns != ns || !valid_path(dpath) {
        return Err(format!("id: `{}`: the mod's own damage type, `{ns}:<name>`", d.id));
    }
    let mut v = serde_json::json!({
        "message_id": d.message_id.clone().unwrap_or_else(|| format!("{dns}.{dpath}")),
        "scaling": d.scaling,
        "exhaustion": d.exhaustion,
    });
    if let Some(e) = &d.effects {
        v["effects"] = Value::String(e.clone());
    }
    if let Some(m) = &d.death_message_type {
        v["death_message_type"] = Value::String(m.clone());
    }
    let tags = d
        .tags
        .iter()
        .map(|t| {
            let t = full_id(t, "minecraft");
            let (tns, tpath) = t.split_once(':').unwrap_or(("minecraft", &t));
            (format!("data/{tns}/tags/damage_type/{tpath}.json"), id.clone())
        })
        .collect();
    Ok(((format!("data/{dns}/damage_type/{dpath}.json"), pretty(&v)), tags))
}

/// The mod's recipe layouts (`.yui` `<Component recipe_layout="…">`),
/// checked: each kind the game's or one the mod declares, laid out once;
/// a mod kind's slots its own; stations known items.
fn recipe_layouts(
    root: &Path,
    ns: &str,
    kinds: &HashMap<String, yog_formats::def::RecipeKindDef>,
    known: &refs::Known,
    errors: &mut Vec<String>,
) -> Vec<yog_book::layout::RecipeLayout> {
    use yog_book::layout::LayoutPart;
    let mut laid_out: HashMap<String, String> = HashMap::new();
    let mut out = Vec::new();
    for (shown, l) in crate::book_page::find_layouts(root, ns, errors) {
        let mut ok = true;
        for k in &l.kinds {
            let game = yog_formats::recipe::TYPES.contains(&k.as_str());
            if !game && !kinds.contains_key(k) {
                errors.push(format!(
                    "{shown}: recipe_layout: `{k}`: one of {}, or a kind the mod declares (a .ydef of kind recipe_kind)",
                    yog_formats::recipe::TYPES.join(", ")
                ));
                ok = false;
            }
            if let Some(other) = laid_out.insert(k.clone(), shown.clone()) {
                errors.push(format!("{shown}: recipe kind {k} is also laid out by {other}"));
                ok = false;
            }
        }
        for part in &l.parts {
            match part {
                LayoutPart::Slots { slot, .. } => {
                    for k in l.kinds.iter().filter_map(|k| kinds.get(k)) {
                        if !k.slots.0.iter().any(|(s, _)| s == slot) {
                            let all: Vec<&str> = k.slots.0.iter().map(|(s, _)| s.as_str()).collect();
                            errors.push(format!("{shown}: <RecipeSlot slot=\"{slot}\">: {} has no slot `{slot}` — {}", k.id, all.join(", ")));
                            ok = false;
                        }
                    }
                }
                LayoutPart::Item { item, .. } => {
                    if let Err(e) = known.check(refs::RefKind::Item, item) {
                        errors.push(format!("{shown}: <ItemSlot item=\"{item}\">: {e}"));
                        ok = false;
                    }
                }
                _ => {}
            }
        }
        if ok {
            out.push(l);
        }
    }
    out
}

/// A dimension as the runtime and the hosts read it (Yog Data, binary):
/// its type, biome, flags, the generator that makes its chunks — its
/// `terrain`'s (the runtime's own, under the dimension's id) or the mod's.
fn dimension(d: yog_formats::def::DimensionDef, ns: &str) -> (String, Vec<u8>) {
    use yog_data::{Map, Schema, Value as V};
    let id = if d.id.contains(':') { d.id.clone() } else { format!("{ns}:{}", d.id) };
    let mut m = Map::new();
    m.insert("id", V::from(id.as_str()));
    m.insert("dimension_type", d.dimension_type.write());
    m.insert("biome", V::from(d.biome.map_or("minecraft:plains".to_string(), |b| b.id).as_str()));
    m.insert("structures", V::Bool(d.structures));
    m.insert("features", V::Bool(d.features));
    m.insert("mob_spawning", V::Bool(d.mob_spawning));
    if let Some(t) = &d.terrain {
        m.insert("generator_type", V::from(id.as_str()));
        m.insert("terrain", t.write());
    } else if let Some(g) = &d.generator {
        m.insert("generator_type", V::from(if g.contains(':') { g.clone() } else { format!("{ns}:{g}") }.as_str()));
    }
    (id, yog_data::binary::encode(&V::Map(m)))
}

/// What an ore makes: its files, and the biome feature (feature, biomes, step) for Fabric's host.
type OreOut = (Vec<Output>, (String, String, String));

/// An ore: its configured and placed features (the same on every version),
/// the biome modifiers Forge and NeoForge read (each its own), and the
/// biome feature the build registers for Fabric's host.
fn ore(o: yog_formats::def::OreDef, ns: &str) -> OreOut {
    use serde_json::json;
    let p = o.id.split_once(':').map_or(o.id.as_str(), |(_, p)| p).to_string();
    let id = format!("{ns}:{p}");
    let targets: Vec<Value> = o
        .targets
        .iter()
        .map(|t| {
            let target = match t.replaces.strip_prefix('#') {
                Some(tag) => json!({ "predicate_type": "minecraft:tag_match", "tag": tag }),
                None => json!({ "predicate_type": "minecraft:block_match", "block": t.replaces }),
            };
            json!({ "target": target, "state": { "Name": t.block.id } })
        })
        .collect();
    let configured = json!({
        "type": "minecraft:ore",
        "config": { "size": o.size, "discard_chance_on_air_exposure": o.air_exposure, "targets": targets }
    });
    let shape = match o.height.shape {
        yog_formats::def::HeightShape::Uniform => "minecraft:uniform",
        yog_formats::def::HeightShape::Triangle => "minecraft:trapezoid",
    };
    let placed = json!({
        "feature": id,
        "placement": [
            { "type": "minecraft:count", "count": o.per_chunk },
            { "type": "minecraft:in_square" },
            { "type": "minecraft:height_range", "height": { "type": shape, "min_inclusive": { "absolute": o.height.min }, "max_inclusive": { "absolute": o.height.max } } },
            { "type": "minecraft:biome" }
        ]
    });
    let biomes = o.biomes.map(|b| b.id).unwrap_or_else(|| "minecraft:is_overworld".into());
    let modifier = |kind: &str| json!({ "type": kind, "biomes": format!("#{biomes}"), "features": id, "step": "underground_ores" });
    let mut files: Vec<Output> = Vec::new();
    let mut put = |path: String, v: Value| files.push((path, pretty(&v)));
    put(format!("data/{ns}/worldgen/configured_feature/{p}.json"), configured);
    put(format!("data/{ns}/worldgen/placed_feature/{p}.json"), placed);
    for (dir, kind) in [("when/forge/any", "forge:add_features"), ("when/neoforge/1.20", "forge:add_features")] {
        put(format!("{dir}/data/{ns}/forge/biome_modifier/{p}.json"), modifier(kind));
    }
    put(format!("when/neoforge/1.21/data/{ns}/neoforge/biome_modifier/{p}.json"), modifier("neoforge:add_features"));
    (files, (id, biomes, "underground_ores".into()))
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
    /// Mods it needs (`requires`): registered only when they're all loaded.
    requires: Vec<String>,
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
fn compile_recipe(
    cx: &mut Ctx,
    text: &str,
    path: &Path,
    mod_ns: Option<&str>,
    known: &refs::Known,
    kinds: &HashMap<String, yog_formats::def::RecipeKindDef>,
    brews: &mut Vec<(String, yog_registry::Brew)>,
) -> Option<Recipe> {
    use yog_formats::recipe::{Body, RecipeFile};
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
    for f in &read.tags {
        if let Err(e) = known.check_tag(&f.value.kind, &f.value.id) {
            cx.err(line(&f.path), format!("{}: {e}", f.path));
            ok = false;
        }
    }
    // What's only on some versions or loaders keeps the recipe out of the others.
    if ok {
        for (missing, why) in recipe_gaps(&read.refs, &read.tags) {
            cx.warn(line(&At::root()), format!("left out on {}: {why}", refs::describe_hosts(&missing)));
        }
    }
    let RecipeFile { common: c, body } = read.value;
    let ty = match &body {
        Body::Shaped(_) => "shaped".to_string(),
        Body::Shapeless(_) => "shapeless".to_string(),
        Body::Cooking(t, _) | Body::Custom(t, _) => t.clone(),
        Body::Stonecutting(_) => "stonecutting".to_string(),
        Body::Smithing(_) => "smithing".to_string(),
        Body::Brewing(_) => "brewing".to_string(),
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
        Body::Brewing(k) => {
            if c.count != 1 {
                cx.err(line(&root.key("count")), "brewing recipes make one potion");
                ok = false;
            }
            if c.category.is_some() {
                cx.err(line(&root.key("category")), "brewing recipes don't have a recipe-book category");
                ok = false;
            }
            if ok {
                brews.push((c.result.clone(), yog_registry::Brew { from: k.from, with: k.ingredient }));
            }
            return None;
        }
        Body::Custom(kind, k) => {
            // The mod's own kind of recipe: `<mod id>:<kind>`.
            let own = mod_ns.is_some_and(|m| kind.split_once(':').is_some_and(|(n, p)| n == m && valid_path(p)));
            if !own {
                cx.err(
                    line(&root.key("type")),
                    format!("type `{kind}`: one of {}, or the mod's own kind `{}:<kind>`", yog_formats::recipe::TYPES.join(", "), mod_ns.unwrap_or("mymod")),
                );
                return None;
            }
            if c.category.is_some() {
                cx.err(line(&root.key("category")), format!("category is for the game's recipe types; {kind} takes \"slots\" and \"values\""));
                ok = false;
            }
            // What its kind declares (`.ydef` recipe_kind).
            let Some(def) = kinds.get(&kind) else {
                let file = kind.split_once(':').map_or(kind.as_str(), |(_, k)| k);
                cx.err(line(&root.key("type")), format!("type `{kind}`: no such kind — declare it: content/{file}.ydef, {{ kind: \"recipe_kind\", id: \"{file}\", slots: {{ … }} }}"));
                return None;
            };
            for (name, ings) in &k.slots.0 {
                let at = root.key("slots").key(name);
                match def.slots.0.iter().find(|(n, _)| n == name) {
                    None => {
                        let names: Vec<&str> = def.slots.0.iter().map(|(n, _)| n.as_str()).collect();
                        cx.err(line(&at), format!("slots.{name}: {kind} has no slot `{name}` ({})", names.join(", ")));
                        ok = false;
                    }
                    Some((_, spec)) if ings.len() > spec.max as usize => {
                        cx.err(line(&at), format!("slots.{name}: at most {} ingredient(s), found {}", spec.max, ings.len()));
                        ok = false;
                    }
                    _ => {}
                }
            }
            for (name, spec) in &def.slots.0 {
                if spec.required && !k.slots.0.iter().any(|(n, ings)| n == name && !ings.is_empty()) {
                    cx.err(line(&root.key("slots")), format!("slots: {kind} needs `{name}`"));
                    ok = false;
                }
            }
            for (name, v) in &k.values.0 {
                let at = root.key("values").key(name);
                let fits = match def.values.0.iter().find(|(n, _)| n == name).map(|(_, t)| *t) {
                    None => {
                        let names: Vec<&str> = def.values.0.iter().map(|(n, _)| n.as_str()).collect();
                        cx.err(line(&at), format!("values.{name}: {kind} has no value `{name}` ({})", names.join(", ")));
                        ok = false;
                        continue;
                    }
                    Some(yog_formats::def::ValueType::Number) => v.as_f64().is_some(),
                    Some(yog_formats::def::ValueType::Bool) => v.as_bool().is_some(),
                    Some(yog_formats::def::ValueType::Text) => v.as_str().is_some(),
                };
                if !fits {
                    cx.err(line(&at), format!("values.{name}: a {}", yog_formats::content::word(&def.values.0.iter().find(|(n, _)| n == name).unwrap().1)));
                    ok = false;
                }
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
        result: c.result,
        count: c.count,
        group: c.group,
        category: c.category,
        requires: c.requires,
    })
}

/// The hosts (`<loader>/<version>`) a recipe naming `refs` and `tags` can't
/// be on, with why: an item or tag one hasn't got. Alternatives (`["a",
/// "#b"]`, one list in the file) count as there when any of them is.
fn recipe_gaps(items: &[yog_data::schema::Found<yog_data::Ref>], tags: &[yog_data::schema::Found<yog_data::Ref>]) -> Vec<(BTreeSet<String>, String)> {
    use refs::RefKind;
    // Where each reference is (`None`: everywhere), grouped by the list it's in.
    let mut lists: BTreeMap<String, Vec<(String, Option<BTreeSet<String>>)>> = BTreeMap::new();
    let parent = |p: &yog_data::Path| {
        let mut p = p.clone();
        if matches!(p.segments().last(), Some(yog_data::Seg::Index(_))) {
            p.pop();
        }
        p.to_string()
    };
    for f in items {
        let Some(kind) = RefKind::from_word(&f.value.kind) else { continue };
        if !f.value.id.starts_with("minecraft:") {
            continue;
        }
        let on: BTreeSet<String> = refs::hosts()
            .iter()
            .filter(|h| h.split_once('/').is_some_and(|(_, v)| refs::equivalent(kind, &f.value.id, v).is_some()))
            .cloned()
            .collect();
        let on = (on.len() < refs::hosts().len()).then_some(on);
        lists.entry(parent(&f.path)).or_default().push((f.value.id.clone(), on));
    }
    for f in tags {
        let Some(on) = refs::tag_hosts(refs::tag_registry(&f.value.kind), &f.value.id) else { continue };
        lists.entry(parent(&f.path)).or_default().push((format!("#{}", f.value.id), on));
    }
    let mut out = Vec::new();
    for alts in lists.values() {
        if alts.iter().any(|(_, on)| on.is_none()) {
            continue;
        }
        let on: BTreeSet<String> = alts.iter().flat_map(|(_, on)| on.iter().flatten().cloned()).collect();
        let missing: BTreeSet<String> = refs::hosts().iter().filter(|h| !on.contains(*h)).cloned().collect();
        if missing.is_empty() {
            continue;
        }
        let names: Vec<&str> = alts.iter().map(|(id, _)| id.as_str()).collect();
        // A Yog tag that's everywhere, if one is these.
        let hint = alts
            .iter()
            .filter_map(|(id, _)| id.strip_prefix('#').and_then(|t| refs::yog_tag_for("item", t)))
            .find(|y| refs::tag_hosts("item", y) == Some(None))
            .map(|y| format!(" — @tag:item:{y} is on every version and loader"))
            .unwrap_or_default();
        out.push((missing, format!("{} isn't there{hint}", names.join(" / "))));
    }
    out
}

/// What the `.ydef`s define: every ladder, its levels bottom to top (the
/// game's tool ladder and the mod's), and the mod's sound groups.
struct Defs {
    order: BTreeMap<String, Vec<String>>,
    sound_groups: Vec<yog_formats::def::SoundGroup>,
    tool_materials: Vec<yog_formats::def::ToolMaterial>,
    armor_materials: Vec<yog_formats::def::ArmorMaterial>,
    /// Placed features the mod adds to biomes (its ores), for the hosts that add them in code.
    biome_features: Vec<(String, String, String)>,
    /// Its dimensions' ids (their data in `generated/dimensions/`).
    dimensions: Vec<String>,
    /// Its tables (`kind: "table"`), each an enum in its code.
    tables: Vec<table::Table>,
    /// Its loot modifiers' ids (their data in `generated/loot/`).
    loot_modifiers: Vec<String>,
    /// Its attributes, effects and potions: (kind, id).
    definitions: Vec<(&'static str, String)>,
    /// Its advancements' ids (their data in the datapack).
    advancements: Vec<String>,
}

impl Defs {
    /// A level's ladder and place on it.
    fn level(&self, id: &str) -> Option<(&str, usize)> {
        self.order.iter().find_map(|(l, levels)| levels.iter().position(|x| x == id).map(|i| (l.as_str(), i)))
    }
}

/// Reads the `.ydef`s: ladders, levels placed on them (`above`, in any
/// order of files) and sound groups.
fn read_defs(defs: &[(String, String)], mod_ns: Option<&str>, errors: &mut Vec<String>) -> Defs {
    let mut order: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut sound_groups: Vec<yog_formats::def::SoundGroup> = Vec::new();
    order.insert(yog_formats::def::TOOL_LADDER.into(), yog_formats::def::TOOL_LEVELS.iter().map(|l| format!("minecraft:{l}")).collect());
    let ns = mod_ns.unwrap_or("minecraft");
    let full = |id: &str| if id.contains(':') { id.to_string() } else { format!("{ns}:{id}") };
    // (file, line, id, ladder, above)
    let mut levels: Vec<(String, usize, String, String, Option<String>)> = Vec::new();
    let mut tools: Vec<(String, yog_formats::def::ToolMaterialDef)> = Vec::new();
    let mut armor_materials: Vec<yog_formats::def::ArmorMaterial> = Vec::new();
    let repair = |r: &yog_data::ItemIdOrTag| match r {
        yog_data::OneOrTag::One(i) => i.id.clone(),
        yog_data::OneOrTag::Tag(t) => format!("#{}", t.id),
    };
    for (file, text) in defs {
        let doc = match yog_data::text::parse(text) {
            Ok(d) => d,
            Err(e) => {
                errors.push(format!("{file}:{}: {}", e.at.line, e.message));
                continue;
            }
        };
        match yog_data::schema::from_doc::<yog_formats::def::Def>(text, &doc) {
            Ok(r) => match r.value {
                yog_formats::def::Def::Ladder { id } => {
                    if order.insert(full(&id), Vec::new()).is_some() {
                        errors.push(format!("{file}:1: ladder {} is defined twice", full(&id)));
                    }
                }
                yog_formats::def::Def::Level { id, ladder, above } => levels.push((file.clone(), doc.locate(text, &At::root().key("id")).line as usize, full(&id), ladder, above)),
                yog_formats::def::Def::SoundGroup(mut g) => {
                    // Short ids — the group's and its sounds' — are the mod's.
                    for s in [&mut g.id, &mut g.break_sound, &mut g.step, &mut g.place, &mut g.hit, &mut g.fall] {
                        *s = full(s);
                    }
                    if yog_formats::def::SoundGroup::built_in(&g.id).is_some() {
                        errors.push(format!("{file}:1: sound group {} is the game's", g.id));
                    } else if sound_groups.iter().any(|x| x.id == g.id) {
                        errors.push(format!("{file}:1: sound group {} is defined twice", g.id));
                    } else {
                        sound_groups.push(g);
                    }
                }
                // Read with the recipes (`compile_layout`).
                yog_formats::def::Def::RecipeKind(_) | yog_formats::def::Def::Ore(_) | yog_formats::def::Def::Dimension(_) | yog_formats::def::Def::Table(_) | yog_formats::def::Def::Tag(_) | yog_formats::def::Def::DamageType(_) | yog_formats::def::Def::Tree(_) | yog_formats::def::Def::LootModifier(_) | yog_formats::def::Def::LootTable(_) | yog_formats::def::Def::Attribute(_) | yog_formats::def::Def::Effect(_) | yog_formats::def::Def::Potion(_) | yog_formats::def::Def::Advancement(_) => {}
                // Its level is placed on the ladder below.
                yog_formats::def::Def::ToolMaterial(mut m) => {
                    m.id = full(&m.id);
                    tools.push((file.clone(), m));
                }
                yog_formats::def::Def::ArmorMaterial(m) => {
                    let id = full(&m.id);
                    if yog_formats::def::ArmorMaterial::built_in(&id).is_some() || armor_materials.iter().any(|x| x.id == id) {
                        errors.push(format!("{file}:1: armor material {id} is the game's or defined twice"));
                        continue;
                    }
                    armor_materials.push(yog_formats::def::ArmorMaterial {
                        texture: m.texture.as_deref().map(full).unwrap_or_else(|| id.clone()),
                        equip_sound: full(&m.equip_sound),
                        durability: m.durability,
                        defense: m.defense,
                        enchantability: m.enchantability,
                        toughness: m.toughness,
                        knockback_resistance: m.knockback_resistance,
                        repair: repair(&m.repair),
                        id,
                    });
                }
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
    // Tool materials: what their level is on each version — 1.20's level
    // number and 1.21's tag of what they can't mine, as blocks of their
    // level are tagged (`tool_level_tags`: the weakest of the game's levels
    // at or above it).
    let ladder = order.get(yog_formats::def::TOOL_LADDER).cloned().unwrap_or_default();
    let mut tool_materials: Vec<yog_formats::def::ToolMaterial> = Vec::new();
    for (file, m) in tools {
        if yog_formats::def::ToolMaterial::built_in(&m.id).is_some() || tool_materials.iter().any(|x| x.id == m.id) {
            errors.push(format!("{file}:1: tool material {} is the game's or defined twice", m.id));
            continue;
        }
        let Some(at) = ladder.iter().position(|l| l == &m.level) else {
            errors.push(format!("{file}:1: level: {} isn't a level of the tool ladder", m.level));
            continue;
        };
        let place = |v: &str| ladder.iter().position(|x| x == &format!("minecraft:{v}")).unwrap_or(0);
        let game = ["wood", "stone", "iron", "diamond", "netherite"];
        let (level_index, incorrect) = match game.iter().position(|v| place(v) >= at) {
            Some(i) => (i as u32, format!("minecraft:incorrect_for_{}_tool", if game[i] == "wood" { "wooden" } else { game[i] })),
            // Above netherite: a tag of its own (the build writes it).
            None => {
                let (n, p) = m.level.split_once(':').unwrap_or(("minecraft", &m.level));
                (5, format!("{n}:incorrect_for_{p}_tool"))
            }
        };
        tool_materials.push(yog_formats::def::ToolMaterial {
            id: m.id,
            durability: m.durability,
            speed: m.speed,
            attack_damage: m.attack_damage,
            enchantability: m.enchantability,
            level: m.level,
            repair: repair(&m.repair),
            level_index,
            incorrect,
        });
    }
    Defs { order, sound_groups, tool_materials, armor_materials, biome_features: Vec::new(), dimensions: Vec::new(), tables: Vec::new(), loot_modifiers: Vec::new(), definitions: Vec::new(), advancements: Vec::new() }
}

/// The tags that make block `id` need tool level `level`, on every version:
/// the game's `needs_*_tool` where they say it, and — for a level above
/// diamond — each version's own way (`incorrect_for_*_tool` on 1.21,
/// Fabric's `needs_tool_level_N` and Forge's `needs_netherite_tool` on 1.20).
fn tool_level_tags(id: &str, level: &str, ladders: &Defs, out: &mut Compiled, err: &mut dyn FnMut(String), warn: &mut dyn FnMut(String)) {
    let Some((ladder, at)) = ladders.level(level) else { return };
    if ladder != yog_formats::def::TOOL_LADDER {
        err(format!("{level} is a level of {ladder}, not of the tool ladder ({})", yog_formats::def::TOOL_LADDER));
        return;
    }
    let tools = &ladders.order[yog_formats::def::TOOL_LADDER];
    let place = |l: &str| tools.iter().position(|x| x == &format!("minecraft:{l}")).unwrap_or(0);
    // The weakest of the game's tools that mines it.
    let able = ["wood", "stone", "iron", "diamond", "netherite"].into_iter().find(|v| place(v) >= at);
    let mut tag = |path: String| {
        out.tags.entry(path).or_default().insert(id.to_string(), true);
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
    let mut bodies: Vec<BodyFile> = Vec::new();
    let mut fonts: Vec<FontFile> = Vec::new();
    let mut font_ids: HashMap<String, String> = HashMap::new();
    let mut entity_files: Vec<EntityFile> = Vec::new();
    let mut body_ids: HashMap<(bool, String), String> = HashMap::new();
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
        if matches!(kind, Kind::Model | Kind::Anim) {
            if let Some(b) = compile_body(&shown, &text, kind, root, path, mod_ns.unwrap_or("minecraft"), &mut errors) {
                if let Some(other) = body_ids.insert((kind == Kind::Model, b.id.clone()), shown.clone()) {
                    errors.push(format!("{shown}: {} {} is also defined by {other}", kind.word(), b.id));
                }
                bodies.push(b);
            }
            continue;
        }
        if kind == Kind::Font {
            if let Some(font) = compile_font(&shown, &text, root, path, mod_ns.unwrap_or("minecraft"), &mut errors) {
                if let Some(other) = font_ids.insert(font.id.clone(), shown.clone()) {
                    errors.push(format!("{shown}: font {} is also defined by {other}", font.id));
                }
                fonts.push(font);
            }
            continue;
        }
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
        if kind == Kind::Def {
            // Kinds of recipe are checked with the recipes, ores and dimensions with what they name.
            let of = yog_data::text::parse_value(&text).ok().and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_string));
            if matches!(of.as_deref(), Some("recipe_kind" | "ore" | "dimension" | "table" | "tag" | "damage_type" | "tree" | "loot_modifier" | "loot_table" | "attribute" | "effect" | "advancement")) {
                pending_layouts.push((shown.clone(), text));
            } else {
                pending_defs.push((shown.clone(), text));
            }
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
        if kind == Kind::Entity {
            if let Some(e) = compile_entity(&mut cx, &text, path, mod_ns, &mut out) {
                if let Some(other) = ids.insert(e.id.clone(), shown.clone()) {
                    errors.push(format!("{shown}: id {} is also defined by {other}", e.id));
                }
                entity_files.push(e);
            }
            continue;
        }
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
    let mut potion_defs: BTreeMap<String, yog_registry::PotionDef> = entries
        .iter()
        .filter_map(|e| e.potion.clone().map(|p| (e.id.clone(), p)))
        .collect();
    entries.retain(|e| e.potion.is_none());
    // Books name the mod's content, vanilla's and other mods' declared ids.
    let manifest = crate::manifest::YogToml::read(&root.join("yog.toml")).ok();
    let externals = manifest.as_ref().map(|t| t.externals.clone()).unwrap_or_default();
    let datapack = manifest.as_ref().and_then(|t| t.datapack.clone());
    let mut known = refs::Known::new(mod_ns.unwrap_or("minecraft"), &externals, datapack.as_deref());
    for e in &entries {
        known.add(if e.kind == Kind::Block { refs::RefKind::Block } else { refs::RefKind::Item }, e.id.clone());
    }
    for id in potion_defs.keys() {
        known.add(refs::RefKind::Potion, id.clone());
    }
    for e in &entity_files {
        known.add(refs::RefKind::Entity, e.id.clone());
        // A model or clips of this mod must be among its `.ymodel`/`.yanim`.
        for (wanted, model, what) in [(&e.model, true, "model"), (&e.animations, false, "animations")] {
            if let Some(id) = wanted
                && mod_ns.is_some_and(|ns| id.starts_with(&format!("{ns}:")))
                && !bodies.iter().any(|b| b.model == model && &b.id == id)
            {
                errors.push(format!("{}: {what} `{id}`: no such file in content/ (`{}`)", e.file, if model { ".ymodel" } else { ".yanim" }));
            }
        }
    }
    // Kinds of recipe (`.ydef`): what they name checked; a kind declared
    // twice is a mistake.
    let mut recipe_defs: HashMap<String, yog_formats::def::RecipeKindDef> = HashMap::new();
    let mut ores: Vec<OreOut> = Vec::new();
    let mut dimensions: Vec<(String, Vec<u8>)> = Vec::new();
    let mut tables: Vec<table::Table> = Vec::new();
    // Loot modifiers: (id, their data).
    let mut loot: Vec<(String, Vec<u8>)> = Vec::new();
    // Attributes, effects, potions: (kind, id, their data).
    let mut definitions: Vec<(&'static str, String, Vec<u8>)> = Vec::new();
    let mut brews: Vec<(String, yog_registry::Brew)> = Vec::new();
    let mut advancements: Vec<String> = Vec::new();
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
        let Some(k) = compile_layout(&mut cx, text, mod_ns, &known, &mut ores, &mut dimensions, &mut tables, &mut loot, &mut definitions, &mut advancements, &mut out) else { continue };
        if recipe_defs.contains_key(&k.id) {
            errors.push(format!("{shown}: recipe kind {} is declared twice", k.id));
        }
        recipe_defs.insert(k.id.clone(), k);
    }
    // How kinds of recipe are shown: `.yui` `recipe_layout` components.
    let layouts = recipe_layouts(root, mod_ns.unwrap_or("minecraft"), &recipe_defs, &known, &mut errors);
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
        if let Some(r) = compile_recipe(&mut cx, text, path, mod_ns, &known, &recipe_defs, &mut brews) {
            if let Some(other) = recipe_ids.insert(r.id.clone(), shown.clone()) {
                errors.push(format!("{shown}: recipe {} is also defined by {other}", r.id));
            }
            recipes.push(r);
        }
    }
    for r in &recipes {
        known.add(refs::RefKind::Recipe, r.id.clone());
    }
    for (to, brew) in brews {
        if let Some(potion) = potion_defs.get_mut(&to) {
            potion.brewed.push(brew);
        } else {
            errors.push(format!("brewing recipe result `{to}`: no potion `.yitem` defines it"));
        }
    }
    // Definitions: the ladders and their levels, sound groups.
    let mut defs = read_defs(&pending_defs, mod_ns, &mut errors);
    definitions.extend(potion_defs.into_iter().map(|(id, potion)| ("potion", id, yog_data::binary::encode(&potion.write()))));
    for (files, feature) in ores {
        out.outputs.extend(files);
        defs.biome_features.push(feature);
    }
    // What blocks and items name (drops, levels): known now.
    for (file, line, path, r) in &deferred {
        let found = match r.kind.as_str() {
            "level" => defs.level(&r.id).map(|_| ()).ok_or_else(|| format!("no level {} — the game's tool levels or a .ydef's", r.id)),
            "ladder" => defs.order.contains_key(&r.id).then_some(()).ok_or_else(|| format!("no ladder {}", r.id)),
            "tool_material" => (defs.tool_materials.iter().any(|m| m.id == r.id) || yog_formats::def::ToolMaterial::built_in(&r.id).is_some())
                .then_some(())
                .ok_or_else(|| format!("no tool material {} — the game's or a .ydef's", r.id)),
            "armor_material" => (defs.armor_materials.iter().any(|m| m.id == r.id) || yog_formats::def::ArmorMaterial::built_in(&r.id).is_some())
                .then_some(())
                .ok_or_else(|| format!("no armor material {} — the game's or a .ydef's", r.id)),
            "sound_group" => (defs.sound_groups.iter().any(|g| g.id == r.id) || yog_formats::def::SoundGroup::built_in(&r.id).is_some())
                .then_some(())
                .ok_or_else(|| format!("no sound group {} — the game's or a .ydef's", r.id)),
            kind if kind.starts_with(TAG_REF) => {
                // `tag:block|item`: a tag of either.
                let mut found = kind[TAG_REF.len()..].split('|').map(|k| known.check_tag(k, &r.id));
                let first = found.next().unwrap_or(Ok(()));
                if first.is_ok() || found.any(|f| f.is_ok()) { Ok(()) } else { first }
            }
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
            tool_level_tags(&e.id, level, &defs, &mut out, &mut |m| errors.push(format!("{}:{line}: block.tool_level: {m}", e.file)), &mut |m| warnings.push(format!("{}:{line}: block.tool_level: {m}", e.file)));
        }
    }
    let recipe_kinds: HashMap<String, String> = recipes.iter().map(|r| (r.id.clone(), r.ty.clone())).collect();
    // The mod's own kinds of book page (`.yui` components with `book_page`).
    let page_types = crate::book_page::find(root, mod_ns.unwrap_or("minecraft"), &mut errors);
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
        if let Some(b) = compile_book(&mut cx, text, mod_ns, &known, &layouts, &recipe_kinds, &page_types) {
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
    if (entries.iter().any(|e| e.auto) || !recipes.is_empty() || !books.is_empty() || !bodies.is_empty() || !entity_files.is_empty() || !fonts.is_empty() || !defs.sound_groups.is_empty() || !defs.tool_materials.is_empty() || !defs.armor_materials.is_empty() || !defs.biome_features.is_empty() || !dimensions.is_empty() || !loot.is_empty() || !definitions.is_empty())
        && !mentions_content_macro(&root.join("src"))
    {
        out.warnings.push(format!(
            "{} has blocks/items, but src/ never calls `yog_api::content!();` — they get models but aren't registered",
            CONTENT_DIR
        ));
    }
    out.data_files = books.iter().map(|b| (b.path(), b.data.clone())).collect();
    out.data_files.extend(bodies.iter().map(|b| (b.path(), b.data.clone())));
    out.data_files.extend(fonts.iter().map(|f| (f.path(), f.data.clone())));
    out.data_files.extend(entity_files.iter().map(|e| (e.path(), e.data.clone())));
    out.data_files.extend(dimensions.iter().map(|(id, data)| (dimension_path(id), data.clone())));
    out.data_files.extend(loot.iter().map(|(id, data)| (loot_path(id), data.clone())));
    out.data_files.extend(definitions.iter().map(|(kind, id, data)| (definition_path(kind, id), data.clone())));
    defs.definitions = definitions.iter().map(|(kind, id, _)| (*kind, id.clone())).collect();
    defs.loot_modifiers = loot.into_iter().map(|(id, _)| id).collect();
    defs.advancements = advancements;
    // A tool level above netherite gets a tag of what its tools can't mine.
    for m in &defs.tool_materials {
        if let Some((n, p)) = m.incorrect.split_once(':')
            && n != "minecraft"
        {
            out.tags.entry(format!("when/any/1.21/data/{n}/tags/block/{p}.json")).or_default();
        }
    }
    defs.dimensions = dimensions.iter().map(|(id, _)| id.clone()).collect();
    defs.tables = tables;
    out.source = Some(rust_source(&entries, &recipes, &books, &bodies, &entity_files, &fonts, &defs));
    version_overlays(&mut out, &known.datapack);
    out.registrations = registrations(&entries, &entity_files, &dimensions, &defs);
    Ok(out)
}

/// [`yog_registry::REGISTRATIONS_FILE`]: what the mod registers at the
/// game's start, each with a hash of what can't change while it runs — a
/// block's properties and states, not its model, name or tags (the game
/// reloads those).
fn registrations(entries: &[Entry], entities: &[EntityFile], dimensions: &[(String, Vec<u8>)], defs: &Defs) -> String {
    fn hash(text: &str) -> String {
        // FNV-1a: the same on every build of the CLI.
        let h = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        format!("{h:016x}")
    }
    let mut lines = Vec::new();
    for e in entries {
        let what = match e.kind {
            Kind::Block => {
                // Its tool tags and level are data (tags), reloaded with the rest.
                let b = BlockProps { tools: Vec::new(), tool_level: None, ..e.block.clone() };
                format!("{b:?} {:?} {:?}", e.item, e.requires)
            }
            _ => format!("{:?} {:?}", e.item, e.requires),
        };
        lines.push(format!("{} {} {}", e.kind.word(), e.id, hash(&what)));
    }
    for e in entities {
        lines.push(format!("entity {} {}", e.id, hash(&format!("{:?}", e.data))));
    }
    for (id, data) in dimensions {
        lines.push(format!("dimension {id} {}", hash(&format!("{data:?}"))));
    }
    for g in &defs.sound_groups {
        lines.push(format!("sound_group {} {}", g.id, hash(&format!("{g:?}"))));
    }
    for m in &defs.tool_materials {
        lines.push(format!("tool_material {} {}", m.id, hash(&format!("{m:?}"))));
    }
    for m in &defs.armor_materials {
        lines.push(format!("armor_material {} {}", m.id, hash(&format!("{m:?}"))));
    }
    lines.sort();
    lines.join("\n") + "\n"
}

/// The mod's data names vanilla content as its datapack version does; for
/// each other version that calls some of it otherwise, an overlay of the
/// file with those names (`when/any/<version>/…`). What a version hasn't
/// got under any name is said in a warning.
fn version_overlays(out: &mut Compiled, datapack: &str) {
    fn map(v: &mut Value, version: &str, missing: &mut BTreeSet<String>) -> bool {
        match v {
            Value::String(s) if s.starts_with("minecraft:") => {
                let id = s.trim_start_matches('#').to_string();
                let eq = refs::equivalent(refs::RefKind::Item, &id, version).or_else(|| refs::equivalent(refs::RefKind::Block, &id, version));
                match eq {
                    Some(e) if e != id => {
                        *s = e;
                        true
                    }
                    Some(_) => false,
                    // Not an id Yog knows (a loot function, a tag), or one this version hasn't got.
                    None => {
                        if refs::is_vanilla(&id) {
                            missing.insert(id);
                        }
                        false
                    }
                }
            }
            Value::Array(a) => a.iter_mut().fold(false, |c, x| map(x, version, missing) | c),
            Value::Object(o) => o.values_mut().fold(false, |c, x| map(x, version, missing) | c),
            _ => false,
        }
    }
    let mut overlays = Vec::new();
    for (path, bytes) in &out.outputs {
        if !path.starts_with("data/") || !path.ends_with(".json") {
            continue;
        }
        let Ok(json) = serde_json::from_slice::<Value>(bytes) else { continue };
        for version in refs::versions().iter().filter(|v| *v != datapack) {
            let mut copy = json.clone();
            let mut missing = BTreeSet::new();
            if map(&mut copy, version, &mut missing) {
                overlays.push((format!("when/any/{version}/{path}"), pretty(&copy)));
            }
            for id in missing {
                out.warnings.push(format!("{path} names {id}, which Minecraft {version} hasn't got"));
            }
        }
    }
    out.outputs.extend(overlays);
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

/// `stmt`, only when the mods `requires` names are all loaded (an optional integration).
fn when_loaded(requires: &[String], stmt: &str) -> String {
    if requires.is_empty() {
        return stmt.to_string();
    }
    let cond: Vec<String> = requires.iter().map(|m| format!("::yog_api::mod_loaded({m:?})")).collect();
    format!("if {} {{ {stmt} }}", cond.join(" && "))
}

/// The generated files' header.
const GENERATED_HEADER: &str = "// <auto-generated>\n//     Generated by `yog build` from content/** — don't edit, it's rewritten\n//     on every build. Included by `yog_api::content!()`.\n// </auto-generated>\n";

/// Where the content's code goes besides the root file, under
/// `.yog-build/generated/`: one file a part (`items.g.rs`, `register.g.rs`, …).
pub const GENERATED_PARTS_DIR: &str = "content";

/// The content's code: the root file ([`GENERATED_FILE`]), which only
/// includes the parts, and the parts — (name, code) each.
#[derive(Debug, Clone, Default)]
pub struct Source {
    pub root: String,
    pub parts: Vec<(String, String)>,
}

impl Source {
    /// Part `name`'s code (`"items"`).
    #[cfg(test)]
    pub fn part(&self, name: &str) -> &str {
        self.parts.iter().find(|(n, _)| n == name).map_or("", |(_, c)| c.as_str())
    }

    /// Every part's code, for tests that look for a line wherever it is.
    #[cfg(test)]
    pub fn all(&self) -> String {
        self.parts.iter().map(|(_, c)| c.as_str()).collect()
    }
}

/// Moves what `code` has so far into part `name`.
fn flush(parts: &mut Vec<(String, String)>, code: &mut String, name: &str) {
    if !code.trim().is_empty() {
        parts.push((name.to_string(), format!("{GENERATED_HEADER}{}", std::mem::take(code))));
    }
    code.clear();
}

fn rust_source(entries: &[Entry], recipes: &[Recipe], books: &[BookFile], bodies: &[BodyFile], entities: &[EntityFile], fonts: &[FontFile], defs: &Defs) -> Source {
    let sound_groups = &defs.sound_groups;
    let mut files: Vec<(String, String)> = Vec::new();
    let mut code = String::new();
    if !entities.is_empty() {
        code.push_str("\n/// The entity types of `content/` (`.yentity`), by id.\n#[allow(dead_code)]\npub struct Entities;\n\n#[allow(dead_code, clippy::all)]\nimpl Entities {\n");
        let mut used: HashMap<String, usize> = HashMap::new();
        for e in entities {
            let path = e.id.split_once(':').map_or(e.id.as_str(), |(_, p)| p);
            let mut ident = rust_ident(path).to_ascii_uppercase();
            let n = used.entry(ident.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let _ = write!(ident, "_{n}");
            }
            let _ = writeln!(code, "    pub const {ident}: &'static str = {:?};", e.id);
        }
        code.push_str("}\n");
        flush(&mut files, &mut code, "entities");
    }
    if !defs.advancements.is_empty() {
        code.push_str("\n/// The advancements of `content/` (`.ydef` kind `advancement`), by id\n/// (`yog_api::advancements::grant`/`done`).\n#[allow(dead_code)]\npub struct Advancements;\n\n#[allow(dead_code, clippy::all)]\nimpl Advancements {\n");
        let mut used: HashMap<String, usize> = HashMap::new();
        for id in &defs.advancements {
            let path = id.split_once(':').map_or(id.as_str(), |(_, p)| p);
            let mut ident = rust_ident(path).to_ascii_uppercase();
            let n = used.entry(ident.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let _ = write!(ident, "_{n}");
            }
            let _ = writeln!(code, "    pub const {ident}: &'static str = {id:?};");
        }
        code.push_str("}\n");
        flush(&mut files, &mut code, "advancements");
    }
    for (model, set, what) in [(true, "Models", "models (`.ymodel`)"), (false, "Anims", "animations (`.yanim`)")] {
        let mine: Vec<&BodyFile> = bodies.iter().filter(|b| b.model == model).collect();
        if mine.is_empty() {
            continue;
        }
        let _ = writeln!(code, "\n/// The {what} of `content/`, by id (`yog_api::body::model`/`clip`).\n#[allow(dead_code)]\npub struct {set};\n\n#[allow(dead_code, clippy::all)]\nimpl {set} {{");
        let mut used: HashMap<String, usize> = HashMap::new();
        for b in mine {
            let path = b.id.split_once(':').map_or(b.id.as_str(), |(_, p)| p);
            let mut ident = rust_ident(path).to_ascii_uppercase();
            let n = used.entry(ident.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                let _ = write!(ident, "_{n}");
            }
            let _ = writeln!(code, "    pub const {ident}: &'static str = {:?};", b.id);
        }
        code.push_str("}\n");
        flush(&mut files, &mut code, &set.to_ascii_lowercase());
    }
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
                Kind::Recipe | Kind::Book | Kind::Def | Kind::Model | Kind::Anim | Kind::Entity | Kind::Font => unreachable!("only blocks and items get here"),
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
                    if !p.facing.is_empty() {
                        let _ = write!(b, "\n            .facing({:?}, {:?})", p.facing, p.placement);
                    }
                    if p.attach {
                        b.push_str("\n            .attach()");
                    }
                    if p.waterlogged {
                        b.push_str("\n            .waterlogged()");
                    }
                    for (name, values) in &p.properties {
                        let vs: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
                        let _ = write!(b, "\n            .property({name:?}, &[{}])", vs.join(", "));
                    }
                    if let Some(name) = &p.powered {
                        let _ = write!(b, "\n            .powered({name:?})");
                    }
                    if let Some(name) = &p.comparator {
                        let _ = write!(b, "\n            .comparator({name:?})");
                    }
                    if p.accepts_items {
                        b.push_str("\n            .accepts_items()");
                    }
                    if p.no_occlusion {
                        b.push_str("\n            .no_occlusion()");
                    }
                    if !p.kind.is_empty() {
                        let _ = write!(b, "\n            .kind({:?})", p.kind);
                    }
                    if let Some(base) = &p.base {
                        let _ = write!(b, "\n            .base({base:?})");
                    }
                    if let Some(set) = &p.set {
                        let _ = write!(b, "\n            .set({set:?})");
                    }
                    if let Some(t) = p.press_ticks {
                        let _ = write!(b, "\n            .press_ticks({t})");
                    }
                    if p.mobs_only {
                        b.push_str("\n            .mobs_only()");
                    }
                }
                Kind::Item => b.push_str(&item_props(&e.item)),
            }
            let _ = writeln!(
                code,
                "    /// The definition from `{}` (name and tooltip come from the lang files).\n    pub fn {ident}() -> ::yog_api::{def} {{\n        {b}\n    }}",
                e.file
            );
        }
        code.push_str("}\n");
        flush(&mut files, &mut code, &set.to_ascii_lowercase());
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
            Kind::Block => {
                // Its item form, when the file says how it stacks, burns or survives fire.
                let item = item_props(&e.item);
                let stmt = if item.is_empty() {
                    format!("registry.register_block(Blocks::{ident}());")
                } else {
                    format!("{{ registry.register_block(Blocks::{ident}()); registry.register_item(::yog_api::ItemDef::new({:?}){}); }}", e.id, item.replace("\n            ", ""))
                };
                writeln!(code, "    {}", when_loaded(&e.requires, &stmt))
            }
            Kind::Item => writeln!(code, "    {}", when_loaded(&e.requires, &format!("registry.register_item(Items::{ident}());"))),
            Kind::Recipe | Kind::Book | Kind::Def | Kind::Model | Kind::Anim | Kind::Entity | Kind::Font => unreachable!("only blocks and items get here"),
        };
    }
    for g in sound_groups {
        let _ = writeln!(
            code,
            "    registry.register_sound_group(::yog_api::SoundGroup::new({:?}).sounds({:?}, {:?}, {:?}, {:?}, {:?}).volume({}).pitch({}));",
            g.id,
            g.break_sound,
            g.step,
            g.place,
            g.hit,
            g.fall,
            float(g.volume as f64),
            float(g.pitch as f64)
        );
    }
    for id in &defs.dimensions {
        let _ = writeln!(
            code,
            "    registry.register_dimension_data({id:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            dimension_path(id)
        );
    }
    for (feature, biomes, step) in &defs.biome_features {
        let _ = writeln!(code, "    registry.add_biome_feature({feature:?}, {biomes:?}, {step:?});");
    }
    for m in &defs.tool_materials {
        let _ = writeln!(
            code,
            "    registry.register_tool_material(::yog_api::ToolMaterial {{ id: {:?}.into(), durability: {}, speed: {:?}, attack_damage: {:?}, enchantability: {}, level: {:?}.into(), repair: {:?}.into(), level_index: {}, incorrect: {:?}.into() }});",
            m.id, m.durability, m.speed, m.attack_damage, m.enchantability, m.level, m.repair, m.level_index, m.incorrect
        );
    }
    for m in &defs.armor_materials {
        let d = m.defense;
        let _ = writeln!(
            code,
            "    registry.register_armor_material(::yog_api::ArmorMaterial {{ id: {:?}.into(), durability: {}, defense: ::yog_api::ArmorDefense {{ head: {}, chest: {}, legs: {}, feet: {} }}, enchantability: {}, equip_sound: {:?}.into(), toughness: {:?}, knockback_resistance: {:?}, repair: {:?}.into(), texture: {:?}.into() }});",
            m.id, m.durability, d.head, d.chest, d.legs, d.feet, m.enchantability, m.equip_sound, m.toughness, m.knockback_resistance, m.repair, m.texture
        );
    }
    for r in recipes {
        let _ = writeln!(code, "    {}", when_loaded(&r.requires, &r.registration()));
    }
    for b in books {
        let _ = writeln!(
            code,
            "    registry.register_book_data({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            b.id,
            b.path()
        );
    }
    for (kind, id) in &defs.definitions {
        let _ = writeln!(
            code,
            "    registry.register_definition_data({kind:?}, {id:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            definition_path(kind, id)
        );
    }
    for id in &defs.loot_modifiers {
        let _ = writeln!(
            code,
            "    registry.add_loot_modifier_data({id:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            loot_path(id)
        );
    }
    for e in entities {
        let _ = writeln!(
            code,
            "    registry.register_entity_data({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            e.id,
            e.path()
        );
    }
    for b in bodies {
        let _ = writeln!(
            code,
            "    registry.register_body_{}({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            if b.model { "model" } else { "anims" },
            b.id,
            b.path()
        );
    }
    for font in fonts {
        let _ = writeln!(
            code,
            "    registry.register_font_data({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{}\")));",
            font.id,
            font.path()
        );
    }
    code.push_str("}\n");
    if entries.iter().any(|e| e.auto) || !recipes.is_empty() || !books.is_empty() || !bodies.is_empty() || !entities.is_empty() || !fonts.is_empty() || !sound_groups.is_empty() || !defs.tool_materials.is_empty() || !defs.armor_materials.is_empty() || !defs.biome_features.is_empty() || !defs.dimensions.is_empty() {
        code.push_str(
            "\n// Runs when the mod's library loads: `export_mod!` calls `register` after\n// `Mod::register`.\n#[::yog_api::ctor::ctor(unsafe, crate_path = ::yog_api::ctor)]\nfn __yog_content_auto_register() {\n    ::yog_api::__yog_content_registrars()\n        .lock()\n        .unwrap_or_else(|e| e.into_inner())\n        .push(register);\n}\n",
        );
    }
    flush(&mut files, &mut code, "register");
    for t in &defs.tables {
        code.push_str(&table::rust(t));
    }
    flush(&mut files, &mut code, "tables");
    let mut root = String::from(GENERATED_HEADER);
    for (name, _) in &files {
        let _ = writeln!(root, "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{GENERATED_PARTS_DIR}/{name}.g.rs\"));");
    }
    Source { root, parts: files }
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
    write_tags(root, compiled)?;
    let out = root.join(GENERATED_FILE);
    let dir = out.parent().unwrap_or(root);
    let parts_dir = dir.join(GENERATED_PARTS_DIR);
    let Some(source) = &compiled.source else {
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_dir_all(&parts_dir);
        return Ok(());
    };
    std::fs::create_dir_all(&parts_dir).map_err(|e| format!("{}: {e}", parts_dir.display()))?;
    let wanted: Vec<std::path::PathBuf> = source.parts.iter().map(|(name, _)| parts_dir.join(format!("{name}.g.rs"))).collect();
    for ((_, code), file) in source.parts.iter().zip(&wanted) {
        crate::write_if_changed(file, code)?;
    }
    // Parts the content no longer has.
    for entry in std::fs::read_dir(&parts_dir).map_err(|e| format!("{}: {e}", parts_dir.display()))?.flatten() {
        if !wanted.contains(&entry.path()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    crate::write_if_changed(&out, &source.root)?;
    for (path, bytes) in &compiled.data_files {
        let file = dir.join(path);
        if std::fs::read(&file).ok().as_deref() != Some(bytes.as_slice()) {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            std::fs::write(&file, bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    Ok(())
}

/// Where the mod's tags are listed for tools (`#ns:path` → its entries, by
/// kind) — the IDE shows a tag's items from here, as the build makes them.
pub const TAGS_FILE: &str = ".yog-build/generated/tags.json";

/// Writes [`TAGS_FILE`]: `{ "item": { "ns:path": ["ns:id", "#ns:tag", …] }, "block": … }`.
fn write_tags(root: &Path, compiled: &Compiled) -> Result<(), String> {
    let mut by_kind: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for (path, entries) in &compiled.tags {
        // `[when/<loader>/<version>/]data/<ns>/tags/<kind>/<path>.json`
        let data = path.find("data/").map_or(path.as_str(), |i| &path[i + 5..]);
        let Some((ns, rest)) = data.split_once("/tags/") else { continue };
        let Some((kind, tag)) = rest.split_once('/') else { continue };
        let tag = tag.trim_end_matches(".json");
        by_kind.entry(kind.to_string()).or_default().entry(format!("{ns}:{tag}")).or_default().extend(entries.keys().cloned());
    }
    let text = serde_json::to_string_pretty(&by_kind).map_err(|e| e.to_string())?;
    crate::write_if_changed(&root.join(TAGS_FILE), &text)
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
            for (id, &required) in ids {
                let listed = |v: &Value| v.as_str().or_else(|| v.get("id").and_then(Value::as_str)) == Some(id.as_str());
                if !list.iter().any(listed) {
                    list.push(if required { Value::String(id.clone()) } else { serde_json::json!({ "id": id, "required": false }) });
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
        let src = &c.source.as_ref().unwrap().all();
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
    fn a_facing_block_turns_to_its_placer() {
        let root = project(&[(
            "content/screen.yblock",
            r##"{ "model": { "parent": "cube_all", "textures": { "all": "minecraft:block/stone" } },
                 "states": { "facing": "horizontal" } }"##,
        )]);
        let c = compile(&root, "mymod", &yres::Refs::default()).unwrap();
        let bs = output(&c, "assets/mymod/blockstates/screen.json");
        assert_eq!(bs["variants"]["facing=north"], json!({ "model": "mymod:block/screen" }));
        assert_eq!(bs["variants"]["facing=west"]["y"], json!(270));
        assert!(c.source.unwrap().all().contains(".facing(\"horizontal\", \"toward\")"));
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
        assert_eq!(m["parent"], json!("minecraft:block/block"), "shown in a slot and held as a block");
        let bs = output(&c, "assets/mymod/blockstates/pipe.json");
        assert_eq!(bs["multipart"].as_array().unwrap().len(), 7);
        assert_eq!(bs["multipart"][5]["apply"]["x"], json!(270));
        let src = c.source.unwrap().all();
        assert!(
            src.contains(".shape(6.0, 6.0, 6.0, 10.0, 10.0, 10.0)"),
            "{src}"
        );
        assert!(src.contains(".connects_to_neighbors()"), "{src}");
        assert!(src.contains(".connect_groups(&[\"mymod:pipe\"])"), "{src}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sound_groups_are_the_games_or_a_mods_ydef() {
        let cube = r#"model: { parent: "cube_all", textures: { all: "minecraft:block/stone" } }"#;
        let crystal = r#"{ kind: "sound_group", id: "crystal", break: "crystal.break", step: "minecraft:block.glass.step",
  place: "crystal.place", hit: "crystal.hit", fall: "crystal.fall", pitch: 1.5 }"#;
        let a = format!(r#"{{ {cube}, block: {{ sound: "@sound_group:mymod:crystal" }} }}"#);
        let b = format!(r#"{{ {cube}, block: {{ sound: "amethyst" }} }}"#);
        let c = compile(&project(&[("content/a.yblock", &a), ("content/b.yblock", &b), ("content/crystal.ydef", crystal)]), "mymod", &yres::Refs::default()).unwrap();
        let src = c.source.unwrap().all();
        assert!(src.contains(r#"registry.register_sound_group(::yog_api::SoundGroup::new("mymod:crystal").sounds("mymod:crystal.break", "minecraft:block.glass.step", "mymod:crystal.place", "mymod:crystal.hit", "mymod:crystal.fall").volume(1.0).pitch(1.5));"#), "{src}");
        assert!(src.contains(r#".sound("mymod:crystal")"#) && src.contains(r#".sound("minecraft:amethyst")"#), "{src}");

        let bad = format!(r#"{{ {cube}, block: {{ sound: "crystal" }} }}"#);
        let missing = format!(r#"{{ {cube}, block: {{ sound: "@sound_group:mymod:nope" }} }}"#);
        let e = compile(&project(&[("content/a.yblock", &bad), ("content/b.yblock", &missing)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("`crystal`: not one of the game's sound groups"), "{e}");
        assert!(e.contains("no sound group mymod:nope"), "{e}");
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
            assert!(c.tags.get(tag).is_some_and(|ids| ids.contains_key("mymod:ore")), "{tag}: {:?}", c.tags);
        }
        // The game's levels by name.
        let iron = format!(r#"{{ {cube}, block: {{ tool: "pickaxe", tool_level: "iron" }} }}"#);
        let c = compile(&project(&[("content/ore.yblock", &iron)]), "mymod", &yres::Refs::default()).unwrap();
        assert!(c.tags["data/minecraft/tags/block/needs_iron_tool.json"].contains_key("mymod:ore"));
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
        let src = c.source.clone().unwrap().all();
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
        assert!(c.source.unwrap().all().contains(r#".inventory("mymod:bench")"#));
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
        let src = out.source.unwrap().all();
        assert!(src.contains(r#"registry.register_book_data("mymod:guide", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/generated/books/mymod__guide.ydata")));"#), "{src}");
        // The runtime reads the bytes by the book's schema; references come back as ids.
        assert_eq!(out.data_files[0].0, "books/mymod__guide.ydata");
        let data = yog_data::binary::decode(&out.data_files[0].1).unwrap();
        let text = yog_data::text::to_string(&data);
        assert!(text.contains("mymod.book.title") && text.contains("item.mymod.ruby"), "{text}");
        assert!(text.contains("not a ref") && text.contains("i18n"), "{text}");
        let read = yog_data::from_value::<yog_book::Book>(&data).unwrap().value;
        assert_eq!(read.entries[0].category, "mymod:basics");
        assert!(matches!(&read.entries[0].pages[1], yog_book::BookPage::Spotlight { item, .. } if item == "mymod:ruby"), "references become ids");

        // Bare ids, missing content and version-only ids are errors, each on its line.
        let bad = r#"{ "id": "x",
            "categories": [{ "id": "c", "name": "C" }],
            "entries": [{ "id": "e", "name": "E", "category": "c",
                "pages": [
                    { "type": "spotlight", "item": "minecraft:stick" },
                    { "type": "spotlight", "item": "@item:minecraft:grass" },
                    { "type": "recipe", "recipe": "@recipe:mymod:nope" },
                    { "type": "entity", "entity": "@entity:create:train" }
                ] }] }"#;
        let e = compile(&project(&[("content/bad.ybook", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("bad.ybook:3: entries[0].category: `c`: name it by a reference — `@category:c`"), "{e}");
        assert!(e.contains("bad.ybook:5:") && e.contains("`@item:minecraft:stick`"), "{e}");
        assert!(e.contains("minecraft:grass isn't in Minecraft 1.21.1 (the mod's datapack; it's in 1.20.1) — there it's minecraft:short_grass"), "{e}");
        assert!(e.contains("no recipe mymod:nope in this mod"), "{e}");
        assert!(e.contains("[external.create]"), "{e}");

        let bad = project(&[("content/bad.ybook", r#"{ "id": "x", "entries": [{ "pages": [{ "type": "nope" }] }] }"#)]);
        let e = compile(&bad, "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("entries[0].pages[0].type: unknown page type `nope`"), "{e}");
        let bad = project(&[("content/bad.ybook", r#"{ "id": "x", "name": "X", "categories": [{ "id": "c", "name": "C" }] }"#)]);
        let e = compile(&bad, "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("categories[0].id: `c`: write its namespace (`mymod:c`)"), "{e}");
    }

    #[test]
    fn books_use_the_mods_own_page_types() {
        let ty = r#"<Component name="PatternPage" book_page="pattern">
            <Property name="name" type="book_text"/>
            <Property name="patterns" type="data" default="[]"/>
            <BookHeader text="{x:Bind name}"/>
            <Drawn drawer="mymod:pattern" h="64" data="{x:Bind patterns}"/>
        </Component>"#;
        let book = |page: &str| format!(r#"{{ id: "guide", name: "G", categories: [{{ id: "mymod:c", name: "C" }}],
            entries: [{{ id: "mymod:e", name: "E", category: "@category:mymod:c", pages: [{page}] }}] }}"#);
        let ok = book(r#"{ type: "mymod:pattern", name: "@item:minecraft:stick", patterns: [{ startdir: "EAST", signature: "qaq" }] }"#);
        let c = compile(&project(&[("src/ui/pattern_page.yui", ty), ("content/guide.ybook", &ok)]), "mymod", &yres::Refs::default()).unwrap();
        let b = yog_data::read_bytes::<yog_book::Book>(&c.data_files[0].1).unwrap();
        assert_eq!(b.page_types.len(), 1, "the type goes with the book");
        let yog_book::BookPage::Custom(p) = &b.entries[0].pages[0] else { panic!("{:?}", b.entries[0].pages[0]) };
        assert_eq!(p.fields.get("name").and_then(|n| n.as_str()), Some("item.minecraft.stick"), "texts resolve as the book's do");
        assert!(b.i18n);

        let unknown = book(r#"{ type: "mymod:pattern", name: "N", colour: 3 }"#);
        let e = compile(&project(&[("src/ui/pattern_page.yui", ty), ("content/guide.ybook", &unknown)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("mymod:pattern has no field `colour` (name, patterns)"), "{e}");
        let missing = book(r#"{ type: "mymod:nope" }"#);
        let e = compile(&project(&[("content/guide.ybook", &missing)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("no page type mymod:nope"), "{e}");
    }

    #[test]
    fn tools_and_armor_of_the_games_materials_or_the_mods() {
        let model = r#"model: { parent: "generated", textures: { layer0: "minecraft:item/iron_pickaxe" } }"#;
        let level = r#"{ kind: "level", id: "mythril", ladder: "@ladder:minecraft:tool", above: "@level:minecraft:diamond" }"#;
        let tool = r#"{ kind: "tool_material", id: "mythril", durability: 1800, speed: 10, attack_damage: 3, enchantability: 18,
            level: "@level:mymod:mythril", repair: "@tag:item:c:ingots" }"#;
        let armor = r#"{ kind: "armor_material", id: "mythril", durability: 30, defense: { head: 3, chest: 8, legs: 6, feet: 3 },
            enchantability: 18, equip_sound: "minecraft:item.armor.equip_diamond", repair: "@item:minecraft:diamond" }"#;
        let pick = format!(r#"{{ {model}, item: {{ tool: {{ kind: "pickaxe", material: "@tool_material:mymod:mythril" }} }} }}"#);
        let helm = format!(r#"{{ {model}, item: {{ armor: {{ slot: "head", material: "iron" }} }} }}"#);
        let files = [("content/mythril.ydef", level), ("content/mythril_tools.ydef", tool), ("content/mythril_armor.ydef", armor), ("content/pick.yitem", pick.as_str()), ("content/helm.yitem", helm.as_str())];
        let c = compile(&project(&files), "mymod", &yres::Refs::default()).unwrap();
        let src = c.source.unwrap().all();
        assert!(src.contains(r#".tool("pickaxe", "mymod:mythril", 1.0, -2.8)"#), "the kind's attack: {src}");
        assert!(src.contains(r#".armor("head", "minecraft:iron")"#), "{src}");
        assert!(src.contains(r##"level: "mymod:mythril".into(), repair: "#c:ingots".into(), level_index: 4, incorrect: "minecraft:incorrect_for_netherite_tool".into()"##), "above diamond: netherite's on both versions: {src}");
        assert!(src.contains(r#"registry.register_armor_material(::yog_api::ArmorMaterial { id: "mymod:mythril".into()"#) && src.contains(r#"texture: "mymod:mythril".into()"#), "{src}");

        let bad = format!(r#"{{ {model}, item: {{ tool: {{ kind: "spoon", material: "bronze" }} }} }}"#);
        let e = compile(&project(&[("content/bad.yitem", &bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("`spoon`: one of pickaxe, axe, shovel, hoe, sword") && e.contains("`bronze`: one of the game's (wood, stone, iron, diamond, gold, netherite)"), "{e}");
        let missing = format!(r#"{{ {model}, item: {{ armor: {{ slot: "feet", material: "@armor_material:mymod:nope" }} }} }}"#);
        let e = compile(&project(&[("content/m.yitem", &missing)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("no armor material mymod:nope"), "{e}");
    }

    #[test]
    fn data_naming_renamed_content_gets_each_versions_name() {
        let model = r#"model: { parent: "cube_all", textures: { all: "minecraft:block/stone" } }"#;
        let block = format!(r#"{{ {model}, block: {{ drops: {{ item: "@item:minecraft:short_grass" }} }} }}"#);
        let c = compile(&project(&[("content/tuft.yblock", &block)]), "mymod", &yres::Refs::default()).unwrap();
        let loot = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| String::from_utf8_lossy(b).to_string());
        assert!(loot("data/mymod/loot_table/blocks/tuft.json").is_some_and(|l| l.contains("minecraft:short_grass")), "the datapack's name");
        let old = loot("when/any/1.20.1/data/mymod/loot_table/blocks/tuft.json").expect("1.20.1's overlay");
        assert!(old.contains("minecraft:grass") && !old.contains("short_grass"), "{old}");
        let crafter = format!(r#"{{ {model}, block: {{ drops: {{ item: "@item:minecraft:crafter" }} }} }}"#);
        let c = compile(&project(&[("content/c.yblock", &crafter)]), "mymod", &yres::Refs::default()).unwrap();
        assert!(c.warnings.iter().any(|w| w.contains("minecraft:crafter, which Minecraft 1.20.1 hasn't got")), "{:?}", c.warnings);
    }

    #[test]
    fn ores_become_features_in_biomes_on_every_loader() {
        let model = r#"model: { parent: "cube_all", textures: { all: "minecraft:block/stone" } }"#;
        let block = format!(r#"{{ {model} }}"#);
        let ore = r#"{ kind: "ore", id: "mythril_ore", targets: [{ replaces: "stone", block: "@block:mymod:mythril_ore" },
            { replaces: "@block:minecraft:basalt", block: "@block:mymod:mythril_ore" }], size: 6, per_chunk: 4,
            height: { min: -48, max: 16, shape: "triangle" } }"#;
        let c = compile(&project(&[("content/mythril_ore.yblock", &block), ("content/mythril_ore.ydef", ore)]), "mymod", &yres::Refs::default()).unwrap();
        let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap()).unwrap_or_else(|| panic!("no {p}"));
        let cf = get("data/mymod/worldgen/configured_feature/mythril_ore.json");
        assert_eq!(cf["config"]["targets"][0]["target"]["tag"], "minecraft:stone_ore_replaceables");
        assert_eq!(cf["config"]["targets"][1]["target"]["block"], "minecraft:basalt");
        let pf = get("data/mymod/worldgen/placed_feature/mythril_ore.json");
        assert_eq!(pf["placement"][2]["height"]["type"], "minecraft:trapezoid");
        assert_eq!(get("when/neoforge/1.21/data/mymod/neoforge/biome_modifier/mythril_ore.json")["type"], "neoforge:add_features");
        assert_eq!(get("when/forge/any/data/mymod/forge/biome_modifier/mythril_ore.json")["biomes"], "#minecraft:is_overworld");
        let src = c.source.unwrap().all();
        assert!(src.contains(r#"registry.add_biome_feature("mymod:mythril_ore", "minecraft:is_overworld", "underground_ores");"#), "{src}");
        let bad = r#"{ kind: "ore", id: "x", targets: [{ replaces: "granite", block: "@block:mymod:nope" }], size: 6, per_chunk: 4, height: { min: 0, max: 10 } }"#;
        let e = compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("`granite`: one of stone, deepslate, netherrack, end_stone"), "{e}");
        let missing = r#"{ kind: "ore", id: "x", targets: [{ replaces: "stone", block: "@block:mymod:nope" }], size: 6, per_chunk: 4, height: { min: 0, max: 10 } }"#;
        let e = compile(&project(&[("content/x.ydef", missing)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("no block mymod:nope"), "{e}");
    }

    #[test]
    fn advancements_are_definitions_for_every_version() {
        let root = r#"{ kind: "advancement", id: "root", icon: "@item:minecraft:amethyst_shard", title: "Gems",
            background: "minecraft:textures/block/calcite.png", criteria: [{ has_item: "@tag:item:yog:gems/amethyst" }] }"#;
        let lore = r#"{ kind: "advancement", id: "lore/one", parent: "@advancement:mymod:root", icon: "@item:minecraft:paper",
            title: "@lang:advancement.mymod.lore.one", hidden: true, criteria: [{ granted: true }] }"#;
        let c = compile(&project(&[("content/root.ydef", root), ("content/lore.ydef", lore)]), "mymod", &yres::Refs::default()).unwrap();
        let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap()).unwrap_or_else(|| panic!("no {p}"));
        let new = get("data/mymod/advancement/root.json");
        assert_eq!(new["display"]["icon"]["id"], "minecraft:amethyst_shard");
        assert_eq!(new["criteria"]["c0"]["conditions"]["items"][0]["items"], "#yog:gems/amethyst");
        assert_eq!(new["display"]["title"]["translate"], "advancement.mymod.root");
        let old = get("when/any/1.20.1/data/mymod/advancement/root.json");
        assert_eq!(old["display"]["icon"]["item"], "minecraft:amethyst_shard");
        assert_eq!(old["criteria"]["c0"]["conditions"]["items"][0]["tag"], "yog:gems/amethyst");
        let one = get("data/mymod/advancement/lore/one.json");
        assert_eq!((one["parent"].as_str(), one["criteria"]["c0"]["trigger"].as_str()), (Some("mymod:root"), Some("minecraft:impossible")));
        assert_eq!(c.lang[&("mymod".to_string(), yres::NEUTRAL_LOCALE.to_string())]["advancement.mymod.root"], "Gems");
        // The code names them by constant.
        let src = &c.source.as_ref().unwrap().all();
        assert!(src.contains("pub const LORE_ONE: &'static str = \"mymod:lore/one\";"), "{src}");
        let bad = r#"{ kind: "advancement", id: "x", icon: "@item:minecraft:paper", title: "X", criteria: [{ granted: true, used_item: "@item:minecraft:paper" }] }"#;
        assert!(compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err().contains("one of granted"));
    }

    #[test]
    fn a_mod_has_loot_tables_of_its_own() {
        let table = r#"{ kind: "loot_table", id: "random_gem", add: [{ item: "@item:minecraft:diamond", count: [1, 3] }] }"#;
        let c = compile(&project(&[("content/random_gem.ydef", table)]), "mymod", &yres::Refs::default()).unwrap();
        let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap());
        assert_eq!(get("data/mymod/loot_table/random_gem.json").expect("the table")["pools"], json!([]));
        let src = c.source.unwrap().all();
        assert!(src.contains(r#"registry.add_loot_modifier_data("mymod:random_gem""#), "{src}");
        let modifier = r#"{ kind: "loot_modifier", id: "more_gems", tables: ["@loot_table:minecraft:chests/simple_dungeon"], add: [{ item: "@item:minecraft:emerald", chance: 0.5 }] }"#;
        let c = compile(&project(&[("content/more_gems.ydef", modifier)]), "mymod", &yres::Refs::default()).unwrap();
        assert!(c.source.unwrap().all().contains(r#"registry.add_loot_modifier_data("mymod:more_gems""#));
        let missing = r#"{ kind: "loot_modifier", id: "m", tables: ["@loot_table:minecraft:chests/nope"], add: [] }"#;
        assert!(compile(&project(&[("content/m.ydef", missing)]), "mymod", &yres::Refs::default()).unwrap_err().contains("chests/nope"));
        let bad = r#"{ kind: "loot_table", id: "minecraft:nope", add: [] }"#;
        assert!(compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err().contains("the mod's own"));
    }

    #[test]
    fn a_block_says_how_its_item_stacks() {
        let model = r#"model: { parent: "cube_all", textures: { all: "minecraft:block/stone" } }"#;
        let block = format!(r#"{{ item: {{ stack_size: 16, fire_resistant: true, fuel_ticks: 300 }}, {model} }}"#);
        let plain = format!(r#"{{ {model} }}"#);
        let c = compile(&project(&[("content/log.yblock", &block), ("content/stone.yblock", &plain)]), "mymod", &yres::Refs::default()).unwrap();
        let src = c.source.unwrap().all();
        assert!(src.contains(r#"registry.register_item(::yog_api::ItemDef::new("mymod:log").max_stack(16).fire_resistant().fuel(300));"#), "{src}");
        assert!(!src.contains(r#"ItemDef::new("mymod:stone")"#), "no item form of its own: the host's default");
    }

    #[test]
    fn blocks_and_items_join_their_tags() {
        let model = r#"model: { parent: "cube_all", textures: { all: "minecraft:block/stone" } }"#;
        let log = format!(r#"{{ tags: ["minecraft:logs", "edified_logs"], {model} }}"#);
        let staff = r#"{ tags: ["staves"], model: { parent: "generated", textures: { layer0: "minecraft:item/stick" } } }"#;
        let c = compile(&project(&[("content/log.yblock", &log), ("content/staff.yitem", staff)]), "mymod", &yres::Refs::default()).unwrap();
        for t in ["data/minecraft/tags/block/logs.json", "data/minecraft/tags/item/logs.json", "data/mymod/tags/block/edified_logs.json", "data/mymod/tags/item/edified_logs.json"] {
            assert!(c.tags[t].contains_key("mymod:log"), "{t}");
        }
        assert!(c.tags["data/mymod/tags/item/staves.json"].contains_key("mymod:staff"));
        assert!(!c.tags.contains_key("data/mymod/tags/block/staves.json"), "an item has no block tag");
    }

    #[test]
    fn trees_are_definitions() {
        let model = r#"model: { parent: "cube_all", textures: { all: "minecraft:block/stone" } }"#;
        let block = format!(r#"{{ {model} }}"#);
        let tree = r#"{ kind: "tree", id: "glass_tree", min_clipped_height: 6,
            trunk: { blocks: [{ block: "@block:mymod:log", weight: 8 }, { block: "@block:minecraft:oak_log" }], shape: "fancy", base_height: 5, random_a: 5, random_b: 3 },
            leaves: { blocks: [{ block: "@block:mymod:leaves" }], shape: "fancy", radius: 1, offset: 5, height: 5 } }"#;
        let c = compile(&project(&[("content/log.yblock", &block), ("content/leaves.yblock", &block), ("content/glass_tree.ydef", tree)]), "mymod", &yres::Refs::default()).unwrap();
        let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap()).unwrap_or_else(|| panic!("no {p}"));
        let f = &get("data/mymod/worldgen/configured_feature/glass_tree.json")["config"];
        assert_eq!(f["trunk_placer"]["type"], "minecraft:fancy_trunk_placer");
        assert_eq!(f["trunk_provider"]["entries"][0]["weight"], 8);
        assert_eq!(f["foliage_provider"]["state"]["Name"], "mymod:leaves");
        assert_eq!((f["foliage_placer"]["height"].as_u64(), f["minimum_size"]["min_clipped_height"].as_u64()), (Some(5), Some(6)));
        let bad = r#"{ kind: "tree", id: "t", trunk: { blocks: [{ block: "@block:mymod:nope" }], shape: "straight", base_height: 4 },
            leaves: { blocks: [{ block: "@block:minecraft:oak_leaves" }], shape: "blob", radius: 2 } }"#;
        let e = compile(&project(&[("content/t.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("no block mymod:nope"), "{e}");
    }

    #[test]
    fn what_a_version_or_loader_hasnt_got_keeps_a_recipe_out_of_it() {
        let recipe = |key: &str| format!(r#"{{ type: "shaped", pattern: ["D"], key: {{ D: {key} }}, result: "@item:minecraft:stick" }}"#);
        let check = |key: &str| {
            let c = compile(&project(&[("content/r.yrecipe", &recipe(key))]), "mymod", &yres::Refs::default());
            c.map(|c| c.warnings.iter().filter(|w| w.contains("left out")).cloned().collect::<Vec<_>>().join("\n"))
        };
        let w = check(r#""@tag:item:minecraft:music_discs""#).unwrap();
        assert!(w.contains("left out on Minecraft 1.21.1: #minecraft:music_discs isn't there — @tag:item:yog:music_discs is on every version and loader"), "{w}");
        assert_eq!(check(r#""@tag:item:yog:music_discs""#).unwrap(), "");
        assert_eq!(check(r#"["@tag:item:c:iron_ingots", "@item:minecraft:iron_ingot"]"#).unwrap(), "", "an alternative that's everywhere");
        let w = check(r#""@tag:item:c:iron_ingots""#).unwrap();
        assert!(w.contains("left out on forge 1.20.1, neoforge 1.20.1, Minecraft 1.21.1") && w.contains("@tag:item:yog:ingots/iron"), "{w}");
        let e = check(r#""@tag:item:c:nope""#).unwrap_err();
        assert!(e.contains("no item tag c:nope in Minecraft or its loaders"), "{e}");
        let e = check(r#""@tag:item:yog:nope""#).unwrap_err();
        assert!(e.contains("no item tag yog:nope among Yog's common tags"), "{e}");
        assert!(check(r#""@tag:item:othermod:anything""#).is_ok(), "another mod's tags aren't known here");
        let block = r#"{ tags: ["yog:planks", "yog:nope"], model: { parent: "cube_all", textures: { all: "minecraft:block/stone" } } }"#;
        let e = compile(&project(&[("content/p.yblock", block)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("tags[1]: no block tag yog:nope") && !e.contains("yog:planks"), "{e}");
    }

    #[test]
    fn tags_and_damage_types_are_definitions() {
        let tag = r#"{ kind: "tag", of: "entity", id: "sticky_teleporters", values: ["@entity:minecraft:pig", "@tag:entity:c:bosses"],
            optional: ["@entity:create:seat"] }"#;
        let logs = r#"{ kind: "tag", of: "block", id: "minecraft:logs", values: ["@block:minecraft:stone"] }"#;
        let dmg = r#"{ kind: "damage_type", id: "overcast", scaling: "never", tags: ["minecraft:bypasses_armor", "bypasses_effects"] }"#;
        let c = compile(&project(&[("content/a.ydef", tag), ("content/b.ydef", logs), ("content/c.ydef", dmg)]), "mymod", &yres::Refs::default()).unwrap();
        let sticky = &c.tags["data/mymod/tags/entity_type/sticky_teleporters.json"];
        assert_eq!(sticky.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(), vec![("#c:bosses", false), ("create:seat", false), ("minecraft:pig", true)], "c:bosses isn't on every loader: where it isn't, the tag still loads");
        assert!(c.tags["data/minecraft/tags/block/logs.json"].contains_key("minecraft:stone"));
        let get = |p: &str| c.outputs.iter().find(|(q, _)| q == p).map(|(_, b)| serde_json::from_slice::<Value>(b).unwrap()).unwrap_or_else(|| panic!("no {p}"));
        let d = get("data/mymod/damage_type/overcast.json");
        assert_eq!((d["message_id"].as_str(), d["scaling"].as_str()), (Some("mymod.overcast"), Some("never")));
        for t in ["data/minecraft/tags/damage_type/bypasses_armor.json", "data/minecraft/tags/damage_type/bypasses_effects.json"] {
            assert!(c.tags[t].contains_key("mymod:overcast"), "{t}");
        }
        let bad = r#"{ kind: "tag", of: "entity", id: "x", values: ["@entity:minecraft:nope", "@block:minecraft:stone"] }"#;
        let e = compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("nope") && e.contains("`@block:minecraft:stone` in a tag of entity"), "{e}");
    }

    #[test]
    fn a_dimension_and_its_terrain_go_to_the_runtime_as_data() {
        let dim = r#"{ kind: "dimension", id: "flatlands", dimension_type: { has_sky_light: true }, biome: "@biome:minecraft:desert",
            terrain: { seed: 3, base_height: 70, noise: [{ scale: 80, amplitude: 6 }], fill: "@block:minecraft:sandstone",
                layers: [{ block: "@block:minecraft:sand", depth: 3 }] } }"#;
        let c = compile(&project(&[("content/flatlands.ydef", dim)]), "mymod", &yres::Refs::default()).unwrap();
        let (path, data) = c.data_files.iter().find(|(p, _)| p.starts_with("dimensions/")).expect("the dimension's data");
        assert_eq!(path, "dimensions/mymod__flatlands.ydata");
        let v = yog_data::binary::decode(data).unwrap();
        assert_eq!(v.get("biome"), Some(&yog_data::Value::from("minecraft:desert")), "a plain id: the hosts read it");
        assert_eq!(v.get("generator_type"), Some(&yog_data::Value::from("mymod:flatlands")));
        let t = yog_data::read_value::<yog_worldgen::Terrain>(v.get("terrain").unwrap()).unwrap();
        assert_eq!((t.base_height, t.fill.as_str()), (70, "minecraft:sandstone"));
        let src = c.source.unwrap().all();
        assert!(src.contains(r#"registry.register_dimension_data("mymod:flatlands", include_bytes!("#), "{src}");
        let bad = r#"{ kind: "dimension", id: "x", terrain: { fill: "@block:minecraft:nope" } }"#;
        let e = compile(&project(&[("content/x.ydef", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("no block minecraft:nope"), "{e}");
    }

    #[test]
    fn content_for_other_mods_is_registered_only_with_them() {
        let model = r#"model: { parent: "generated", textures: { layer0: "minecraft:item/stick" } }"#;
        let item = format!(r#"{{ requires: ["pehkui", "create"], {model} }}"#);
        let recipe = r#"{ requires: ["pehkui"], type: "shapeless", ingredients: ["@item:minecraft:stick"], result: "@item:minecraft:stick" }"#;
        let c = compile(&project(&[("content/shrink.yitem", &item), ("content/r.yrecipe", recipe)]), "mymod", &yres::Refs::default()).unwrap();
        let src = c.source.unwrap().all();
        assert!(src.contains(r#"if ::yog_api::mod_loaded("pehkui") && ::yog_api::mod_loaded("create") { registry.register_item(Items::shrink()); }"#), "{src}");
        assert!(src.contains(r#"if ::yog_api::mod_loaded("pehkui") { registry.add_recipe("#), "{src}");
    }

    #[test]
    fn books_lay_out_the_mods_own_recipe_kinds() {
        let recipe = r#"{ type: "mymod:infusion", slots: { core: ["@item:minecraft:diamond"] }, result: "@item:minecraft:emerald" }"#;
        let book = r#"{ id: "guide", name: "G", categories: [{ id: "mymod:c", name: "C" }],
            entries: [{ id: "mymod:e", name: "E", category: "@category:mymod:c",
                pages: [{ type: "recipe", recipe: "@recipe:mymod:infusion" }] }] }"#;
        let kind = r#"{ kind: "recipe_kind", id: "infusion", slots: { core: { required: true } } }"#;
        let layout = r#"<Component name="InfusionLayout" recipe_layout="infusion" width="96" height="24">
                <RecipeSlot slot="core" x="4" y="4" frame="true"/>
                <ItemSlot item="minecraft:beacon" x="40" y="4"/>
                <RecipeResult x="76" y="4" frame="true"/>
            </Component>"#;
        let files = [("content/infusion.yrecipe", recipe), ("content/guide.ybook", book), ("content/infusion.ydef", kind)];
        let e = compile(&project(&files), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("entries[0].pages[0].recipe: no layout for recipe kind mymod:infusion"), "{e}");
        let c = compile(&project(&[files[0], files[1], files[2], ("src/ui/infusion.yui", layout)]), "mymod", &yres::Refs::default()).unwrap();
        let book = yog_data::read_bytes::<yog_book::Book>(&c.data_files[0].1).unwrap();
        assert_eq!(book.layouts[0].kinds, ["mymod:infusion"], "the book carries the mod's layouts");
        // What a layout names is checked: the kinds, a mod kind's slots, the stations.
        let bad = r#"<Component recipe_layout="create:mixing infusion" width="1" height="1">
                <RecipeSlot slot="ring"/><ItemSlot item="minecraft:nope"/></Component>"#;
        let e = compile(&project(&[files[2], ("src/ui/bad.yui", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("`create:mixing`: one of shaped"), "{e}");
        assert!(e.contains("mymod:infusion has no slot `ring` — core"), "{e}");
        assert!(e.contains("<ItemSlot item=\"minecraft:nope\">: no item minecraft:nope in Minecraft"), "{e}");
    }

    #[test]
    fn compiles_a_recipe_of_the_mods_own_kind() {
        let kind = r#"{ kind: "recipe_kind", id: "infusion", slots: { core: { required: true }, ring: { max: 2 } }, values: { time: "number" } }"#;
        let r = r##"{ type: "mymod:infusion", slots: { core: ["@item:minecraft:diamond"], ring: [["@item:minecraft:coal", "@item:minecraft:charcoal"], "@tag:item:c:dusts"] },
                     values: { time: 200 }, result: "@item:minecraft:emerald", count: 2 }"##;
        let c = compile(&project(&[("content/infusion.ydef", kind), ("content/infusion.yrecipe", r)]), "mymod", &yres::Refs::default()).unwrap();
        let src = c.source.unwrap().all();
        assert!(src.contains("registry.add_recipe_json(\"mymod:infusion\""), "{src}");
        assert!(src.contains(r#"\"type\":\"mymod:infusion\""#) && src.contains(r#"\"time\":200"#), "{src}");

        // Against its kind: slots, how many, values and their types.
        let bad = r##"{ type: "mymod:infusion", slots: { ring: ["@item:minecraft:coal", "@item:minecraft:coal", "@item:minecraft:coal"], nope: ["@item:minecraft:coal"] },
                       values: { time: "soon", heat: 3 }, result: "@item:minecraft:emerald" }"##;
        let e = compile(&project(&[("content/infusion.ydef", kind), ("content/bad.yrecipe", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        for want in ["slots.ring: at most 2 ingredient(s), found 3", "slots.nope: mymod:infusion has no slot `nope` (core, ring)", "slots: mymod:infusion needs `core`", "values.time: a number", "values.heat: mymod:infusion has no value `heat` (time)"] {
            assert!(e.contains(want), "{want}: {e}");
        }
        let undeclared = r#"{ "type": "mymod:infusion", "slots": {}, "result": "@item:minecraft:emerald" }"#;
        let e = compile(&project(&[("content/u.yrecipe", undeclared)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("type `mymod:infusion`: no such kind — declare it: content/infusion.ydef"), "{e}");
        let bad = r#"{ "type": "mymod:infusion", "ingredient": "@item:minecraft:stone", "result": "@item:minecraft:emerald" }"#;
        let e = compile(&project(&[("content/infusion.ydef", kind), ("content/bad.yrecipe", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
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
        let src = c.source.clone().unwrap().all();
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

    #[test]
    fn body_files_compile_and_register() {
        let model = r#"{ "texture": "misc/wings.png", "bones": [
            { "name": "wing", "pivot": [5, 0, 0], "cubes": [{ "origin": [-10, 0, 0], "size": [10, 20, 2], "uv": [22, 0] }] } ] }"#;
        let anim = r#"{ "spread": { "length": 0.5, "bones": { "wing": { "rotation": { "0": [0, 0, 0], "0.5": [0, 0, -90] } } } } }"#;
        let out = compile(&project(&[("content/wings.ymodel", model), ("content/wings.yanim", anim)]), "mymod", &yres::Refs::default()).unwrap();
        assert!(out.data_files.iter().any(|(p, _)| p == "body/models/mymod__wings.ydata"));
        let src = out.source.unwrap().all();
        assert!(src.contains("registry.register_body_model(\"mymod:wings\""), "{src}");
        assert!(src.contains("registry.register_body_anims(\"mymod:wings\""), "{src}");
        assert!(src.contains("pub const WINGS: &'static str = \"mymod:wings\";"), "{src}");
        let bad = r#"{ "texture": "x.png", "bones": [{ "name": "a", "parent": "b" }] }"#;
        let e = compile(&project(&[("content/bad.ymodel", bad)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("must come before it"), "{e}");
    }

    #[test]
    fn font_files_compile_validate_and_register() {
        let font = r#"{ units_per_em: 100, ascent: 80, descent: -20,
            glyphs: [{ name: "a", advance: 100, ops: [] }],
            chars: [{ char: 97, glyph: 0 }] }"#;
        let out = compile(&project(&[("content/type/a.yfont", font)]), "mymod", &yres::Refs::default()).unwrap();
        let (_, bytes) = out.data_files.iter().find(|(p, _)| p == "fonts/mymod__type__a.yfont").unwrap();
        let built = yog_font::Font::from_bytes(bytes).unwrap();
        assert_eq!(built.glyph_of('a'), Some(0));
        let src = out.source.unwrap().part("register").to_string();
        assert!(src.contains("registry.register_font_data(\"mymod:type/a\""), "{src}");
        assert!(src.contains("/generated/fonts/mymod__type__a.yfont"), "{src}");

        let invalid = r#"{ units_per_em: 100, ascent: 80, descent: -20,
            glyphs: [{ advance: 100, ops: [] }],
            chars: [{ char: 97, glyph: 4 }] }"#;
        let error = compile(&project(&[("content/bad.yfont", invalid)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(error.contains("chars[0].glyph 4 does not name a glyph"), "{error}");
    }

    #[test]
    fn entities_compile_register_and_name_their_models() {
        let model = r#"{ "texture": "entity/wisp.png", "bones": [{ "name": "body", "cubes": [{ "origin": [-2, 0, -2], "size": [4, 4, 4] }] }] }"#;
        let wisp = r#"{ "name": "Wisp", "kind": "mob", "width": 0.5, "height": 0.5, "model": "wisp", "goals": ["float", "wander"],
            "attributes": { "max_health": 6, "movement_speed": 0.3 } }"#;
        let out = compile(&project(&[("content/wisp.ymodel", model), ("content/wisp.yentity", wisp)]), "mymod", &yres::Refs::default()).unwrap();
        // Each part in its file; the root only includes them.
        let src = out.source.as_ref().unwrap();
        assert!(src.part("register").contains("registry.register_entity_data(\"mymod:wisp\""), "{src:?}");
        assert!(src.part("entities").contains("pub const WISP: &'static str = \"mymod:wisp\";"), "{src:?}");
        assert!(src.root.contains("\"/generated/content/entities.g.rs\""), "{}", src.root);
        assert!(src.root.lines().all(|l| l.starts_with("//") || l.starts_with("include!(")), "{}", src.root);
        assert!(out.data_files.iter().any(|(p, _)| p == "entities/mymod__wisp.ydata"));
        let (_, data) = out.data_files.iter().find(|(p, _)| p == "entities/mymod__wisp.ydata").unwrap();
        let def: yog_registry::EntityDef = yog_data::read_bytes(data).unwrap();
        assert_eq!((def.kind, def.model.as_deref()), (yog_registry::EntityKind::Mob, Some("mymod:wisp")));
        let missing = r#"{ "kind": "object", "width": 1, "height": 1, "model": "nope" }"#;
        let e = compile(&project(&[("content/thing.yentity", missing)]), "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("model `mymod:nope`: no such file"), "{e}");
    }

}
