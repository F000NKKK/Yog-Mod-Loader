//! `.yblock` / `.yitem` content files — a mod's blocks and items: their
//! properties, 3D model, block states and item form, in one JSON file each,
//! edited in Yog-IDLE's 3D model editor. See Yog-IDLE's
//! `docs/blocks-items.md` for the format.
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
//!   also registers them (unless an entry says `"register": "manual"`).

mod import;
pub mod json;

pub use import::import_command;

use crate::yres;
use json::{num_value, J, V};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Where content files live, under the mod root.
pub const CONTENT_DIR: &str = "content";
/// The generated Rust code, under the mod root.
pub const GENERATED_FILE: &str = ".yog-build/generated/yog-content.g.rs";

/// A generated file: path inside the `.yog` and its bytes.
pub type Output = (String, Vec<u8>);

/// What a content file defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `.yblock` — a block and its item form.
    Block,
    /// `.yitem` — an item.
    Item,
}

impl Kind {
    pub fn of(path: &Path) -> Option<Kind> {
        match path.extension()?.to_str()? {
            "yblock" => Some(Kind::Block),
            "yitem" => Some(Kind::Item),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Kind::Block => "block",
            Kind::Item => "item",
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
pub const TOOLS: [&str; 4] = ["pickaxe", "axe", "shovel", "hoe"];
pub const TOOL_LEVELS: [&str; 3] = ["stone", "iron", "diamond"];
pub const RENDER_LAYERS: [&str; 4] = ["solid", "cutout", "cutout_mipped", "translucent"];

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
        && s
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-'))
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

/// Every `content/**/*.yblock|*.yitem` of the mod at `root`, sorted.
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

/// A name or tooltip.
#[derive(Debug, Clone)]
enum Text {
    /// A `.yres` text: its key.
    Key(String),
    Literal(String),
}

#[derive(Debug, Clone, Default)]
struct BlockProps {
    hardness: Option<f64>,
    resistance: Option<f64>,
    sound: Option<String>,
    light: u8,
    tool: Option<String>,
    tool_level: Option<String>,
    requires_tool: bool,
    no_collision: bool,
    slipperiness: Option<f64>,
    /// `None` = full cube.
    shape: Option<[f64; 6]>,
    connect_groups: Vec<String>,
    connects: bool,
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
}

impl Ctx<'_> {
    fn err(&mut self, line: usize, msg: impl std::fmt::Display) {
        self.errors.push(format!("{}:{line}: {msg}", self.file));
    }

    fn warn(&mut self, line: usize, msg: impl std::fmt::Display) {
        self.warnings.push(format!("{}:{line}: {msg}", self.file));
    }

    /// Reports keys of `obj` not in `known`.
    fn unknown_keys(&mut self, obj: &J, what: &str, known: &[&str]) {
        for (k, v) in obj.members() {
            if !known.contains(&k.as_str()) {
                self.err(
                    v.line,
                    format!("unknown key {k:?} in {what} (expected one of: {})", known.join(", ")),
                );
            }
        }
    }

    fn number(&mut self, j: &J, what: &str, min: f64, max: f64) -> Option<f64> {
        match j.num() {
            Some(n) if (min..=max).contains(&n) => Some(n),
            Some(n) => {
                self.err(j.line, format!("{what} is {n}, must be {min}..{max}"));
                None
            }
            None => {
                self.err(j.line, format!("{what} must be a number, not {}", j.kind()));
                None
            }
        }
    }

    fn boolean(&mut self, j: &J, what: &str) -> Option<bool> {
        let b = j.bool();
        if b.is_none() {
            self.err(j.line, format!("{what} must be true or false"));
        }
        b
    }

    fn string<'j>(&mut self, j: &'j J, what: &str) -> Option<&'j str> {
        let s = j.str();
        if s.is_none() {
            self.err(j.line, format!("{what} must be a string, not {}", j.kind()));
        }
        s
    }

    fn one_of(&mut self, j: &J, what: &str, allowed: &[&str]) -> Option<String> {
        let s = self.string(j, what)?;
        if allowed.contains(&s) {
            Some(s.to_string())
        } else {
            self.err(j.line, format!("{what} {s:?} isn't one of: {}", allowed.join(", ")));
            None
        }
    }

    fn vec<const N: usize>(&mut self, j: &J, what: &str, min: f64, max: f64) -> Option<[f64; N]> {
        let items = j.items();
        if !j.is_arr() || items.len() != N {
            self.err(j.line, format!("{what} must be an array of {N} numbers"));
            return None;
        }
        let mut out = [0.0; N];
        for (i, item) in items.iter().enumerate() {
            out[i] = self.number(item, what, min, max)?;
        }
        Some(out)
    }

    /// A name/tooltip: `@<Set>.<name>` of a `.yres` text, `@@...` for a
    /// literal `@`, or a literal.
    fn text(&mut self, j: &J, what: &str) -> Option<Text> {
        let s = self.string(j, what)?;
        match s.strip_prefix('@') {
            Some(rest) if rest.starts_with('@') => Some(Text::Literal(rest.to_string())),
            Some(rest) => match self.refs.texts.get(rest) {
                Some((key, _)) => Some(Text::Key(key.clone())),
                None => {
                    self.err(j.line, format!("{what} `{s}`: no text `{rest}` in resources/*.yres"));
                    None
                }
            },
            None => Some(Text::Literal(s.to_string())),
        }
    }

