//! The `.yitem` and `.yblock` formats in Yog Data: an item or a block, its
//! model (a template or elements) and properties. `yog build` turns them
//! into the game's models, block states, loot tables and the Rust glue.

use yog_data::kinds::Item;
use yog_data::with::{Opt, RefTo};
use yog_data::{Ordered, Reader, Res, Schema, Shape, Text, Value, ValueRef};

use crate::def::{ArmorMaterialKind, LevelKind, SoundGroupKind, TOOL_LEVELS, ToolMaterialKind};

/// Registered by `yog_api::content!()` (`auto`), or by the mod's code.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Register {
    #[default]
    Auto,
    Manual,
}

/// What eating it gives.
#[derive(Schema, Debug, Clone)]
pub struct Food {
    #[data(range = "0..=1000")]
    pub nutrition: u32,
    #[data(default, range = "0..=100")]
    pub saturation: f64,
    /// Eaten even when not hungry.
    #[data(default)]
    pub always_edible: bool,
}

/// An item's properties.
#[derive(Schema, Debug, Clone, Default)]
pub struct ItemProps {
    /// How many stack (1–99; 1 with durability).
    #[data(range = "1..=99")]
    pub stack_size: Option<u8>,
    /// Uses before it breaks (0: none).
    #[data(range = "0..=1000000000")]
    pub durability: Option<u32>,
    /// Survives fire and lava.
    #[data(default)]
    pub fire_resistant: bool,
    /// Ticks it burns in a furnace.
    #[data(range = "0..=1000000000")]
    pub fuel_ticks: Option<u32>,
    pub food: Option<Food>,
    /// A tool or a weapon: its kind and material.
    pub tool: Option<ToolProps>,
    /// A piece of armor: its slot and material.
    pub armor: Option<ArmorProps>,
}

/// An item as a tool or weapon.
#[derive(Schema, Debug, Clone)]
pub struct ToolProps {
    /// `pickaxe`, `axe`, `shovel`, `hoe`, `sword`, or a mod's kind of tool
    /// by the blocks it mines (`@tag:block:mymod:mineable/hammer`).
    #[data(with = "ToolKindRef")]
    pub kind: String,
    /// The game's material (`wood`, `stone`, `iron`, `diamond`, `gold`,
    /// `netherite`) or a mod's (`@tool_material:mymod:mythril`, a `.ydef`).
    #[data(with = "MaterialRef<ToolMaterialKind>")]
    pub material: String,
    /// Attack damage on top of the material's (default: the kind's — a
    /// sword 3, an axe 6, a shovel 1.5, a pickaxe 1, a hoe 0).
    pub attack_damage: Option<f32>,
    /// Attack speed change (default: the kind's — a sword -2.4, an axe
    /// -3.1, a shovel -3, a pickaxe -2.8, a hoe -1).
    pub attack_speed: Option<f32>,
}

impl ToolProps {
    /// The kind's attack damage and speed when the file doesn't say.
    pub fn attack(&self) -> (f32, f32) {
        let (d, s) = match self.kind.as_str() {
            "sword" => (3.0, -2.4),
            "axe" => (6.0, -3.1),
            "shovel" => (1.5, -3.0),
            "hoe" => (0.0, -1.0),
            _ => (1.0, -2.8),
        };
        (self.attack_damage.unwrap_or(d), self.attack_speed.unwrap_or(s))
    }
}

/// Where a piece of armor is worn.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorSlot {
    Head,
    Chest,
    Legs,
    Feet,
}

/// An item as a piece of armor.
#[derive(Schema, Debug, Clone)]
pub struct ArmorProps {
    pub slot: ArmorSlot,
    /// The game's material (`leather`, `chainmail`, `iron`, `gold`,
    /// `diamond`, `turtle`, `netherite`) or a mod's
    /// (`@armor_material:mymod:mythril`, a `.ydef`).
    #[data(with = "MaterialRef<ArmorMaterialKind>")]
    pub material: String,
}

/// The game's kinds of tool, by name.
pub const TOOL_KINDS: [&str; 5] = ["pickaxe", "axe", "shovel", "hoe", "sword"];

/// A kind of tool: the game's by name, or a mod's mineable tag.
pub struct ToolKindRef;

