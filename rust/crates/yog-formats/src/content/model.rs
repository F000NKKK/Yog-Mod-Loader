//! Models: textures, elements, display transforms, and a block's states.

use yog_data::{Ordered, Reader, Schema, Shape, Value, ValueRef};

/// A texture: a slot of the model (`#side`), or a texture by id —
/// `@texture:mymod:block/ruby` (the mod's `.ytexture`) or the game's
/// (`@texture:minecraft:block/stone`).
#[derive(Debug, Clone, PartialEq)]
pub enum Texture {
    Slot(String),
    /// A texture's id, its namespace given (the game's if none).
    Ref(String),
}

/// What a texture slot starts with: `#side`.
pub const SLOT_MARK: char = '#';

impl Schema for Texture {
    fn shape() -> Shape {
        yog_data::schema::ref_shape::<yog_data::kinds::Texture>()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        if let ValueRef::Str(s) = v
            && let Some(slot) = s.strip_prefix(SLOT_MARK)
        {
            if slot.is_empty() {
                cx.problem("`#` needs a slot name");
                return None;
            }
            return Some(Texture::Slot(slot.to_string()));
        }
        let id = yog_data::Id::<yog_data::kinds::Texture>::read(v, cx)?;
        Some(Texture::Ref(yog_ids::qualify(&id.id)))
    }
    fn write(&self) -> Value {
        match self {
            Texture::Slot(s) => Value::Str(format!("{SLOT_MARK}{s}")),
            Texture::Ref(id) => yog_data::Id::<yog_data::kinds::Texture>::new(id.clone()).write(),
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
    pub cullface: Option<yog_core::Face>,
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
    /// A `.ymaterial` (`@material:mymod:block/ruby`): its textures, under the
    /// model's own, and — a block's — its render layer when the block gives none.
    #[data(with = "yog_data::with::Opt<yog_data::with::RefTo<yog_data::kinds::Material>>")]
    pub material: Option<String>,
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
    /// (`"energized=true,facing=up"`) → a name in `models`, or a list of
    /// names to pick from at random per position. The first that matches
    /// wins, else the block's own model; turning for `facing` and `face` is
    /// added.
    #[data(default)]
    pub variants: Ordered<ModelNames>,
}

/// One model name, or several to pick from at random (a blockstate's list of variants).
pub type ModelNames = yog_data::OneOrList<String>;

pub use yog_registry::{Facing, Placement};
