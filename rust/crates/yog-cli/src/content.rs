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
//! - builds the mod's textures (`.ytexture`, see `textures`);
//! - generates `.yog-build/generated/yog-content.g.rs` — `Blocks`/`Items`
//!   with an id constant and a `BlockDef`/`ItemDef` builder per entry and
//!   a `register` function — pulled in by `yog_api::content!();`, which
//!   also registers them (unless an entry says `"register": "manual"`) and
//!   every recipe.

mod block_item;
mod body;
mod book;
mod datapack;
mod defs;
mod import;
pub mod json;
mod model;
mod outputs;
mod recipe;
mod refs;
mod source;
mod table;
mod textures;

pub use import::import_command;
pub use source::write_source;
pub use outputs::merge_outputs;

use yog_ids::Format;
use crate::yres;
use yog_data::Path as At;
use json::num_value;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use yog_data::Schema;
use crate::content::block_item::compile_file;
use crate::content::block_item::inventory_screens;
use crate::content::body::{BodyCx, compile_body};
use crate::content::body::compile_entity;
use crate::content::body::compile_font;
use crate::content::book::compile_book;
use crate::content::book::compile_layout;
use crate::content::book::recipe_layouts;
use crate::content::defs::definition_path;
use crate::content::defs::dimension_path;
use crate::content::defs::loot_path;
use crate::content::defs::read_defs;
use crate::content::defs::tool_level_tags;
use crate::content::outputs::version_overlays;
use crate::content::recipe::compile_recipe;
use crate::content::source::float;
use crate::content::source::mentions_content_macro;
use crate::content::source::registrations;
use crate::content::source::{Content, rust_source};

/// Where content files live, under the mod root.
pub const CONTENT_DIR: &str = "content";
/// The generated Rust code, under the mod root.
pub const GENERATED_FILE: &str = ".yog-build/generated/yog-content.g.rs";

/// A generated file: path inside the `.yog` and its bytes.
pub type Output = (String, Vec<u8>);

/// The formats a content file under `content/` is written in.
const CONTENT: [Format; 9] = [
    Format::Block,
    Format::Item,
    Format::Recipe,
    Format::Book,
    Format::Def,
    Format::Model,
    Format::Anim,
    Format::Entity,
    Format::Font,
];