impl yog_data::with::With<String> for ToolKindRef {
    fn shape() -> Shape {
        Shape::Tag(&["block"])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        match v {
            ValueRef::Str(s) if TOOL_KINDS.contains(s) => Some(s.to_string()),
            ValueRef::Str(s) if !s.starts_with('@') => {
                cx.problem(format!("`{s}`: one of {} — or a mod's kind of tool by its mineable tag, `@tag:block:mymod:mineable/hammer`", TOOL_KINDS.join(", ")));
                None
            }
            _ => yog_data::TagId::<yog_data::kinds::Block>::read(v, cx).map(|t| t.id),
        }
    }
    fn write(f: &String) -> Value {
        if TOOL_KINDS.contains(&f.as_str()) {
            Value::from(f.as_str())
        } else {
            Value::Tag(yog_data::Ref::new("block", f.clone()))
        }
    }
}

/// A material: one of the game's by short name (built-in data), or a
/// reference to a mod's (`@tool_material:…`, `@armor_material:…`).
pub struct MaterialRef<K>(std::marker::PhantomData<K>);

/// Which built-in materials a kind of material has.
pub trait MaterialKind: yog_data::Kind {
    fn built_in(name: &str) -> Option<String>;
    fn names() -> Vec<String>;
}

impl MaterialKind for ToolMaterialKind {
    fn built_in(name: &str) -> Option<String> {
        yog_registry::ToolMaterial::built_in(name).map(|m| m.id.clone())
    }
    fn names() -> Vec<String> {
        yog_registry::ToolMaterial::built_ins().iter().map(|m| m.id.trim_start_matches("minecraft:").to_string()).collect()
    }
}

impl MaterialKind for ArmorMaterialKind {
    fn built_in(name: &str) -> Option<String> {
        yog_registry::ArmorMaterial::built_in(name).map(|m| m.id.clone())
    }
    fn names() -> Vec<String> {
        yog_registry::ArmorMaterial::built_ins().iter().map(|m| m.id.trim_start_matches("minecraft:").to_string()).collect()
    }
}

impl<K: MaterialKind> yog_data::with::With<String> for MaterialRef<K> {
    fn shape() -> Shape {
        Shape::Ref(<K as yog_data::Kinds>::NAMES)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        match v {
            ValueRef::Str(s) if !s.starts_with('@') => K::built_in(s).or_else(|| {
                cx.problem(format!("`{s}`: one of the game's ({}) — or a mod's, `@{}:mymod:…` (a .ydef)", K::names().join(", "), K::NAME));
                None
            }),
            _ => yog_data::with::RefTo::<K>::read(v, cx),
        }
    }
    fn write(f: &String) -> Value {
        match f.strip_prefix("minecraft:") {
            Some(s) if K::built_in(s).is_some() => Value::from(s),
            _ => Value::Ref(yog_data::Ref::new(K::NAME, f.clone())),
        }
    }
}

/// A block tag a block joins, as its full id. Tools and tool tiers are
/// tags in Minecraft, so a mod's own are written like any other tag; the
/// game's have short names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTag(pub String);

/// `pickaxe` → `minecraft:mineable/pickaxe`, or any `@tag:block:mymod:mineable/hammer`.
pub struct ToolTag;

/// A sound group: the game's by short name (`stone`, `amethyst` =
/// `@sound_group:minecraft:amethyst`) or any `@sound_group:…` (a `.ydef`).
pub struct SoundRef;

/// A tool level: the game's by short name (`iron` = `@level:minecraft:iron`)
/// or any `@level:…`.
pub struct LevelRef;

/// The game's tools and tiers, by short name.
pub const TOOLS: [&str; 4] = ["pickaxe", "axe", "shovel", "hoe"];

fn block_tag(
    v: &ValueRef,
    cx: &mut Reader,
    short: &[&str],
    full: fn(&str) -> String,
    what: &str,
) -> Option<BlockTag> {
    match v {
        ValueRef::Str(s) if short.contains(s) => Some(BlockTag(full(s))),
        ValueRef::Str(s) if !s.starts_with('@') => {
            cx.problem(format!(
                "`{s}`: one of {} — or a tag of the mod's own {what}s, `@tag:block:mymod:…`",
                short.join(", ")
            ));
            None
        }
        _ => yog_data::TagId::<yog_data::kinds::Block>::read(v, cx).map(|t| BlockTag(t.id)),
    }
}

