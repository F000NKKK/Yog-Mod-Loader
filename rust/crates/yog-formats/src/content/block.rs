//! A `.yblock`'s properties: shape, drops, sounds, tools, redstone.

use yog_data::kinds::Item;
use yog_data::with::{Opt, RefTo};
use yog_data::{Reader, Schema, Shape, Value, ValueRef};
use yog_ids::units::{BLOCK_PIXELS, MAX_STACK};

use super::refs::{BlockTag, LevelRef, OneOrMany, SoundRef, ToolTag};

#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderLayer {
    Solid,
    Cutout,
    CutoutMipped,
    Translucent,
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
                if b.iter()
                    .any(|n| !(0.0..=f64::from(BLOCK_PIXELS)).contains(n))
                {
                    cx.problem("a box is [x1, y1, z1, x2, y2, z2] in pixels, 0–16");
                    return None;
                }
                Some(BlockShape::Box(b))
            }
            other => {
                yog_data::expected(cx,
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
        let stack = 0.0..=f64::from(MAX_STACK);
        if !stack.contains(&n.0) || !stack.contains(&n.1) || n.0 > n.1 {
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
                yog_data::expected(cx,
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

/// Who sets off a button or a pressure plate.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TriggeredBy {
    /// Any entity — items and arrows too (a wooden one).
    #[default]
    Everything,
    /// Living things only (a stone one).
    Mobs,
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
    /// It hides the faces of blocks behind it and stops light, as a full
    /// block does (`false`: as glass; a door's default).
    pub occludes: Option<bool>,
    /// The block a `kind` is made from: stairs act as it, a double slab
    /// shows its model.
    #[data(with = "Opt<RefTo<yog_data::kinds::Block>>")]
    pub base: Option<String>,
    /// A `kind`'s set — the game's `oak`, `dark_oak`, `iron`, `stone`… (any
    /// registered block set; a fence gate's wood type): its sounds and
    /// whether a hand opens it. `oak` by default.
    pub set: Option<String>,
    /// How long a button stays pressed, ticks (30, as a wooden one).
    #[data(range = "1..=1000000")]
    pub press_ticks: Option<u32>,
    /// Who sets off a button or a pressure plate.
    pub triggered_by: Option<TriggeredBy>,
}
