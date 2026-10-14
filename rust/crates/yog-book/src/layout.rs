//! Recipe layouts: how a book shows a kind of recipe — data, not code.
//! The game's kinds have built-in layouts (Patchouli's: the crafting grid,
//! the furnace strip, the smithing strip, …); a mod lays out its own kinds
//! (`mymod:infusion`) the same way, in `content/*.ylayout`:
//!
//! ```json5
//! {
//!   kinds: ["mymod:infusion"],
//!   size: [96, 44],
//!   parts: [
//!     { type: "image", source: "mymod:gui/infusion", uv: [0, 0, 96, 44] },
//!     { type: "slots", slot: "ring", at: [[4, 4], [4, 24]], frame: true },
//!     { type: "slots", slot: "core", at: [[40, 14]], frame: true },
//!     { type: "result", at: [76, 14] },
//!     { type: "text", text: "{time} ticks", at: [40, 34] },
//!   ],
//! }
//! ```
//!
//! Positions are GUI pixels from the layout's top-left; an item is 16×16.
//! A slot's ingredients fill its places in order. The game's kinds name
//! their slots: `grid` (crafting, 9 places; a shaped recipe's cells by
//! position), `input` (cooking, stonecutting), `template`/`base`/`addition`
//! (smithing). A text's `{name}` is the recipe's value of that name
//! (`time`, `experience`, or a mod kind's `values`).

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use yog_data::Schema;
use yog_data::kinds::Item;
use yog_data::with::RefTo;

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
    /// Text; `{name}` is the recipe's value of that name.
    Text { text: String, at: [f32; 2] },
}

/// The image the built-in layouts are drawn from (Patchouli's crafting sheet).
pub const SHEET: &str = crate::renderer::CRAFTING_SHEET;

/// A slot frame on [`SHEET`], 24×24, the item inside at +4,+4.
pub const FRAME_UV: [f32; 4] = [83.0, 71.0, 24.0, 24.0];

/// The game's kinds' layouts.
pub static BUILT_IN: LazyLock<Vec<RecipeLayout>> = LazyLock::new(|| {
    yog_data::from_text::<Vec<RecipeLayout>>(include_str!("../assets/layouts.ydata"))
        .unwrap_or_else(|e| panic!("yog-book's built-in layouts: {e}"))
        .value
});

/// The layout for recipes of `kind`: one of `own` (a book's), else built in.
pub fn find<'a>(own: &'a [RecipeLayout], kind: &str) -> Option<&'a RecipeLayout> {
    own.iter()
        .chain(BUILT_IN.iter())
        .find(|l| l.kinds.iter().any(|k| k == kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_game_kind_has_a_built_in_layout() {
        for kind in [
            "shaped",
            "shapeless",
            "smelting",
            "blasting",
            "smoking",
            "campfire_cooking",
            "stonecutting",
            "smithing",
        ] {
            assert!(find(&[], kind).is_some(), "{kind}");
        }
    }

    #[test]
    fn a_mod_layout_reads_and_wins() {
        let text = r#"{ kinds: ["shaped", "mymod:infusion"], size: [96, 44], parts: [
            { type: "slots", slot: "core", at: [[40, 14]], frame: true },
            { type: "item", item: "@item:minecraft:beacon", at: [0, 0] },
        ] }"#;
        let l = yog_data::from_text::<RecipeLayout>(text).unwrap().value;
        assert!(matches!(&l.parts[1], LayoutPart::Item { item, .. } if item == "minecraft:beacon"));
        let own = [l];
        assert_eq!(find(&own, "shaped").unwrap().size, [96.0, 44.0]);
        assert_eq!(find(&own, "mymod:infusion").unwrap().kinds.len(), 2);
        let e = yog_data::from_text::<RecipeLayout>(
            r#"{ kinds: [], size: [1, 1], parts: [{ type: "slot", at: [] }] }"#,
        )
        .unwrap_err();
        assert!(
            e.to_string()
                .contains("unknown `slot` — did you mean `slots`?"),
            "{e}"
        );
    }
}