impl yog_data::with::With<BlockTag> for ToolTag {
    fn shape() -> Shape {
        Shape::Tag(&["block"])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<BlockTag> {
        block_tag(v, cx, &TOOLS, |s| format!("minecraft:mineable/{s}"), "tool")
    }
    fn write(f: &BlockTag) -> Value {
        match f.0.strip_prefix("minecraft:mineable/") {
            Some(s) if TOOLS.contains(&s) => Value::from(s),
            _ => Value::Tag(yog_data::Ref::new("block", f.0.clone())),
        }
    }
}

impl yog_data::with::With<String> for LevelRef {
    fn shape() -> Shape {
        Shape::Ref(&["level"])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        match v {
            ValueRef::Str(s) if TOOL_LEVELS.contains(s) => Some(format!("minecraft:{s}")),
            ValueRef::Str(s) if !s.starts_with('@') => {
                cx.problem(format!(
                    "`{s}`: one of {} — or a level of the tool ladder, `@level:mymod:…` (a .ydef)",
                    TOOL_LEVELS.join(", ")
                ));
                None
            }
            _ => yog_data::with::RefTo::<LevelKind>::read(v, cx),
        }
    }
    fn write(f: &String) -> Value {
        match f.strip_prefix("minecraft:") {
            Some(s) if TOOL_LEVELS.contains(&s) => Value::from(s),
            _ => Value::Ref(yog_data::Ref::new("level", f.clone())),
        }
    }
}

impl yog_data::with::With<String> for SoundRef {
    fn shape() -> Shape {
        Shape::Ref(&["sound_group"])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        match v {
            ValueRef::Str(s) if !s.starts_with('@') => match yog_registry::SoundGroup::built_in(s) {
                Some(g) => Some(g.id.clone()),
                None => {
                    cx.problem(format!("`{s}`: not one of the game's sound groups (stone, wood, metal, amethyst…) — a mod's is `@sound_group:mymod:…` (a .ydef)"));
                    None
                }
            },
            _ => yog_data::with::RefTo::<SoundGroupKind>::read(v, cx),
        }
    }
    fn write(f: &String) -> Value {
        match f.strip_prefix("minecraft:") {
            Some(s) if yog_registry::SoundGroup::built_in(s).is_some() => Value::from(s),
            _ => Value::Ref(yog_data::Ref::new("sound_group", f.clone())),
        }
    }
}

/// One value or a list of them, each through `W`.
pub struct OneOrMany<W>(std::marker::PhantomData<W>);

impl<F, W: yog_data::with::With<F>> yog_data::with::With<Vec<F>> for OneOrMany<W> {
    fn shape() -> Shape {
        Shape::List(W::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Vec<F>> {
        match v {
            ValueRef::List(_) => yog_data::with::List::<W>::read(v, cx),
            _ => W::read(v, cx).map(|x| vec![x]),
        }
    }
    fn write(f: &Vec<F>) -> Value {
        match f.as_slice() {
            [one] => W::write(one),
            many => Value::List(many.iter().map(W::write).collect()),
        }
    }
    fn absent() -> Option<Vec<F>> {
        Some(Vec::new())
    }
    fn skip(f: &Vec<F>) -> bool {
        f.is_empty()
    }
}

#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderLayer {
    Solid,
    Cutout,
    CutoutMipped,
    Translucent,
}

/// A schema enum's name as written.
pub fn word<T: Schema>(v: &T) -> String {
    v.write().as_str().unwrap_or_default().to_string()
}

/// The collision box: the model's bounds (`auto`), a full cube, or
/// `[x1, y1, z1, x2, y2, z2]` in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockShape {
    Auto,
    Full,
    Box([f64; 6]),
}

impl Schema for BlockShape {
    fn shape() -> Shape {
        Shape::Any
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        match v {
            ValueRef::Str(s) if *s == "auto" => Some(BlockShape::Auto),
            ValueRef::Str(s) if *s == "full" => Some(BlockShape::Full),
            ValueRef::List(_) => {
                let b = <[f64; 6]>::read(v, cx)?;
                if b.iter().any(|n| !(0.0..=16.0).contains(n)) {
                    cx.problem("a box is [x1, y1, z1, x2, y2, z2] in pixels, 0–16");
                    return None;
                }
                Some(BlockShape::Box(b))
            }
            other => {
                cx.expected(
                    "\"auto\" (the model's bounds), \"full\" or [x1, y1, z1, x2, y2, z2] in pixels",
                    other,
                );
                None
            }
        }
    }
    fn write(&self) -> Value {
        match self {
            BlockShape::Auto => Value::from("auto"),
            BlockShape::Full => Value::from("full"),
            BlockShape::Box(b) => b.write(),
        }
    }
}

/// How many drop: a number, or `[min, max]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropCount {
    Exactly(f64),
    Between(f64, f64),
}