    /// A texture: `#slot`, `@<Set>.<name>` (a `.yres` image), a `.png` path
    /// inside `resources/` (bundled), or a texture id (`ns:block/x`,
    /// `block/stone` = `minecraft:block/stone`). → the model's form.
    fn texture(&mut self, j: &J, what: &str) -> Option<String> {
        let s = self.string(j, what)?.trim().to_string();
        if let Some(slot) = s.strip_prefix('#') {
            if slot.is_empty() {
                self.err(j.line, format!("{what}: `#` needs a slot name"));
                return None;
            }
            return Some(s);
        }
        if let Some(rest) = s.strip_prefix('@') {
            return match self.refs.images.get(rest) {
                Some(id) => Some(id.clone()),
                None => {
                    self.err(j.line, format!("{what} `{s}`: no image `{rest}` in resources/*.yres"));
                    None
                }
            };
        }
        if let Some(stem) = s.strip_suffix(".png") {
            let rel = stem.trim_start_matches("./").trim_start_matches('/');
            let rel = rel.strip_prefix("resources/").unwrap_or(rel);
            if rel.split('/').any(|p| p == "..") || !valid_path(rel) {
                self.err(
                    j.line,
                    format!("{what} `{s}`: a path inside resources/ — lower case a-z, 0-9, `_`, `-`, `.`, `/`"),
                );
                return None;
            }
            let file = self.root.join(yres::RESOURCES_DIR).join(format!("{rel}.png"));
            if !file.is_file() {
                self.err(j.line, format!("{what}: resources/{rel}.png doesn't exist"));
                return None;
            }
            let target = format!("assets/{}/textures/{rel}.png", self.ns);
            if let Some(other) = self.textures.insert(target.clone(), file.clone()) {
                if other != file {
                    self.err(j.line, format!("{what}: {target} also comes from {}", display(self.root, &other)));
                }
            }
            let meta = self.root.join(yres::RESOURCES_DIR).join(format!("{rel}.png.mcmeta"));
            if meta.is_file() {
                self.textures.insert(format!("{target}.mcmeta"), meta);
            }
            return Some(format!("{}:{rel}", self.ns));
        }
        if !valid_id(&s) {
            self.err(
                j.line,
                format!("{what} `{s}` isn't a texture — `#slot`, `@Set.image`, `path/in/resources.png` or a texture id like `minecraft:block/stone`"),
            );
            return None;
        }
        let id = if s.contains(':') { s } else { format!("minecraft:{s}") };
        let (ns, path) = id.split_once(':').unwrap_or_default();
        if ns == self.ns {
            let file = self.root.join("assets").join(ns).join("textures").join(format!("{path}.png"));
            let bundled = self.textures.contains_key(&format!("assets/{ns}/textures/{path}.png"))
                || self.refs.images.values().any(|v| *v == id);
            if !file.is_file() && !bundled {
                self.warn(j.line, format!("{what}: texture {id} isn't in assets/{ns}/textures/ or resources/"));
            }
        }
        Some(id)
    }
}

/// A checked model: Minecraft's JSON, its slots, its bounding box.
struct Model {
    json: Value,
    /// Elements' bounding box in pixels, when it has elements.
    bounds: Option<[f64; 6]>,
}

/// Checks a model object → Minecraft's model JSON. `item` = an item model
/// (sprite templates allowed).
fn model(cx: &mut Ctx, j: &J, what: &str, item: bool) -> Option<Model> {
    if !j.is_obj() {
        cx.err(j.line, format!("{what} must be an object"));
        return None;
    }
    cx.unknown_keys(
        j,
        what,
        &["parent", "textures", "elements", "display", "ambient_occlusion", "gui_light"],
    );
    let mut out = Map::new();
    let mut tpl: Option<&Template> = None;
    let mut raw_parent = false;
    if let Some(p) = j.get("parent") {
        let name = cx.string(p, &format!("{what}.parent"))?;
        if let Some(t) = template(name) {
            if t.sprite && !item {
                cx.err(p.line, format!("{what}.parent {name:?} is an item sprite — block models use cube_all, cube_column, ..."));
            }
            out.insert("parent".into(), Value::String(t.parent.into()));
            tpl = Some(t);
        } else if name.contains(':') && valid_id(name) {
            out.insert("parent".into(), Value::String(name.into()));
            raw_parent = true;
        } else {
            let names: Vec<&str> = TEMPLATES.iter().filter(|t| item || !t.sprite).map(|t| t.name).collect();
            cx.err(
                p.line,
                format!("{what}.parent {name:?} isn't a template ({}) or a model id like `minecraft:block/stairs`", names.join(", ")),
            );
            return None;
        }
    }

    // Textures.
    let mut slots: Vec<String> = Vec::new();
    let mut textures = Map::new();
    if let Some(t) = j.get("textures") {
        if !t.is_obj() {
            cx.err(t.line, format!("{what}.textures must be an object of slot → texture"));
        }
        for (slot, v) in t.members() {
            if slot.is_empty() || slot.starts_with('#') {
                cx.err(v.line, format!("{what}.textures: slot names are plain words (`all`, `side`, `0`)"));
                continue;
            }
            if let Some(tex) = cx.texture(v, &format!("{what}.textures.{slot}")) {
                textures.insert(slot.clone(), Value::String(tex));
                slots.push(slot.clone());
            }
        }
    }
    let defined = |slot: &str, textures: &Map<String, Value>| textures.contains_key(slot);
    if let Some(t) = tpl {
        if t.sprite {
            if !defined("layer0", &textures) {
                cx.err(j.line, format!("{what}: parent {:?} needs textures.layer0", t.name));
            }
            for slot in &slots {
                let n = slot.strip_prefix("layer").and_then(|n| n.parse::<usize>().ok());
                match n {
                    Some(n) if n == 0 || defined(&format!("layer{}", n - 1), &textures) => {}
                    Some(_) => cx.err(j.line, format!("{what}: textures.{slot} without the layer before it")),
                    None if slot == "particle" => {}
                    None => cx.warn(j.line, format!("{what}: textures.{slot} isn't used by an item sprite (layer0, layer1, ...)")),
                }
            }
        } else {
            for slot in t.slots {
                if !defined(slot, &textures) {
                    cx.err(j.line, format!("{what}: parent {:?} needs textures.{slot}", t.name));
                }
            }
        }
    }

    // Elements.
    let mut bounds: Option<[f64; 6]> = None;
    if let Some(e) = j.get("elements") {
        if let Some(t) = tpl {
            cx.err(
                e.line,
                format!("{what}: elements replace the shape of parent {:?} — remove one of them", t.name),
            );
        }
        if !e.is_arr() {
            cx.err(e.line, format!("{what}.elements must be an array"));
        }
        let mut elements = Vec::new();
        for (i, el) in e.items().iter().enumerate() {
            if let Some((json, from, to)) = element(cx, el, &format!("{what}.elements[{i}]"), &textures, raw_parent) {
                let b = bounds.get_or_insert([16.0, 16.0, 16.0, 0.0, 0.0, 0.0]);
                for a in 0..3 {
                    b[a] = b[a].min(from[a].min(to[a]));
                    b[a + 3] = b[a + 3].max(from[a].max(to[a]));
                }
                elements.push(json);
            }
        }
        out.insert("elements".into(), Value::Array(elements));
    } else if tpl.is_none() && !raw_parent {
        cx.err(j.line, format!("{what}: no parent and no elements — give a template (\"parent\": \"cube_all\") or elements"));
    }

    // `particle` — the break/run particles. Default: the first texture.
    if !textures.contains_key("particle") && !tpl.is_some_and(|t| t.has_particle) && !raw_parent {
        if let Some(first) = slots.first() {
            let v = textures[first].clone();
            textures.insert("particle".into(), v);
        }
    }
    if !textures.is_empty() {
        out.insert("textures".into(), Value::Object(textures));
    }

    if let Some(d) = j.get("display") {
        if !d.is_obj() {
            cx.err(d.line, format!("{what}.display must be an object"));
        }
        let mut display = Map::new();
        for (slot, t) in d.members() {
            let w = format!("{what}.display.{slot}");
            if !DISPLAY_SLOTS.contains(&slot.as_str()) {
                cx.err(t.line, format!("{w}: unknown display position (expected one of: {})", DISPLAY_SLOTS.join(", ")));
                continue;
            }
            cx.unknown_keys(t, &w, &["rotation", "translation", "scale"]);
            let mut m = Map::new();
            for (key, lim) in [("rotation", 360.0), ("translation", 80.0), ("scale", 4.0)] {
                if let Some(v) = t.get(key) {
                    if let Some(v3) = cx.vec::<3>(v, &format!("{w}.{key}"), -lim, lim) {
                        m.insert(key.into(), Value::Array(v3.iter().map(|n| num_value(*n)).collect()));
                    }
                }
            }
            display.insert(slot.clone(), Value::Object(m));
        }
        out.insert("display".into(), Value::Object(display));
    }
    if let Some(ao) = j.get("ambient_occlusion") {
        if let Some(b) = cx.boolean(ao, &format!("{what}.ambient_occlusion")) {
            out.insert("ambientocclusion".into(), Value::Bool(b));
        }
    }
    if let Some(gl) = j.get("gui_light") {
        if let Some(s) = cx.one_of(gl, &format!("{what}.gui_light"), &["front", "side"]) {
            out.insert("gui_light".into(), Value::String(s));
        }
    }
    Some(Model {
        json: Value::Object(out),
        bounds,
    })
}

