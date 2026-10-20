//! A recipe layout on an inventory screen (`<RecipeArea>`): its pictures
//! and stations as `.yui` elements, its places and result as real slots.

use yog_formats::book::layout::{ICON_PX, LayoutPart, RecipeLayout};
use yog_gfx::image::{ImageSource, Stretch};
use yog_ids::{yui, yui_attr as attr};

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
    let mut out = ScreenArea {
        next: first,
        ..ScreenArea::default()
    };
    for part in &layout.parts {
        match part {
            LayoutPart::Image { source, uv, at } => out.elements.push(picture(source, *uv, *at)),
            LayoutPart::Slots { slot, at, .. } => {
                for (i, p) in at.iter().enumerate() {
                    out.places.push((slot.clone(), i, out.next));
                    out.slot(*p);
                }
            }
            LayoutPart::Result { at, .. } => {
                out.result = Some(out.next);
                out.slot(*at);
            }
            LayoutPart::Item { item, at } => out
                .elements
                .push(icon(ImageSource::Item(item.clone()), *at)),
            LayoutPart::Block { block, at, .. } => {
                let id = block
                    .split(yog_ids::sep::STATE_OPEN)
                    .next()
                    .unwrap_or(block);
                out.elements
                    .push(icon(ImageSource::Block(id.to_string()), *at));
            }
            LayoutPart::Text { text, at } if !text.contains(VALUE_OPEN) => {
                out.elements.push(label(text, *at))
            }
            LayoutPart::Text { .. } | LayoutPart::Count { .. } => {}
        }
    }
    out
}

impl ScreenArea {
    /// The next screen slot, its item at `at`.
    fn slot(&mut self, at: [f32; 2]) {
        let mut attrs = vec![(attr::INDEX, self.next.to_string())];
        attrs.extend(xy(at, SLOT_BORDER));
        self.elements.push(ScreenElement {
            kind: yui::SLOT,
            attrs,
        });
        self.next += 1;
    }
}

/// Where a text binds a recipe's value (`{time}`).
const VALUE_OPEN: char = '{';

/// `at`, less `d`, as `x` and `y` attributes.
fn xy(at: [f32; 2], d: f32) -> [(&'static str, String); 2] {
    [
        (attr::X, (at[0] - d).to_string()),
        (attr::Y, (at[1] - d).to_string()),
    ]
}

/// An icon of `source` (an item's, a block's) at `at`.
fn icon(source: ImageSource, at: [f32; 2]) -> ScreenElement {
    let mut attrs = vec![
        (attr::SOURCE, source.to_string()),
        (attr::W, ICON_PX.to_string()),
        (attr::H, ICON_PX.to_string()),
    ];
    attrs.extend(xy(at, 0.0));
    ScreenElement {
        kind: yui::IMAGE,
        attrs,
    }
}

/// A picture of the layout: `source`'s region `uv` (a sprite is whole) at `at`.
fn picture(source: &str, uv: [f32; 4], at: [f32; 2]) -> ScreenElement {
    let [u, v, w, h] = uv;
    let mut attrs = vec![
        (attr::SOURCE, source.to_string()),
        (attr::W, w.to_string()),
        (attr::H, h.to_string()),
        (attr::STRETCH, Stretch::Fill.name().into()),
    ];
    attrs.extend(xy(at, 0.0));
    if !matches!(ImageSource::parse(source), Some(ImageSource::Sprite(_))) {
        attrs.push((attr::UV, format!("{u} {v} {w} {h}")));
    }
    ScreenElement {
        kind: yui::IMAGE,
        attrs,
    }
}

/// A text as it is, in the game's dark grey, shadowless.
fn label(text: &str, at: [f32; 2]) -> ScreenElement {
    let mut attrs = vec![
        (attr::TEXT, text.to_string()),
        (attr::COLOR, LABEL_COLOR.into()),
        (attr::SHADOW, false.to_string()),
    ];
    attrs.extend(xy(at, 0.0));
    ScreenElement {
        kind: yui::LABEL,
        attrs,
    }
}