impl Schema for DropCount {
    fn shape() -> Shape {
        Shape::Any
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let n = match v {
            ValueRef::List(_) => {
                let [lo, hi] = <[f64; 2]>::read(v, cx)?;
                (lo, hi, DropCount::Between(lo, hi))
            }
            _ => {
                let n = f64::read(v, cx)?;
                (n, n, DropCount::Exactly(n))
            }
        };
        if !(0.0..=64.0).contains(&n.0) || !(0.0..=64.0).contains(&n.1) || n.0 > n.1 {
            cx.problem("a count is 1–64, or [min, max] within 0–64");
            return None;
        }
        Some(n.2)
    }
    fn write(&self) -> Value {
        match *self {
            DropCount::Exactly(n) => Value::Float(n),
            DropCount::Between(a, b) => Value::List(vec![Value::Float(a), Value::Float(b)]),
        }
    }
}

/// A drop of an item: by default the block's own.
#[derive(Schema, Debug, Clone)]
pub struct DropItem {
    #[data(with = "Opt<RefTo<Item>>")]
    pub item: Option<String>,
    pub count: Option<DropCount>,
}

/// What breaking the block drops: itself, nothing, or an item.
#[derive(Debug, Clone)]
pub enum Drops {
    Itself,
    Nothing,
    Item(DropItem),
}

impl Schema for Drops {
    fn shape() -> Shape {
        Shape::Any
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        match v {
            ValueRef::Str(s) if *s == "self" => Some(Drops::Itself),
            ValueRef::Str(s) if *s == "none" => Some(Drops::Nothing),
            ValueRef::Map(_) => DropItem::read(v, cx).map(Drops::Item),
            other => {
                cx.expected(
                    "\"self\", \"none\" or { item: \"@item:ns:item\", count: [1, 3] }",
                    other,
                );
                None
            }
        }
    }
    fn write(&self) -> Value {
        match self {
            Drops::Itself => Value::from("self"),
            Drops::Nothing => Value::from("none"),
            Drops::Item(d) => d.write(),
        }
    }
}

/// A block's properties.
#[derive(Schema, Debug, Clone, Default)]
pub struct BlockProps {
    /// Seconds-ish to mine it (-1: unbreakable).
    #[data(range = "-1..=1000000")]
    pub hardness: Option<f64>,
    /// Against explosions.
    #[data(range = "0..=10000000")]
    pub resistance: Option<f64>,
    /// How it sounds: one of the game's sound groups (`stone`, `wood`,
    /// `metal`, `amethyst`, `copper`…) or a mod's (`@sound_group:mymod:crystal`, a `.ydef`).
    #[data(with = "Opt<SoundRef>")]
    pub sound: Option<String>,
    /// Light it gives off, 0–15.
    #[data(range = "0..=15")]
    pub light: Option<u8>,
    /// The tools that mine it fast: `pickaxe`, `axe`, `shovel`, `hoe`, or
    /// a mod's tool tag (`@tag:block:mymod:mineable/hammer`); one or a list.
    #[data(with = "OneOrMany<ToolTag>")]
    pub tool: Vec<BlockTag>,
    /// The tool level it needs: `stone`, `iron`, `diamond`, `netherite`, or
    /// any level of the tool ladder (`@level:mymod:mythril`, a `.ydef`).
    #[data(with = "Opt<LevelRef>")]
    pub tool_level: Option<String>,
    /// Drops nothing without the tool.
    #[data(default)]
    pub requires_tool: bool,
    /// Entities bump into it (`false`: walk through).
    pub collision: Option<bool>,
    #[data(range = "0..=1")]
    pub slipperiness: Option<f64>,
    pub shape: Option<BlockShape>,
    pub render_layer: Option<RenderLayer>,
    pub drops: Option<Drops>,
    /// Blocks it connects to (`states.connect`): these groups' members.
    #[data(default)]
    pub connect_groups: Vec<String>,
    /// The inventory screen it opens: a `.yui` `<Screen id>` with `<Slot>`s.
    pub inventory: Option<String>,
    /// Gives off redstone power, set by the mod per position.
    #[data(default)]
    pub redstone: bool,
    /// A yes/no property (`states.properties`) the block keeps on while it
    /// gets redstone power.
    pub powered: Option<String>,
    /// A yes/no property a comparator reads: 15 while it's on.
    pub comparator: Option<String>,
    /// Hoppers and droppers can put items into it; the mod takes what it
    /// wants of them (`on_item_insert`).
    #[data(default)]
    pub accepts_items: bool,
}