/// Checks one element → (its JSON, from, to).
fn element(
    cx: &mut Ctx,
    j: &J,
    what: &str,
    textures: &Map<String, Value>,
    raw_parent: bool,
) -> Option<(Value, [f64; 3], [f64; 3])> {
    if !j.is_obj() {
        cx.err(j.line, format!("{what} must be an object"));
        return None;
    }
    cx.unknown_keys(j, what, &["name", "from", "to", "rotation", "shade", "faces"]);
    if let Some(n) = j.get("name") {
        cx.string(n, &format!("{what}.name"));
    }
    let (Some(from_j), Some(to_j)) = (j.get("from"), j.get("to")) else {
        cx.err(j.line, format!("{what} needs `from` and `to`"));
        return None;
    };
    let from = cx.vec::<3>(from_j, &format!("{what}.from"), -16.0, 32.0)?;
    let to = cx.vec::<3>(to_j, &format!("{what}.to"), -16.0, 32.0)?;
    let mut out = Map::new();
    out.insert("from".into(), Value::Array(from.iter().map(|n| num_value(*n)).collect()));
    out.insert("to".into(), Value::Array(to.iter().map(|n| num_value(*n)).collect()));
    if let Some(r) = j.get("rotation") {
        let w = format!("{what}.rotation");
        cx.unknown_keys(r, &w, &["origin", "axis", "angle", "rescale"]);
        let axis = r.get("axis").and_then(|a| cx.one_of(a, &format!("{w}.axis"), &["x", "y", "z"]));
        let angle = r.get("angle").and_then(|a| cx.number(a, &format!("{w}.angle"), -45.0, 45.0));
        if let Some(a) = angle {
            if (a / 22.5).fract() != 0.0 {
                cx.err(r.line, format!("{w}.angle is {a} — Minecraft rotates elements by -45, -22.5, 0, 22.5 or 45 degrees"));
            }
        }
        let origin = match r.get("origin") {
            Some(o) => cx.vec::<3>(o, &format!("{w}.origin"), -16.0, 32.0),
            None => Some([8.0, 8.0, 8.0]),
        };
        if axis.is_none() || angle.is_none() {
            if r.get("axis").is_none() || r.get("angle").is_none() {
                cx.err(r.line, format!("{w} needs `axis` and `angle`"));
            }
        } else if let (Some(axis), Some(angle), Some(origin)) = (axis, angle, origin) {
            let mut rot = Map::new();
            rot.insert("origin".into(), Value::Array(origin.iter().map(|n| num_value(*n)).collect()));
            rot.insert("axis".into(), Value::String(axis));
            rot.insert("angle".into(), num_value(angle));
            if let Some(rs) = r.get("rescale") {
                if let Some(b) = cx.boolean(rs, &format!("{w}.rescale")) {
                    if b {
                        rot.insert("rescale".into(), Value::Bool(true));
                    }
                }
            }
            out.insert("rotation".into(), Value::Object(rot));
        }
    }
    if let Some(s) = j.get("shade") {
        if let Some(b) = cx.boolean(s, &format!("{what}.shade")) {
            if !b {
                out.insert("shade".into(), Value::Bool(false));
            }
        }
    }
    let mut faces = Map::new();
    match j.get("faces") {
        None => cx.warn(j.line, format!("{what} has no faces — it's invisible")),
        Some(f) => {
            if !f.is_obj() {
                cx.err(f.line, format!("{what}.faces must be an object of direction → face"));
            }
            for (dir, face) in f.members() {
                let w = format!("{what}.faces.{dir}");
                if !DIRECTIONS.contains(&dir.as_str()) {
                    cx.err(face.line, format!("{w}: unknown face (expected one of: {})", DIRECTIONS.join(", ")));
                    continue;
                }
                cx.unknown_keys(face, &w, &["texture", "uv", "rotation", "cullface", "tint"]);
                let mut fo = Map::new();
                match face.get("texture") {
                    None => cx.err(face.line, format!("{w} needs a texture (`#slot`)")),
                    Some(t) => {
                        if let Some(s) = cx.string(t, &format!("{w}.texture")) {
                            let slot = s.strip_prefix('#').unwrap_or(s);
                            if !textures.contains_key(slot) && !raw_parent {
                                cx.err(t.line, format!("{w}.texture `#{slot}`: no such slot in textures"));
                            }
                            fo.insert("texture".into(), Value::String(format!("#{slot}")));
                        }
                    }
                }
                if let Some(uv) = face.get("uv") {
                    if let Some(uv) = cx.vec::<4>(uv, &format!("{w}.uv"), 0.0, 16.0) {
                        fo.insert("uv".into(), Value::Array(uv.iter().map(|n| num_value(*n)).collect()));
                    }
                }
                if let Some(r) = face.get("rotation") {
                    match r.num() {
                        Some(n) if [0.0, 90.0, 180.0, 270.0].contains(&n) => {
                            if n != 0.0 {
                                fo.insert("rotation".into(), num_value(n));
                            }
                        }
                        _ => cx.err(r.line, format!("{w}.rotation must be 0, 90, 180 or 270")),
                    }
                }
                if let Some(c) = face.get("cullface") {
                    if let Some(c) = cx.one_of(c, &format!("{w}.cullface"), &DIRECTIONS) {
                        fo.insert("cullface".into(), Value::String(c));
                    }
                }
                if let Some(t) = face.get("tint") {
                    if let Some(n) = cx.number(t, &format!("{w}.tint"), -1.0, 255.0) {
                        if n >= 0.0 {
                            fo.insert("tintindex".into(), num_value(n.floor()));
                        }
                    }
                }
                faces.insert(dir.clone(), Value::Object(fo));
            }
        }
    }
    out.insert("faces".into(), Value::Object(faces));
    Some((Value::Object(out), from, to))
}

