//! A recipe layout on an inventory screen (`<RecipeArea>`): its pictures
//! and stations as `.yui` elements, its places and result as real slots.

use yog_formats::book::layout::{ICON_PX, LayoutPart, RecipeLayout};
use yog_gfx::image::ImageSource;

/// One element of a layout on an inventory screen (`<RecipeArea>`): a
/// `.yui` element's name and attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenElement {
    pub kind: &'static str,
    pub attrs: Vec<(&'static str, String)>,
}

/// A layout as an inventory screen shows it: its pictures and stations,
/// its places and result as real slots numbered from `first`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScreenArea {
    pub elements: Vec<ScreenElement>,
    /// Each place: its slot, its index in the slot, its screen slot.
    pub places: Vec<(String, usize, usize)>,
    pub result: Option<usize>,
    /// The next free screen slot.
    pub next: usize,
}

/// A slot is drawn this far up and left of its item.
const SLOT_BORDER: f32 = 1.0;
/// Text a layout shows as it is (no `{value}`), in the game's dark grey.
const LABEL_COLOR: &str = "#FF404040";

/// `layout`'s parts as screen elements, its places and result slots `first`
/// onward. A text bound to a recipe's values has nothing to show on a
/// screen; a block shows as its item does (a screen's images aren't 3D).
pub fn screen_area(layout: &RecipeLayout, first: usize) -> ScreenArea {
    let mut out = ScreenArea { next: first, ..ScreenArea::default() };
    let xy = |p: [f32; 2], d: f32| [("x", (p[0] - d).to_string()), ("y", (p[1] - d).to_string())];
    let icon = |source: String, at: [f32; 2]| {
        let [x, y] = xy(at, 0.0);
        ScreenElement { kind: "Image", attrs: vec![("source", source), ("w", ICON_PX.to_string()), ("h", ICON_PX.to_string()), x, y] }
    };
    for part in &layout.parts {
        match part {
            LayoutPart::Image { source, uv, at } => {
                let [u, v, w, h] = *uv;
                let [x, y] = xy(*at, 0.0);
                let mut attrs = vec![("source", source.clone()), ("w", w.to_string()), ("h", h.to_string()), ("stretch", "Fill".into()), x, y];
                // A sprite is whole; an image's region is its uv.
                if !matches!(ImageSource::parse(source), Some(ImageSource::Sprite(_))) {
                    attrs.push(("uv", format!("{u} {v} {w} {h}")));
                }
                out.elements.push(ScreenElement { kind: "Image", attrs });
            }
            LayoutPart::Slots { slot, at, .. } => {
                for (i, p) in at.iter().enumerate() {
                    let [x, y] = xy(*p, SLOT_BORDER);
                    out.elements.push(ScreenElement { kind: "Slot", attrs: vec![("index", out.next.to_string()), x, y] });
                    out.places.push((slot.clone(), i, out.next));
                    out.next += 1;
                }
            }
            LayoutPart::Result { at, .. } => {
                let [x, y] = xy(*at, SLOT_BORDER);
                out.elements.push(ScreenElement { kind: "Slot", attrs: vec![("index", out.next.to_string()), x, y] });
                out.result = Some(out.next);
                out.next += 1;
            }
            LayoutPart::Item { item, at } => out.elements.push(icon(ImageSource::Item(item.clone()).to_string(), *at)),
            LayoutPart::Block { block, at, .. } => {
                let id = block.split('[').next().unwrap_or(block);
                out.elements.push(icon(ImageSource::Block(id.to_string()).to_string(), *at));
            }
            LayoutPart::Text { text, at } if !text.contains('{') => {
                let [x, y] = xy(*at, 0.0);
                out.elements.push(ScreenElement { kind: "Label", attrs: vec![("text", text.clone()), ("color", LABEL_COLOR.into()), ("shadow", "false".into()), x, y] });
            }
            LayoutPart::Text { .. } | LayoutPart::Count { .. } => {}
        }
    }
    out
}