/// A texture: `#slot`, `@Set.image` (a `.yres` image), a `.png` path in
/// `resources/`, or a texture id (`minecraft:block/stone`).
#[derive(Debug, Clone, PartialEq)]
pub enum Texture {
    Slot(String),
    Res(Res),
    /// A `.png` path or a texture id, as written.
    Plain(String),
}

impl Schema for Texture {
    fn shape() -> Shape {
        Shape::Image
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        match v {
            ValueRef::Res { .. } => Res::read(v, cx).map(Texture::Res),
            ValueRef::Str(s) if s.starts_with('#') => {
                if s.len() == 1 {
                    cx.problem("`#` needs a slot name");
                    return None;
                }
                Some(Texture::Slot(s[1..].to_string()))
            }
            ValueRef::Str(s) if s.starts_with('@') => Res::read(v, cx).map(Texture::Res),
            ValueRef::Str(s) => Some(Texture::Plain(s.to_string())),
            other => {
                cx.expected("a texture", other);
                None
            }
        }
    }
    fn write(&self) -> Value {
        match self {
            Texture::Slot(s) => Value::Str(format!("#{s}")),
            Texture::Res(r) => Value::Res(r.clone()),
            Texture::Plain(s) => Value::Str(s.clone()),
        }
    }
}

#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiLight {
    Front,
    Side,
}

/// An element's rotation about one axis.
#[derive(Schema, Debug, Clone)]
pub struct ElementRotation {
    /// The point it turns about; the block's middle by default.
    #[data(range = "-16..=32")]
    pub origin: Option<[f64; 3]>,
    pub axis: Axis,
    /// -45, -22.5, 0, 22.5 or 45 degrees.
    #[data(range = "-45..=45")]
    pub angle: f64,
    #[data(default)]
    pub rescale: bool,
}

/// One face of an element.
#[derive(Schema, Debug, Clone)]
pub struct Face {
    /// `#slot` — a slot of the model's textures.
    pub texture: String,
    #[data(range = "0..=16")]
    pub uv: Option<[f64; 4]>,
    /// 0, 90, 180 or 270.
    pub rotation: Option<u32>,
    /// Not drawn when this side touches a full block.
    pub cullface: Option<Direction>,
    /// The tint index (-1: none).
    #[data(range = "-1..=255")]
    pub tint: Option<i64>,
}

#[derive(Schema, Debug, Clone, Default)]
pub struct Faces {
    pub down: Option<Face>,
    pub up: Option<Face>,
    pub north: Option<Face>,
    pub south: Option<Face>,
    pub west: Option<Face>,
    pub east: Option<Face>,
}

impl Faces {
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Face)> {
        [
            ("down", &self.down),
            ("up", &self.up),
            ("north", &self.north),
            ("south", &self.south),
            ("west", &self.west),
            ("east", &self.east),
        ]
        .into_iter()
        .filter_map(|(d, f)| f.as_ref().map(|f| (d, f)))
    }
}

/// A box of a model.
#[derive(Schema, Debug, Clone)]
pub struct Element {
    pub name: Option<String>,
    #[data(range = "-16..=32")]
    pub from: [f64; 3],
    #[data(range = "-16..=32")]
    pub to: [f64; 3],
    pub rotation: Option<ElementRotation>,
    pub shade: Option<bool>,
    pub faces: Option<Faces>,
}

/// How an item model sits in one place.
#[derive(Schema, Debug, Clone)]
pub struct Transform {
    #[data(range = "-360..=360")]
    pub rotation: Option<[f64; 3]>,
    #[data(range = "-80..=80")]
    pub translation: Option<[f64; 3]>,
    #[data(range = "-4..=4")]
    pub scale: Option<[f64; 3]>,
}

#[derive(Schema, Debug, Clone, Default)]
pub struct Display {
    pub thirdperson_righthand: Option<Transform>,
    pub thirdperson_lefthand: Option<Transform>,
    pub firstperson_righthand: Option<Transform>,
    pub firstperson_lefthand: Option<Transform>,
    pub gui: Option<Transform>,
    pub head: Option<Transform>,
    pub ground: Option<Transform>,
    pub fixed: Option<Transform>,
}