/// The id of a file: `"id"` or the file stem; `ns:` defaults to the mod's.
fn entry_id(cx: &mut Ctx, j: &J, path: &Path, mod_ns: Option<&str>) -> Option<(String, String)> {
    let (raw, line) = match j.get("id") {
        Some(v) => (cx.string(v, "id")?.to_string(), v.line),
        None => (
            path.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
            1,
        ),
    };
    let (ns, p) = match raw.split_once(':') {
        Some((ns, p)) => (ns.to_string(), p.to_string()),
        None => match mod_ns {
            Some(ns) => (ns.to_string(), raw.clone()),
            None => {
                cx.err(
                    line,
                    "the mod id isn't a valid Minecraft namespace (a-z, 0-9, `_`, `-`, `.`), so give the namespace: \"id\": \"mynamespace:name\"",
                );
                return None;
            }
        },
    };
    if !valid_namespace(&ns) || !valid_path(&p) {
        cx.err(
            line,
            format!("id `{ns}:{p}` isn't a valid Minecraft id — lower case a-z, 0-9, `_`, `-`, `.` (and `/` in the path)"),
        );
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
        if w != 1.0 {
            v.insert("weight".into(), num_value(w));
        }
    }
    Value::Object(v)
}

fn pretty(v: &Value) -> Vec<u8> {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s.into_bytes()
}

