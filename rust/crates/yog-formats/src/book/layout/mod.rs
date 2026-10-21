//! Recipe layouts: how a kind of recipe is shown — in books and in
//! Yog-IDLE's recipe editor. The game's kinds have built-in layouts
//! (Patchouli's: the crafting grid, the furnace strip, the smithing
//! strip, …); a mod lays out its own kinds, or the game's, with a `.yui`
//! component — drawn and arranged in the screen designer like any UI:
//!
//! ```xml
//! <Component name="InfusionLayout" recipe_layout="infusion" width="96" height="44">
//!     <Image source="mymod:gui/infusion" uv="0 0 96 44"/>
//!     <RecipeSlot slot="ring" x="4" y="4" frame="true"/>
//!     <RecipeSlot slot="ring" x="4" y="24" frame="true"/>
//!     <RecipeSlot slot="core" x="40" y="14" frame="true"/>
//!     <ItemSlot item="minecraft:beacon" x="40" y="0"/>
//!     <RecipeResult x="76" y="14"/>
//!     <RecipeCount x="78" y="32"/>
//!     <Label text="{x:Bind time} ticks" x="40" y="34"/>
//! </Component>
//! ```
//!
//! `recipe_layout` names the kinds it shows (several: space-separated):
//! the mod's own (`infusion`, its namespace added) or the game's
//! (`shaped`, `smelting`, …). Positions are GUI pixels from the layout's
//! top-left; an item is 16×16. A slot's places take its ingredients in
//! order (`index` to say which). The game's kinds name their slots:
//! `grid` (crafting, 9 places; a shaped recipe's cells by position),
//! `input` (cooking, stonecutting), `template`/`base`/`addition`
//! (smithing), `from`/`input` (brewing). A label's bindings are the recipe's values (`time`,
//! `experience`, or a mod kind's `values`).

use yog_sync::LazyLock;

use serde::{Deserialize, Serialize};
use yog_data::Schema;
use yog_data::kinds::Item;
use yog_data::with::RefTo;

mod from_yui;

pub use from_yui::{compile, is_recipe_layout};

/// How one or more kinds of recipe are shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct RecipeLayout {
    /// The recipe kinds it shows: the game's (`shaped`, `smelting`, …) or a mod's (`mymod:infusion`).
    pub kinds: Vec<String>,
    /// Width and height in GUI pixels.
    pub size: [f32; 2],
    /// What's drawn, in order.
    #[serde(default)]
    #[data(default)]
    pub parts: Vec<LayoutPart>,
}

/// One part of a [`RecipeLayout`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[data(tag = "type")]
pub enum LayoutPart {
    /// A region of an image (a Yog image source).
    Image {
        source: String,
        /// `[u, v, width, height]` in the image's pixels.
        uv: [f32; 4],
        #[serde(default)]
        #[data(default)]
        at: [f32; 2],
    },
    /// Where a slot's ingredients go, in order.
    Slots {
        slot: String,
        at: Vec<[f32; 2]>,
        /// A slot frame behind each (for layouts without their own picture).
        #[serde(default)]
        #[data(default)]
        frame: bool,
    },
    /// The recipe's result.
    Result {
        at: [f32; 2],
        #[serde(default)]
        #[data(default)]
        frame: bool,
    },
    /// The result's count, when more than one.
    Count { at: [f32; 2] },
    /// An item drawn as it is: the station (a furnace, the crafting table).
    Item {
        #[data(with = "RefTo<Item>")]
        item: String,
        at: [f32; 2],
    },
    /// A block in 3D, at a slot's angle (a station that's a block: the
    /// brewing stand with its bottles): a full block state
    /// (`minecraft:brewing_stand[has_bottle_0=true]`).
    Block {
        block: String,
        at: [f32; 2],
        /// Its side in GUI pixels: a slot's item by default.
        #[serde(default = "icon_px")]
        #[data(default = "icon_px")]
        size: f32,
    },
    /// Text; `{name}` is the recipe's value of that name.
    Text { text: String, at: [f32; 2] },
}

/// The game's kinds' layouts.
pub static BUILT_IN: LazyLock<Vec<RecipeLayout>> = LazyLock::new(|| {
    yog_data::built_in("yog-formats' built-in recipe layouts", include_str!("../../../assets/layouts.ydata"))
});

/// The layout for recipes of `kind`: one of `own` (a book's), else built in.
pub fn find<'a>(own: &'a [RecipeLayout], kind: &str) -> Option<&'a RecipeLayout> {
    own.iter()
        .chain(BUILT_IN.iter())
        .find(|l| l.kinds.iter().any(|k| k == kind))
}

/// The game's kinds of recipe (those with built-in layouts).
pub fn game_kinds() -> impl Iterator<Item = &'static str> {
    BUILT_IN
        .iter()
        .flat_map(|l| l.kinds.iter().map(String::as_str))
}

/// An item's side, in GUI pixels.
pub const ICON_PX: f32 = yog_ids::units::ITEM_PIXELS as f32;

fn icon_px() -> f32 {
    ICON_PX
}
