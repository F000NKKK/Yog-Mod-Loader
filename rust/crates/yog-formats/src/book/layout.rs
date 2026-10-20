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
    yog_data::from_text::<Vec<RecipeLayout>>(include_str!("../../assets/layouts.ydata"))
        .unwrap_or_else(|e| panic!("yog-formats' built-in recipe layouts: {e}"))
        .value
});

/// The layout for recipes of `kind`: one of `own` (a book's), else built in.
pub fn find<'a>(own: &'a [RecipeLayout], kind: &str) -> Option<&'a RecipeLayout> {
    own.iter()
        .chain(BUILT_IN.iter())
        .find(|l| l.kinds.iter().any(|k| k == kind))
}

// ── From `.yui` ────────────────────────────────────────────────────────────────

use crate::yui::Element;

/// Whether `root` is a recipe layout (`<Component recipe_layout="…">`).
pub fn is_recipe_layout(root: &Element) -> bool {
    root.name == yog_ids::yui::COMPONENT && root.attr(yog_ids::yui_attr::RECIPE_LAYOUT).is_some()
}

fn err<T>(el: &Element, msg: impl std::fmt::Display) -> Result<T, String> {
    Err(format!("line {}: <{}>: {msg}", el.line, el.name))
}

fn num(el: &Element, key: &str, default: Option<f32>) -> Result<f32, String> {
    match el.attr(key) {
        Some(v) => v
            .trim()
            .parse::<f32>()
            .or_else(|_| err(el, format_args!("{key}=\"{v}\": a number of GUI pixels"))),
        None => default
            .map(Ok)
            .unwrap_or_else(|| err(el, format_args!("needs {key}=\"…\""))),
    }
}

fn at(el: &Element) -> Result<[f32; 2], String> {
    Ok([num(el, "x", Some(0.0))?, num(el, "y", Some(0.0))?])
}

fn flag(el: &Element, key: &str) -> bool {
    el.attr(key).is_some_and(|v| v.trim() == "true")
}

/// The game's kinds of recipe (those with built-in layouts).
pub fn game_kinds() -> impl Iterator<Item = &'static str> {
    BUILT_IN
        .iter()
        .flat_map(|l| l.kinds.iter().map(String::as_str))
}