/// Compiles one file; pushes its outputs.
#[allow(clippy::too_many_arguments)]
fn compile_file(
    cx: &mut Ctx,
    j: &J,
    kind: Kind,
    path: &Path,
    mod_ns: Option<&str>,
    out: &mut Compiled,
    generated: &mut BTreeMap<String, String>,
) -> Option<Entry> {
    if !j.is_obj() {
        cx.err(j.line, format!("a .y{} file is one JSON object", kind.word()));
        return None;
    }
    let common = ["$schema", "format", "id", "name", "tooltip", "register", "model"];
    let specific: &[&str] = match kind {
        Kind::Block => &["block", "item_model", "states"],
        Kind::Item => &["item"],
    };
    let known: Vec<&str> = common.iter().chain(specific).copied().collect();
    cx.unknown_keys(j, &format!("a .y{}", kind.word()), &known);
    if let Some(f) = j.get("format") {
        if f.num() != Some(1.0) {
            cx.err(f.line, "format must be 1 (this yog understands format 1)");
        }
    }
    let (ns, p) = entry_id(cx, j, path, mod_ns)?;
    cx.ns = ns.clone();
    let id = format!("{ns}:{p}");

    let auto = match j.get("register") {
        None => true,
        Some(r) => cx.one_of(r, "register", &["auto", "manual"]).is_none_or(|s| s == "auto"),
    };

    // Name and tooltip → lang, under the keys the game looks up.
    let key = format!("{}.{ns}.{}", kind.word(), p.replace('/', "."));
    for (field, target) in [("name", key.clone()), ("tooltip", format!("{key}.desc"))] {
        let Some(v) = j.get(field) else { continue };
        match cx.text(v, field)? {
            Text::Key(from) if from == target => {}
            Text::Key(from) => out.lang_aliases.push((from, target)),
            Text::Literal(s) => {
                out.lang
                    .entry((ns.clone(), yres::NEUTRAL_LOCALE.to_string()))
                    .or_default()
                    .insert(target, s);
            }
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

    let mut entry = Entry {
        kind,
        id: id.clone(),
        file: cx.file.clone(),
        auto,
        block: BlockProps::default(),
        item: ItemProps::default(),
    };

    match kind {
        Kind::Item => {
            if let Some(it) = j.get("item") {
                cx.unknown_keys(it, "item", &["stack_size", "durability", "fire_resistant", "fuel_ticks", "food"]);
                let props = &mut entry.item;
                if let Some(v) = it.get("stack_size") {
                    props.stack_size = cx.number(v, "item.stack_size", 1.0, 99.0).map(|n| n as u8);
                }
                if let Some(v) = it.get("durability") {
                    props.durability = cx.number(v, "item.durability", 0.0, 1e9).unwrap_or(0.0) as u32;
                }
                if let Some(v) = it.get("fire_resistant") {
                    props.fire_resistant = cx.boolean(v, "item.fire_resistant").unwrap_or(false);
                }
                if let Some(v) = it.get("fuel_ticks") {
                    props.fuel_ticks = cx.number(v, "item.fuel_ticks", 0.0, 1e9).unwrap_or(0.0) as u32;
                }
                if let Some(f) = it.get("food") {
                    cx.unknown_keys(f, "item.food", &["nutrition", "saturation", "always_edible"]);
                    let n = f.get("nutrition").and_then(|v| cx.number(v, "item.food.nutrition", 0.0, 1000.0));
                    let s = f.get("saturation").and_then(|v| cx.number(v, "item.food.saturation", 0.0, 100.0));
                    let a = f.get("always_edible").and_then(|v| cx.boolean(v, "item.food.always_edible"));
                    if n.is_none() {
                        cx.err(f.line, "item.food needs nutrition");
                    }
                    props.food = n.map(|n| (n as u32, s.unwrap_or(0.0), a.unwrap_or(false)));
                }
                if props.durability > 0 && props.stack_size.is_some_and(|s| s > 1) {
                    cx.warn(it.line, "item.stack_size is ignored: an item with durability stacks to 1");
                }
            }
            let Some(mj) = j.get("model") else {
                cx.err(j.line, "an item needs a model — e.g. { \"parent\": \"generated\", \"textures\": { \"layer0\": \"@Textures.ruby\" } }");
                return Some(entry);
            };
            let m = model(cx, mj, "model", true)?;
            emit(cx, format!("models/item/{p}.json"), &m.json);
            emit(
                cx,
                format!("items/{p}.json"),
                &json!({ "model": { "type": "minecraft:model", "model": format!("{ns}:item/{p}") } }),
            );
        }
        Kind::Block => {
            let mut render_layer: Option<String> = None;
            let mut drops = Value::Null;
            let mut shape_auto = true;
            if let Some(b) = j.get("block") {
                cx.unknown_keys(
                    b,
                    "block",
                    &[
                        "hardness", "resistance", "sound", "light", "tool", "tool_level", "requires_tool",
                        "collision", "slipperiness", "shape", "render_layer", "drops", "connect_groups",
                    ],
                );
                let props = &mut entry.block;
                if let Some(v) = b.get("hardness") {
                    props.hardness = cx.number(v, "block.hardness", -1.0, 1e6);
                }
                if let Some(v) = b.get("resistance") {
                    props.resistance = cx.number(v, "block.resistance", 0.0, 1e7);
                }
                if let Some(v) = b.get("sound") {
                    props.sound = cx.one_of(v, "block.sound", &SOUNDS);
                }
                if let Some(v) = b.get("light") {
                    props.light = cx.number(v, "block.light", 0.0, 15.0).unwrap_or(0.0) as u8;
                }
                if let Some(v) = b.get("tool") {
                    props.tool = cx.one_of(v, "block.tool", &TOOLS);
                }
                if let Some(v) = b.get("tool_level") {
                    props.tool_level = cx.one_of(v, "block.tool_level", &TOOL_LEVELS);
                }
                if let Some(v) = b.get("requires_tool") {
                    props.requires_tool = cx.boolean(v, "block.requires_tool").unwrap_or(false);
                }
                if let Some(v) = b.get("collision") {
                    props.no_collision = !cx.boolean(v, "block.collision").unwrap_or(true);
                }
                if let Some(v) = b.get("slipperiness") {
                    props.slipperiness = cx.number(v, "block.slipperiness", 0.0, 1.0);
                }
                if let Some(v) = b.get("shape") {
                    match &v.v {
                        V::Str(s) if s == "auto" => {}
                        V::Str(s) if s == "full" => shape_auto = false,
                        V::Arr(_) => {
                            shape_auto = false;
                            props.shape = cx.vec::<6>(v, "block.shape", 0.0, 16.0);
                        }
                        _ => cx.err(v.line, "block.shape must be \"auto\" (the model's bounds), \"full\" or [x1, y1, z1, x2, y2, z2] in pixels"),
                    }
                }
                if let Some(v) = b.get("render_layer") {
                    render_layer = cx.one_of(v, "block.render_layer", &RENDER_LAYERS);
                }
                if let Some(v) = b.get("connect_groups") {
                    for g in v.items() {
                        if let Some(s) = cx.string(g, "block.connect_groups[]") {
                            props.connect_groups.push(s.to_string());
                        }
                    }
                    if !v.is_arr() {
                        cx.err(v.line, "block.connect_groups must be an array of group names");
                    }
                }
                if props.tool_level.is_some() && props.tool.is_none() {
                    cx.warn(b.line, "block.tool_level without block.tool — any tool of that level mines it");
                }
                if let Some(d) = b.get("drops") {
                    drops = drops_table(cx, d, &id);
                }
            }
            if drops.is_null() && !j.get("block").and_then(|b| b.get("drops")).is_some() {
                drops = drops_table_self(&id);
            }

            let Some(mj) = j.get("model") else {
                cx.err(j.line, "a block needs a model — e.g. { \"parent\": \"cube_all\", \"textures\": { \"all\": \"@Textures.ruby_block\" } }");
                return Some(entry);
            };
            let mut m = model(cx, mj, "model", false)?;
            if let (Some(layer), Value::Object(o)) = (&render_layer, &mut m.json) {
                // Forge/NeoForge read it from the model; Fabric ignores it.
                o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
            }
            if shape_auto {
                entry.block.shape = m.bounds.filter(|b| *b != [0.0, 0.0, 0.0, 16.0, 16.0, 16.0]).map(|b| {
                    [b[0].max(0.0), b[1].max(0.0), b[2].max(0.0), b[3].min(16.0), b[4].min(16.0), b[5].min(16.0)]
                });
            }
            let main = format!("{ns}:block/{p}");
            emit(cx, format!("models/block/{p}.json"), &m.json);

            // Block states.
            let mut rotations: Vec<Value> = Vec::new();
            let mut arm: Option<(String, bool)> = None;
            if let Some(s) = j.get("states") {
                cx.unknown_keys(s, "states", &["rotations", "connect"]);
                if let Some(r) = s.get("rotations") {
                    if !r.is_arr() || r.items().is_empty() {
                        cx.err(r.line, "states.rotations must be a non-empty array of { \"x\": 0, \"y\": 90, \"weight\": 1, \"uvlock\": false }");
                    }
                    for (i, rot) in r.items().iter().enumerate() {
                        let w = format!("states.rotations[{i}]");
                        cx.unknown_keys(rot, &w, &["x", "y", "weight", "uvlock"]);
                        let mut angle = |k: &str| -> f64 {
                            match rot.get(k).map(|v| (v, v.num())) {
                                None => 0.0,
                                Some((_, Some(n))) if [0.0, 90.0, 180.0, 270.0].contains(&n) => n,
                                Some((v, _)) => {
                                    cx.err(v.line, format!("{w}.{k} must be 0, 90, 180 or 270"));
                                    0.0
                                }
                            }
                        };
                        let (x, y) = (angle("x"), angle("y"));
                        let weight = rot.get("weight").and_then(|v| cx.number(v, &format!("{w}.weight"), 1.0, 1e6));
                        let uvlock = rot.get("uvlock").and_then(|v| cx.boolean(v, &format!("{w}.uvlock"))).unwrap_or(false);
                        rotations.push(variant(&main, x, y, uvlock, weight));
                    }
                }
                if let Some(c) = s.get("connect") {
                    cx.unknown_keys(c, "states.connect", &["arm", "uvlock"]);
                    let uvlock = c.get("uvlock").and_then(|v| cx.boolean(v, "states.connect.uvlock")).unwrap_or(false);
                    match c.get("arm") {
                        None => cx.err(c.line, "states.connect needs `arm` — the model of the arm towards north"),
                        Some(a) => {
                            if let Some(am) = model(cx, a, "states.connect.arm", false) {
                                let mut json = am.json;
                                if let (Some(layer), Value::Object(o)) = (&render_layer, &mut json) {
                                    o.insert("render_type".into(), Value::String(format!("minecraft:{layer}")));
                                }
                                emit(cx, format!("models/block/{p}_arm.json"), &json);
                                arm = Some((format!("{ns}:block/{p}_arm"), uvlock));
                            }
                        }
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
                    for (dir, x, y) in [
                        ("north", 0.0, 0.0),
                        ("east", 0.0, 90.0),
                        ("south", 0.0, 180.0),
                        ("west", 0.0, 270.0),
                        ("up", 270.0, 0.0),
                        ("down", 90.0, 0.0),
                    ] {
                        parts.push(json!({ "when": { dir: "true" }, "apply": variant(arm, x, y, *uvlock, None) }));
                    }
                    json!({ "multipart": parts })
                }
            };
            emit(cx, format!("blockstates/{p}.json"), &blockstate);

            // The item form: the block model, or its own.
            let item_model = match j.get("item_model") {
                None => Some(json!({ "parent": main })),
                Some(im) => model(cx, im, "item_model", true).map(|m| m.json),
            };
            if let Some(im) = item_model {
                emit(cx, format!("models/item/{p}.json"), &im);
            }
            emit(
                cx,
                format!("items/{p}.json"),
                &json!({ "model": { "type": "minecraft:model", "model": format!("{ns}:item/{p}") } }),
            );

            if !drops.is_null() {
                let full = format!("data/{ns}/loot_table/blocks/{p}.json");
                let plural = format!("data/{ns}/loot_tables/blocks/{p}.json");
                if cx.root.join(&full).is_file() || cx.root.join(&plural).is_file() {
                    cx.err(1, format!("{full} is written by this file (block.drops) — delete the hand-written one, or set \"drops\": \"none\""));
                }
                out.outputs.push((full, pretty(&drops)));
            }
            if let Some(tool) = &entry.block.tool {
                out.tags
                    .entry(format!("data/minecraft/tags/block/mineable/{tool}.json"))
                    .or_default()
                    .insert(id.clone());
            }
            if let Some(level) = &entry.block.tool_level {
                out.tags
                    .entry(format!("data/minecraft/tags/block/needs_{level}_tool.json"))
                    .or_default()
                    .insert(id.clone());
            }
        }
    }
    Some(entry)
}

fn drops_table_self(id: &str) -> Value {
    json!({
        "type": "minecraft:block",
        "pools": [{
            "rolls": 1,
            "entries": [{ "type": "minecraft:item", "name": id }],
            "conditions": [{ "condition": "minecraft:survives_explosion" }]
        }]
    })
}

/// `"self"`, `"none"` or `{ "item": "ns:x", "count": 2 | [1, 3] }`.
fn drops_table(cx: &mut Ctx, d: &J, id: &str) -> Value {
    match &d.v {
        V::Str(s) if s == "self" => drops_table_self(id),
        V::Str(s) if s == "none" => Value::Null,
        V::Obj(_) => {
            cx.unknown_keys(d, "block.drops", &["item", "count"]);
            let item = match d.get("item") {
                Some(i) => match cx.string(i, "block.drops.item") {
                    Some(s) if valid_id(s) => {
                        if s.contains(':') {
                            s.to_string()
                        } else {
                            format!("minecraft:{s}")
                        }
                    }
                    Some(s) => {
                        cx.err(i.line, format!("block.drops.item `{s}` isn't an item id"));
                        return Value::Null;
                    }
                    None => return Value::Null,
                },
                None => id.to_string(),
            };
            let mut entry = json!({ "type": "minecraft:item", "name": item });
            if let Some(c) = d.get("count") {
                let count = match &c.v {
                    V::Num(_) => cx.number(c, "block.drops.count", 1.0, 64.0).map(num_value),
                    V::Arr(_) => cx.vec::<2>(c, "block.drops.count", 0.0, 64.0).map(|[min, max]| {
                        json!({ "type": "minecraft:uniform", "min": num_value(min), "max": num_value(max) })
                    }),
                    _ => {
                        cx.err(c.line, "block.drops.count must be a number or [min, max]");
                        None
                    }
                };
                if let Some(count) = count {
                    entry["functions"] = json!([{ "function": "minecraft:set_count", "count": count }]);
                }
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
        _ => {
            cx.err(d.line, "block.drops must be \"self\", \"none\" or { \"item\": \"ns:item\", \"count\": [1, 3] }");
            Value::Null
        }
    }
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
        let j = match json::parse(&text) {
            Ok(j) => j,
            Err(e) => {
                errors.push(format!("{shown}:{}", e.trim_start_matches("line ")));
                continue;
            }
        };
        let mut cx = Ctx {
            root,
            refs,
            file: shown.clone(),
            ns: mod_ns.unwrap_or("minecraft").to_string(),
            errors: &mut errors,
            warnings: &mut warnings,
            textures: &mut textures,
        };
        if let Some(entry) = compile_file(&mut cx, &j, kind, path, mod_ns, &mut out, &mut generated) {
            if let Some(other) = ids.insert(entry.id.clone(), shown.clone()) {
                errors.push(format!("{shown}: id {} is also defined by {other}", entry.id));
            }
            entries.push(entry);
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
        return Err(format!("invalid blocks/items in content/:\n  {}", errors.join("\n  ")));
    }
    out.blocks = entries.iter().filter(|e| e.kind == Kind::Block).count();
    out.items = entries.len() - out.blocks;
    if entries.iter().any(|e| e.auto) && !mentions_content_macro(&root.join("src")) {
        out.warnings.push(format!(
            "{} has blocks/items, but src/ never calls `yog_api::content!();` — they get models but aren't registered",
            CONTENT_DIR
        ));
    }
    out.source = Some(rust_source(&entries));
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
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
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

fn rust_source(entries: &[Entry]) -> String {
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
            if kind == Kind::Block { "blocks" } else { "items" }
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
            let ident = if is_keyword(&ident) { format!("r#{ident}") } else { ident };
            let _ = writeln!(code, "    /// `{}` — `{}`.\n    pub const {upper}: &'static str = {:?};", e.id, e.file, e.id);
            let mut b = format!("::yog_api::{def}::new(Self::{upper})");
            match kind {
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
                        let groups: Vec<String> = p.connect_groups.iter().map(|g| format!("{g:?}")).collect();
                        let _ = write!(b, "\n            .connect_groups(&[{}])", groups.join(", "));
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
        let ident = if is_keyword(&ident) { format!("r#{ident}") } else { ident };
        if !e.auto {
            continue;
        }
        let _ = match e.kind {
            Kind::Block => writeln!(code, "    registry.register_block(Blocks::{ident}());"),
            Kind::Item => writeln!(code, "    registry.register_item(Items::{ident}());"),
        };
    }
    code.push_str("}\n");
    code
}

fn is_keyword(s: &str) -> bool {
    const KW: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl",
        "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct",
        "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract",
        "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
        "gen",
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
    if std::fs::read_to_string(&out).ok().as_deref() != Some(code.as_str()) {
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&out, code).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(())
}

/// A JSON object output at `path`: the one already in `outputs`, else the
/// mod's own file there, else empty.
fn json_output(outputs: &[Output], root: &Path, path: &str) -> Result<Map<String, Value>, String> {
    let bytes = match outputs.iter().rev().find(|(p, _)| p == path) {
        Some((_, b)) => b.clone(),
        None => match std::fs::read(root.join(path)) {
            Ok(b) => b,
            Err(_) => return Ok(Map::new()),
        },
    };
    serde_json::from_slice::<Map<String, Value>>(&bytes).map_err(|e| format!("{path}: {e}"))
}

fn put_output(outputs: &mut Vec<Output>, path: String, bytes: Vec<u8>) {
    outputs.retain(|(p, _)| *p != path);
    outputs.push((path, bytes));
}

/// Adds the content's outputs to `outputs` (the `.yres` ones, say): lang
/// entries and tags merged into what's there (or into the mod's own files),
/// the rest appended.
pub fn merge_outputs(root: &Path, outputs: &mut Vec<Output>, compiled: Compiled) -> Result<(), String> {
    let mut errors = Vec::new();
    // `.yres` texts under the keys the game looks up: in every lang file
    // that has the text.
    if !compiled.lang_aliases.is_empty() {
        let mut lang_files: BTreeSet<String> = outputs
            .iter()
            .map(|(p, _)| p.clone())
            .filter(|p| is_lang_path(p))
            .collect();
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
            let mut map = match json_output(outputs, root, &path) {
                Ok(m) => m,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            let mut changed = false;
            for (from, to) in &compiled.lang_aliases {
                if let Some(text) = map.get(from).cloned() {
                    if !map.contains_key(to) {
                        map.insert(to.clone(), text);
                        changed = true;
                    }
                }
            }
            if changed {
                put_output(outputs, path, pretty(&Value::Object(map)));
            }
        }
    }
    for ((ns, locale), entries) in &compiled.lang {
        let path = format!("assets/{ns}/lang/{locale}.json");
        let mut map = match json_output(outputs, root, &path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        for (k, v) in entries {
            map.entry(k.clone()).or_insert_with(|| Value::String(v.clone()));
        }
        put_output(outputs, path, pretty(&Value::Object(map)));
    }
    for (path, ids) in &compiled.tags {
        // The mod's own tag file may use the pre-1.21 spelling; the loader
        // maps both to one file, so merge into whichever it has.
        let plural = path.replacen("/tags/block/", "/tags/blocks/", 1);
        let path = if !outputs.iter().any(|(p, _)| p == path) && root.join(&plural).is_file() {
            plural
        } else {
            path.clone()
        };
        let mut map = match json_output(outputs, root, &path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let values = map.entry("values").or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(list) = values {
            for id in ids {
                if !list.iter().any(|v| v.as_str() == Some(id)) {
                    list.push(Value::String(id.clone()));
                }
            }
        }
        map.entry("replace").or_insert(Value::Bool(false));
        put_output(outputs, path, pretty(&Value::Object(map)));
    }
    for (path, bytes) in compiled.outputs {
        put_output(outputs, path, bytes);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("merging blocks/items into the mod's files:\n  {}", errors.join("\n  ")))
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

    fn output<'a>(c: &'a Compiled, path: &str) -> Value {
        let (_, b) = c.outputs.iter().find(|(p, _)| p == path).unwrap_or_else(|| {
            panic!("no {path} in {:?}", c.outputs.iter().map(|(p, _)| p).collect::<Vec<_>>())
        });
        serde_json::from_slice(b).unwrap()
    }

    #[test]
    fn compiles_a_cube_block() {
        let root = project(&[
            ("resources/block/ruby_ore.png", "png"),
            (
                "content/blocks/ruby_ore.yblock",
                r#"{
  "name": "Ruby Ore",
  "block": { "hardness": 3, "resistance": 3, "sound": "stone", "tool": "pickaxe", "tool_level": "iron", "requires_tool": true,
             "drops": { "item": "mymod:ruby", "count": [1, 3] } },
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
        assert_eq!(output(&c, "assets/mymod/models/item/ruby_ore.json"), json!({ "parent": "mymod:block/ruby_ore" }));
        assert_eq!(
            output(&c, "assets/mymod/items/ruby_ore.json")["model"]["model"],
            json!("mymod:item/ruby_ore")
        );
        let loot = output(&c, "data/mymod/loot_table/blocks/ruby_ore.json");
        assert_eq!(loot["pools"][0]["entries"][0]["name"], json!("mymod:ruby"));
        assert_eq!(loot["pools"][0]["entries"][0]["functions"][0]["count"]["max"], json!(3));
        assert!(c.outputs.iter().any(|(p, _)| p == "assets/mymod/textures/block/ruby_ore.png"));
        let src = c.source.as_ref().unwrap();
        assert!(src.contains("pub const RUBY_ORE: &'static str = \"mymod:ruby_ore\";"), "{src}");
        assert!(src.contains(".strength(3.0, 3.0)"), "{src}");
        assert!(src.contains(".requires_tool()"), "{src}");
        assert!(src.contains("registry.register_block(Blocks::ruby_ore());"), "{src}");

        let mut outputs = Vec::new();
        merge_outputs(&root, &mut outputs, c).unwrap();
        let find = |p: &str| -> Value {
            serde_json::from_slice(&outputs.iter().find(|(q, _)| q == p).unwrap().1).unwrap()
        };
        assert_eq!(find("assets/mymod/lang/en_us.json")["block.mymod.ruby_ore"], json!("Ruby Ore"));
        assert_eq!(find("data/minecraft/tags/block/mineable/pickaxe.json")["values"], json!(["mymod:ruby_ore"]));
        assert_eq!(find("data/minecraft/tags/block/needs_iron_tool.json")["values"], json!(["mymod:ruby_ore"]));
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
        assert!(src.contains(".shape(6.0, 6.0, 6.0, 10.0, 10.0, 10.0)"), "{src}");
        assert!(src.contains(".connects_to_neighbors()"), "{src}");
        assert!(src.contains(".connect_groups(&[\"mymod:pipe\"])"), "{src}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reports_errors_with_lines() {
        let root = project(&[(
            "content/bad.yblock",
            "{\n  \"model\": {\n    \"textures\": { \"0\": \"minecraft:block/stone\" },\n    \"elements\": [ {\n      \"from\": [0, 0, 0], \"to\": [16, 16, 16],\n      \"rotation\": { \"axis\": \"q\", \"angle\": 30 },\n      \"faces\": { \"north\": { \"texture\": \"#1\" } }\n    } ]\n  },\n  \"bogus\": 1\n}",
        )]);
        let e = compile(&root, "mymod", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("content/bad.yblock:6: model.elements[0].rotation.axis"), "{e}");
        assert!(e.contains("content/bad.yblock:6: model.elements[0].rotation.angle is 30"), "{e}");
        assert!(e.contains("content/bad.yblock:7: model.elements[0].faces.north.texture `#1`"), "{e}");
        assert!(e.contains("content/bad.yblock:10: unknown key \"bogus\""), "{e}");
        let _ = std::fs::remove_dir_all(root);
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
        refs.texts.insert("Items.ruby".into(), ("item.mymod.ruby".into(), "Ruby".into()));
        refs.texts.insert("Strings.ruby_tip".into(), ("mymod.ruby_tip".into(), "Shiny".into()));
        refs.images.insert("Textures.ruby".into(), "mymod:item/ruby".into());
        let c = compile(&root, "mymod", &refs).unwrap();
        assert_eq!(
            output(&c, "assets/mymod/models/item/ruby.json"),
            json!({ "parent": "minecraft:item/generated", "textures": { "layer0": "mymod:item/ruby" } })
        );
        let src = c.source.clone().unwrap();
        assert!(src.contains(".max_stack(16)"), "{src}");
        assert!(src.contains(".food(::yog_api::FoodDef::new(4, 0.3))"), "{src}");
        // The .yres lang file gets the tooltip under the key the game reads.
        let mut outputs = vec![(
            "assets/mymod/lang/ru_ru.json".to_string(),
            br#"{"item.mymod.ruby": "Рубин", "mymod.ruby_tip": "Блестит"}"#.to_vec(),
        )];
        merge_outputs(&root, &mut outputs, c).unwrap();
        let ru: Value = serde_json::from_slice(&outputs[0].1).unwrap();
        assert_eq!(ru["item.mymod.ruby.desc"], json!("Блестит"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn namespace_needed_for_odd_mod_ids() {
        let root = project(&[("content/x.yitem", r#"{ "model": { "parent": "generated", "textures": { "layer0": "minecraft:item/apple" } } }"#)]);
        let e = compile(&root, "HexMod-Yog", &yres::Refs::default()).unwrap_err();
        assert!(e.contains("give the namespace"), "{e}");
        let _ = std::fs::remove_dir_all(root);
    }
}