impl Display {
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Transform)> {
        [
            ("thirdperson_righthand", &self.thirdperson_righthand),
            ("thirdperson_lefthand", &self.thirdperson_lefthand),
            ("firstperson_righthand", &self.firstperson_righthand),
            ("firstperson_lefthand", &self.firstperson_lefthand),
            ("gui", &self.gui),
            ("head", &self.head),
            ("ground", &self.ground),
            ("fixed", &self.fixed),
        ]
        .into_iter()
        .filter_map(|(d, t)| t.as_ref().map(|t| (d, t)))
    }
}

/// A model: a template (`cube_all`, `generated`, …) or a model id as
/// `parent`, its textures, or its own elements.
#[derive(Schema, Debug, Clone)]
pub struct Model {
    pub parent: Option<String>,
    /// Slot → texture.
    #[data(default)]
    pub textures: Ordered<Texture>,
    pub elements: Option<Vec<Element>>,
    pub display: Option<Display>,
    pub ambient_occlusion: Option<bool>,
    pub gui_light: Option<GuiLight>,
}

/// One way the block stands.
#[derive(Schema, Debug, Clone)]
pub struct Rotation {
    /// 0, 90, 180 or 270.
    pub x: Option<u32>,
    /// 0, 90, 180 or 270.
    pub y: Option<u32>,
    #[data(range = "1..=1000000")]
    pub weight: Option<f64>,
    #[data(default)]
    pub uvlock: bool,
}

/// Arms towards neighbours that connect (see `block.connect_groups`).
#[derive(Schema, Debug, Clone)]
pub struct Connect {
    /// The model of the arm towards north.
    pub arm: Model,
    #[data(default)]
    pub uvlock: bool,
}

#[derive(Schema, Debug, Clone)]
pub struct States {
    pub rotations: Option<Vec<Rotation>>,
    pub connect: Option<Connect>,
    /// The block turns when placed (a `facing` property); its model's north
    /// side is its front.
    pub facing: Option<Facing>,
    /// Which way it turns when placed: its front toward the player (as a
    /// furnace, the default) or the way they look (as an observer).
    pub placement: Option<Placement>,
    /// It hangs on a floor, wall or ceiling, as a button does: a `face`
    /// property (`floor`, `wall`, `ceiling`) and a horizontal `facing`. Its
    /// model lies on the floor.
    #[data(default)]
    pub attach: bool,
    /// It can stand in water (a `waterlogged` property).
    #[data(default)]
    pub waterlogged: bool,
    /// Its textures stay as they are in the world when it turns (a
    /// blockstate's `uvlock`).
    #[data(default)]
    pub uvlock: bool,
    /// Properties of its own: a name and its values, the first the
    /// default; `["false", "true"]` is a yes/no.
    #[data(default)]
    pub properties: Ordered<Vec<String>>,
    /// More models, by name, for `variants`.
    #[data(default)]
    pub models: Ordered<Model>,
    /// The model a state shows: some of its properties
    /// (`"energized=true,facing=up"`) → a name in `models`. The first that
    /// matches wins, else the block's own model; turning for `facing` and
    /// `face` is added.
    #[data(default)]
    pub variants: Ordered<String>,
}

/// Which ways a block can turn.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    /// North, east, south or west.
    Horizontal,
    /// Any of the six.
    All,
}

/// Which way a block turns when placed.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Its front toward the player.
    Toward,
    /// Its front the way the player looks.
    Away,
}

/// A `.yitem`.
#[derive(Schema, Debug, Clone)]
pub struct ItemFile {
    /// The file format; 1.
    pub format: Option<u32>,
    /// Its id; the file's name in the mod's namespace by default.
    pub id: Option<String>,
    pub name: Option<Text>,
    pub tooltip: Option<Text>,
    pub register: Option<Register>,
    /// Mods it needs — Yog or Java mod ids (`pehkui`): without every one of
    /// them loaded it isn't there (an optional integration).
    #[data(default)]
    pub requires: Vec<String>,
    pub model: Model,
    pub item: Option<ItemProps>,
}

/// A `.yblock` — the block and its item form.
#[derive(Schema, Debug, Clone)]
pub struct BlockFile {
    pub format: Option<u32>,
    pub id: Option<String>,
    pub name: Option<Text>,
    pub tooltip: Option<Text>,
    pub register: Option<Register>,
    /// Mods it needs — Yog or Java mod ids (`pehkui`): without every one of
    /// them loaded it isn't there (an optional integration).
    #[data(default)]
    pub requires: Vec<String>,
    pub model: Model,
    /// The item form's model; the block's by default.
    pub item_model: Option<Model>,
    pub block: Option<BlockProps>,
    pub states: Option<States>,
}