/// `root` (a `<Component recipe_layout="…">`) as a layout; `ns`: the mod's
/// namespace, for its own kinds written without one.
pub fn compile(root: &Element, ns: &str) -> Result<RecipeLayout, String> {
    let kinds: Vec<String> = root
        .attr(yog_ids::yui_attr::RECIPE_LAYOUT)
        .unwrap_or_default()
        .split([' ', ','])
        .filter(|k| !k.is_empty())
        .map(|k| {
            if k.contains(':') || game_kinds().any(|g| g == k) {
                k.to_string()
            } else {
                format!("{ns}:{k}")
            }
        })
        .collect();
    if kinds.is_empty() {
        return err(
            root,
            "recipe_layout=\"…\": the kinds it shows — the mod's own or the game's",
        );
    }
    let size = [num(root, "width", None)?, num(root, "height", None)?];
    // The elements may sit in one <Canvas>, as a screen's do.
    let mut children: Vec<&Element> = Vec::new();
    for c in &root.children {
        if c.name == yog_ids::yui::CANVAS {
            children.extend(c.children.iter())
        } else {
            children.push(c)
        }
    }
    let mut parts: Vec<LayoutPart> = Vec::new();
    // A slot's places (index, where), gathered into its one part, and its frame.
    type Places = Vec<(usize, [f32; 2])>;
    let mut places: Vec<(String, Places, bool)> = Vec::new();
    for el in children {
        match el.name.as_str() {
            yog_ids::yui::IMAGE => {
                let source = el
                    .attr(yog_ids::yui_attr::SOURCE)
                    .map(Ok)
                    .unwrap_or_else(|| err(el, "needs source=\"…\""))?
                    .to_string();
                let uv = match el.attr(yog_ids::yui_attr::UV) {
                    Some(v) => {
                        let n: Vec<f32> = v
                            .split_whitespace()
                            .filter_map(|x| x.parse().ok())
                            .collect();
                        let [u, v, w, h] = n[..] else {
                            return err(el, format_args!("uv=\"{v}\": u v width height"));
                        };
                        [u, v, w, h]
                    }
                    None => [0.0, 0.0, num(el, "w", None)?, num(el, "h", None)?],
                };
                parts.push(LayoutPart::Image {
                    source,
                    uv,
                    at: at(el)?,
                });
            }
            yog_ids::yui::RECIPE_SLOT => {
                let slot = el
                    .attr(yog_ids::yui_attr::SLOT)
                    .map(Ok)
                    .unwrap_or_else(|| err(el, "needs slot=\"…\" (the kind's slot)"))?
                    .to_string();
                let index = match el.attr(yog_ids::yui_attr::INDEX) {
                    Some(i) => i
                        .trim()
                        .parse::<usize>()
                        .or_else(|_| err(el, format_args!("index=\"{i}\": 0, 1, …")))?,
                    None => places
                        .iter()
                        .find(|(s, ..)| *s == slot)
                        .map_or(0, |(_, p, _)| {
                            p.iter().map(|(i, _)| i + 1).max().unwrap_or(0)
                        }),
                };
                match places.iter_mut().find(|(s, ..)| *s == slot) {
                    Some((_, p, frame)) => {
                        if p.iter().any(|(i, _)| *i == index) {
                            return err(
                                el,
                                format_args!("{slot}'s place {index} is laid out twice"),
                            );
                        }
                        p.push((index, at(el)?));
                        *frame |= flag(el, "frame");
                    }
                    None => {
                        places.push((slot.clone(), vec![(index, at(el)?)], flag(el, "frame")));
                        parts.push(LayoutPart::Slots {
                            slot,
                            at: Vec::new(),
                            frame: false,
                        });
                    }
                }
            }
            yog_ids::yui::RECIPE_RESULT => parts.push(LayoutPart::Result {
                at: at(el)?,
                frame: flag(el, "frame"),
            }),
            yog_ids::yui::RECIPE_COUNT => parts.push(LayoutPart::Count { at: at(el)? }),
            yog_ids::yui::ITEM_SLOT => {
                let item = el
                    .attr(yog_ids::yui_attr::ITEM)
                    .map(Ok)
                    .unwrap_or_else(|| {
                        err(el, "needs item=\"…\" (the station: minecraft:furnace)")
                    })?;
                let item =
                    yog_ids::qualify(item.trim().trim_start_matches(yog_ids::reference::ITEM));
                parts.push(LayoutPart::Item { item, at: at(el)? });
            }
            "BlockSlot" => {
                let block = el.attr(yog_ids::yui_attr::BLOCK).map(Ok).unwrap_or_else(|| err(el, "needs block=\"…\" (a block state: minecraft:brewing_stand[has_bottle_0=true])"))?;
                let block =
                    yog_ids::qualify(block.trim().trim_start_matches(yog_ids::reference::BLOCK));
                let size = el
                    .attr("size")
                    .map(|s| {
                        s.trim()
                            .parse::<f32>()
                            .map_err(|_| format!("size=\"{s}\": a number of pixels"))
                    })
                    .transpose()
                    .or_else(|e| err(el, e))?
                    .unwrap_or(ICON_PX);
                parts.push(LayoutPart::Block {
                    block,
                    at: at(el)?,
                    size,
                });
            }
            yog_ids::yui::LABEL => {
                let text = el
                    .attr(yog_ids::yui_attr::TEXT)
                    .map(Ok)
                    .unwrap_or_else(|| err(el, "needs text=\"…\""))?;
                parts.push(LayoutPart::Text {
                    text: text.replace("{x:Bind ", "{"),
                    at: at(el)?,
                });
            }
            other => {
                return err(
                    el,
                    format_args!(
                        "not a recipe layout's element (`{other}`) — Image, RecipeSlot, RecipeResult, RecipeCount, ItemSlot, BlockSlot, Label"
                    ),
                );
            }
        }
    }
    for part in &mut parts {
        if let LayoutPart::Slots { slot, at, frame } = part
            && let Some((_, p, f)) = places.iter().find(|(s, ..)| s == slot)
        {
            let mut p = p.clone();
            p.sort_by_key(|(i, _)| *i);
            if let Some(gap) = p.iter().enumerate().find(|(n, (i, _))| n != i) {
                return Err(format!(
                    "{slot}: place {} isn't laid out (a slot's places are 0, 1, … in a row)",
                    gap.0
                ));
            }
            *at = p.into_iter().map(|(_, a)| a).collect();
            *frame = *f;
        }
    }
    Ok(RecipeLayout { kinds, size, parts })
}

/// An item's side, in GUI pixels.
pub const ICON_PX: f32 = 16.0;

fn icon_px() -> f32 {
    ICON_PX
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
            "brewing",
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

    #[test]
    fn a_layout_compiles_from_yui() {
        let el = crate::yui::parse(
            r#"<Component name="Infusion" recipe_layout="infusion shaped" width="96" height="44">
                <Canvas>
                    <Image source="mymod:gui/infusion" uv="0 0 96 44"/>
                    <RecipeSlot slot="ring" x="4" y="24" index="1" frame="true"/>
                    <RecipeSlot slot="ring" x="4" y="4" index="0"/>
                    <ItemSlot item="beacon" x="40" y="0"/>
                    <RecipeResult x="76" y="14"/>
                    <Label text="{x:Bind time} ticks" x="40" y="34"/>
                </Canvas>
            </Component>"#,
        )
        .unwrap();
        assert!(is_recipe_layout(&el));
        let l = compile(&el, "mymod").unwrap();
        assert_eq!(l.kinds, ["mymod:infusion", "shaped"]);
        assert_eq!(l.size, [96.0, 44.0]);
        assert!(
            matches!(&l.parts[1], LayoutPart::Slots { slot, at, frame: true } if slot == "ring" && at == &[[4.0, 4.0], [4.0, 24.0]])
        );
        assert!(matches!(&l.parts[2], LayoutPart::Item { item, .. } if item == "minecraft:beacon"));
        assert!(matches!(&l.parts[4], LayoutPart::Text { text, .. } if text == "{time} ticks"));

        let bad = crate::yui::parse(
            r#"<Component recipe_layout="x" width="1" height="1"><Button/></Component>"#,
        )
        .unwrap();
        assert!(
            compile(&bad, "m")
                .unwrap_err()
                .contains("not a recipe layout's element (`Button`)")
        );
        let gap = crate::yui::parse(r#"<Component recipe_layout="x" width="1" height="1"><RecipeSlot slot="a" index="1"/></Component>"#).unwrap();
        assert!(
            compile(&gap, "m")
                .unwrap_err()
                .contains("place 0 isn't laid out")
        );
    }
}