/// The content format of the file at `path`.
fn content_format(path: &Path) -> Option<Format> {
    Format::of_path(path).filter(|f| CONTENT.contains(f))
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
    /// The pictures (`.yres` images' files) an atlas took in: left out of the build.
    pub packed: BTreeSet<String>,
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

/// Every content file (`CONTENT`) under `content/` of the mod at `root`, sorted.
pub fn find(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if content_format(&path).is_some() {
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
    kind: Format,
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
    /// The mod's own textures' ids (`.ytexture` files).
    own_textures: &'a BTreeSet<String>,
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

    /// A texture → the model's form: `#slot`, or a texture id — the mod's
    /// own must be one of its `.ytexture`s.
    fn texture(&mut self, loc: Loc, path: &At, t: &yog_formats::content::Texture) -> Option<String> {
        use yog_formats::content::{SLOT_MARK, Texture};
        match t {
            Texture::Slot(slot) => Some(format!("{SLOT_MARK}{slot}")),
            Texture::Ref(id) => {
                let (ns, tex) = yog_ids::split(id);
                if ns == self.ns && !self.own_textures.contains(id) {
                    self.err_at(loc, path, format!("`{}{id}`: no {}/{tex}.{}", yog_ids::reference::TEXTURE, yres::RESOURCES_DIR, Format::Texture.ext()));
                    return None;
                }
                Some(id.clone())
            }
        }
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

fn pretty(v: &Value) -> Vec<u8> {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s.into_bytes()
}

/// What a recipe takes.
#[derive(Debug, Clone)]
enum RecipeKind {
    /// A crafting grid pattern: rows of key characters (space = empty);
    /// each key any of several items.
    Shaped { pattern: Vec<String>, key: Vec<(char, Vec<String>)> },
    /// Crafting ingredients in any arrangement.
    Shapeless(Vec<Vec<String>>),
    Cooking { method: yog_registry::Cooking, input: Vec<String>, experience: f64, cook_time: u32 },
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
    /// The book as the runtime reads it (`yog_formats::book::Book`'s schema), as
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

impl BodyFile {
    /// Its file under `.yog-build/generated/`.
    fn path(&self) -> String {
        let dir = if self.model { "models" } else { "anims" };
        format!("body/{dir}/{}.ydata", self.id.replace([':', '/'], "__"))
    }
}

impl BookFile {
    /// Its file under `.yog-build/generated/`.
    fn path(&self) -> String {
        format!("books/{}.ydata", self.id.replace(':', "__"))
    }
}

/// The mod's lang: (namespace, locale) → key → text.
type Lang = BTreeMap<(String, String), BTreeMap<String, String>>;

/// A deferred check of a tag of content `<kinds>`: `tag:item`, `tag:block|item`.
const TAG_REF: &str = "tag:";

/// What an ore makes: its files, and the biome feature (feature, biomes, step) for Fabric's host.
type OreOut = (Vec<Output>, (String, String, String));

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
                "Cooking {{ method: ::yog_api::Cooking::{method:?}, ingredient: {}, experience: {}_f32, cook_time: {cook_time} }}",
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
    let own_textures = textures::compile(root, mod_id, refs, &mut errors);
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
        let Some(kind) = content_format(path) else { continue };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{shown}: {e}"));
                continue;
            }
        };
        if matches!(kind, Format::Model | Format::Anim) {
            if let Some(b) = compile_body(&BodyCx { root, ns: mod_ns.unwrap_or(yog_ids::ns::MINECRAFT), own_textures: &own_textures.ids }, &shown, &text, kind, path, &mut errors) {
                if let Some(other) = body_ids.insert((kind == Format::Model, b.id.clone()), shown.clone()) {
                    errors.push(format!("{shown}: {} {} is also defined by {other}", kind.noun(), b.id));
                }
                bodies.push(b);
            }
            continue;
        }
        if kind == Format::Font {
            if let Some(font) = compile_font(&shown, &text, root, path, mod_ns.unwrap_or("minecraft"), &mut errors) {
                if let Some(other) = font_ids.insert(font.id.clone(), shown.clone()) {
                    errors.push(format!("{shown}: font {} is also defined by {other}", font.id));
                }
                fonts.push(font);
            }
            continue;
        }
        if kind == Format::Book {
            // Checked once every block, item and recipe is known.
            pending_books.push((shown.clone(), text));
            continue;
        }
        if kind == Format::Recipe {
            // Checked once every block and item is known.
            pending_recipes.push((shown.clone(), text, path.clone()));
            continue;
        }
        if kind == Format::Def {
            // Kinds of recipe are checked with the recipes, ores and dimensions with what they name.
            let of = yog_data::text::parse_value(&text).ok().and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_string));
            if matches!(of.as_deref(), Some(yog_ids::kind::RECIPE_KIND | yog_ids::kind::ORE | yog_ids::kind::DIMENSION | yog_ids::kind::TABLE | yog_ids::kind::TAG | yog_ids::kind::DAMAGE_TYPE | yog_ids::kind::TREE | yog_ids::kind::LOOT_MODIFIER | yog_ids::kind::LOOT_TABLE | yog_ids::kind::ATTRIBUTE | yog_ids::kind::EFFECT | yog_ids::kind::ADVANCEMENT)) {
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
            own_textures: &own_textures.ids,
            inventories: &mut inventories,
            deferred: &mut deferred,
        };
        if kind == Format::Entity {
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
    let manifest = crate::manifest::YogToml::read(&root.join(yog_ids::file::MANIFEST)).ok();
    let externals = manifest.as_ref().map(|t| t.externals.clone()).unwrap_or_default();
    let datapack = manifest.as_ref().and_then(|t| t.datapack.clone());
    let mut known = refs::Known::new(mod_ns.unwrap_or("minecraft"), &externals, datapack.as_deref());
    for e in &entries {
        known.add(if e.kind == Format::Block { refs::RefKind::Block } else { refs::RefKind::Item }, e.id.clone());
    }
    for id in potion_defs.keys() {
        known.add(refs::RefKind::Item, id.clone());
    }
    for e in &entity_files {
        known.add(refs::RefKind::Entity, e.id.clone());
        // A model or clips of this mod must be among its `.ymodel`/`.yanim`.
        for (wanted, model, what) in [(&e.model, true, "model"), (&e.animations, false, "animations")] {
            if let Some(id) = wanted
                && mod_ns.is_some_and(|ns| id.starts_with(&format!("{ns}:")))
                && !bodies.iter().any(|b| b.model == model && &b.id == id)
            {
                errors.push(format!("{}: {what} `{id}`: no such file in content/ (`.{}`)", e.file, if model { Format::Model } else { Format::Anim }.ext()));
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
            own_textures: &own_textures.ids,
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
            own_textures: &own_textures.ids,
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
        if !potion_defs.contains_key(&brew.from) && !refs::is_vanilla_potion(&brew.from) {
            errors.push(format!("brewing recipe input `{}`: no potion `.yitem` or Yog default potion alias defines it", brew.from));
            continue;
        }
        if let Some(potion) = potion_defs.get_mut(&to) {
            potion.brewed.push(brew);
        } else if refs::is_vanilla_potion(&to) {
            potion_defs
                .entry(to.clone())
                .or_insert_with(|| yog_registry::PotionDef { id: to, name: None, effects: Vec::new(), brewed: Vec::new() })
                .brewed
                .push(brew);
        } else {
            errors.push(format!("brewing recipe result `{to}`: no potion `.yitem` or Yog default potion alias defines it"));
        }
    }
    // Definitions: the ladders and their levels, sound groups.
    let mut defs = read_defs(&pending_defs, mod_ns, &mut errors);
    definitions.extend(potion_defs.into_iter().map(|(id, potion)| (yog_ids::kind::POTION, id, yog_data::binary::encode(&potion.write()))));
    for (files, feature) in ores {
        out.outputs.extend(files);
        defs.biome_features.push(feature);
    }
    // What blocks and items name (drops, levels): known now.
    for (file, line, path, r) in &deferred {
        let found = match r.kind.as_str() {
            yog_ids::kind::LEVEL => defs.level(&r.id).map(|_| ()).ok_or_else(|| format!("no level {} — the game's tool levels or a .ydef's", r.id)),
            yog_ids::kind::LADDER => defs.order.contains_key(&r.id).then_some(()).ok_or_else(|| format!("no ladder {}", r.id)),
            yog_ids::kind::TOOL_MATERIAL => (defs.tool_materials.iter().any(|m| m.id == r.id) || yog_formats::def::ToolMaterial::built_in(&r.id).is_some())
                .then_some(())
                .ok_or_else(|| format!("no tool material {} — the game's or a .ydef's", r.id)),
            yog_ids::kind::ARMOR_MATERIAL => (defs.armor_materials.iter().any(|m| m.id == r.id) || yog_formats::def::ArmorMaterial::built_in(&r.id).is_some())
                .then_some(())
                .ok_or_else(|| format!("no armor material {} — the game's or a .ydef's", r.id)),
            yog_ids::kind::SOUND_GROUP => (defs.sound_groups.iter().any(|g| g.id == r.id) || yog_formats::def::SoundGroup::built_in(&r.id).is_some())
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
            own_textures: &own_textures.ids,
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
    out.outputs.extend(own_textures.outputs);
    out.packed = own_textures.packed;
    out.warnings = warnings;
    if !errors.is_empty() {
        return Err(format!(
            "invalid blocks/items in content/:\n  {}",
            errors.join("\n  ")
        ));
    }
    out.blocks = entries.iter().filter(|e| e.kind == Format::Block).count();
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
        if yog_ids::split(&m.incorrect).0 != yog_ids::ns::MINECRAFT {
            out.tags.entry(datapack::overlay(&yog_registry::ToolLadder::game().incorrect_when, &datapack::tag_file(yog_ids::kind::BLOCK, &m.incorrect))).or_default();
        }
    }
    defs.dimensions = dimensions.iter().map(|(id, _)| id.clone()).collect();
    defs.tables = tables;
    out.source = Some(rust_source(&Content {
        entries: &entries,
        recipes: &recipes,
        books: &books,
        bodies: &bodies,
        entities: &entity_files,
        fonts: &fonts,
        defs: &defs,
        textures: &own_textures.ids,
    }));
    version_overlays(&mut out, &known.datapack);
    out.registrations = registrations(&entries, &entity_files, &dimensions, &defs);
    Ok(out)
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

/// Where the mod's tags are listed for tools (`#ns:path` → its entries, by
/// kind) — the IDE shows a tag's items from here, as the build makes them.
pub const TAGS_FILE: &str = ".yog-build/generated/tags.json";

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

#[cfg(test)]
mod tests;
