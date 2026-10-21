//! A recipe layout from its `.yui` component (`<Component recipe_layout="…">`).

use yog_ids::{yui, yui_attr};

use super::{ICON_PX, LayoutPart, RecipeLayout, game_kinds};
use crate::yui::Element;

/// What separates the kinds `recipe_layout` names.
const KIND_SEPARATORS: [char; 2] = [' ', ','];
/// How a layout's text binds a recipe's value (`{time}`), where a label's binds it `{x:Bind time}`.
const VALUE: &str = "{";
/// A yes in a flag attribute.
const YES: &str = "true";

/// Whether `root` is a recipe layout (`<Component recipe_layout="…">`).
pub fn is_recipe_layout(root: &Element) -> bool {
    root.name == yui::COMPONENT && root.attr(yui_attr::RECIPE_LAYOUT).is_some()
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
    Ok([
        num(el, yui_attr::X, Some(0.0))?,
        num(el, yui_attr::Y, Some(0.0))?,
    ])
}

fn flag(el: &Element, key: &str) -> bool {
    el.attr(key).is_some_and(|v| v.trim() == YES)
}

/// Attribute `key`, which `el` needs (`what` says what it is).
fn needs<'a>(el: &'a Element, key: &str, what: &str) -> Result<&'a str, String> {
    el.attr(key)
        .map(Ok)
        .unwrap_or_else(|| err(el, format_args!("needs {key}=\"…\"{what}")))
}

/// `root` (a `<Component recipe_layout="…">`) as a layout; `ns`: the mod's
/// namespace, for its own kinds written without one.
pub fn compile(root: &Element, ns: &str) -> Result<RecipeLayout, String> {
    let kinds = kinds(root, ns);
    if kinds.is_empty() {
        return err(
            root,
            "recipe_layout=\"…\": the kinds it shows — the mod's own or the game's",
        );
    }
    let size = [
        num(root, yui_attr::WIDTH, None)?,
        num(root, yui_attr::HEIGHT, None)?,
    ];
    let mut parts = Parts::default();
    for el in elements(root) {
        parts.add(el)?;
    }
    Ok(RecipeLayout {
        kinds,
        size,
        parts: parts.finish()?,
    })
}

/// The kinds `root` shows: the game's as they are, the mod's in its namespace.
fn kinds(root: &Element, ns: &str) -> Vec<String> {
    root.attr(yui_attr::RECIPE_LAYOUT)
        .unwrap_or_default()
        .split(KIND_SEPARATORS)
        .filter(|k| !k.is_empty())
        .map(|k| {
            if game_kinds().any(|g| g == k) {
                k.to_string()
            } else {
                yog_ids::qualify_in(ns, k)
            }
        })
        .collect()
}

/// The layout's elements: `root`'s, or those of the one `<Canvas>` they sit in, as a screen's do.
fn elements(root: &Element) -> Vec<&Element> {
    let mut out = Vec::new();
    for c in &root.children {
        if c.name == yui::CANVAS {
            out.extend(c.children.iter());
        } else {
            out.push(c);
        }
    }
    out
}

/// A slot's places: (index, where).
type Places = Vec<(usize, [f32; 2])>;

/// The parts read so far, and each slot's places, gathered into its one part.
#[derive(Default)]
struct Parts {
    parts: Vec<LayoutPart>,
    /// A slot, its places and whether it's framed.
    places: Vec<(String, Places, bool)>,
}

impl Parts {
    fn add(&mut self, el: &Element) -> Result<(), String> {
        let part = match el.name.as_str() {
            yui::IMAGE => image(el)?,
            yui::RECIPE_SLOT => return self.slot(el),
            yui::RECIPE_RESULT => LayoutPart::Result {
                at: at(el)?,
                frame: flag(el, yui_attr::FRAME),
            },
            yui::RECIPE_COUNT => LayoutPart::Count { at: at(el)? },
            yui::ITEM_SLOT => {
                let item = needs(el, yui_attr::ITEM, " (the station: minecraft:furnace)")?;
                LayoutPart::Item {
                    item: yog_ids::qualify(
                        item.trim().trim_start_matches(yog_ids::reference::ITEM),
                    ),
                    at: at(el)?,
                }
            }
            yui::BLOCK_SLOT => block(el)?,
            yui::LABEL => LayoutPart::Text {
                text: needs(el, yui_attr::TEXT, "")?.replace(yog_ids::yui_markup::BIND_OPEN, VALUE),
                at: at(el)?,
            },
            other => {
                return err(
                    el,
                    format_args!(
                        "not a recipe layout's element (`{other}`) — {}",
                        [
                            yui::IMAGE,
                            yui::RECIPE_SLOT,
                            yui::RECIPE_RESULT,
                            yui::RECIPE_COUNT,
                            yui::ITEM_SLOT,
                            yui::BLOCK_SLOT,
                            yui::LABEL
                        ]
                        .join(", ")
                    ),
                );
            }
        };
        self.parts.push(part);
        Ok(())
    }

    /// A place of a slot: the slot's part made the first time.
    fn slot(&mut self, el: &Element) -> Result<(), String> {
        let slot = needs(el, yui_attr::SLOT, " (the kind's slot)")?.to_string();
        let index = match el.attr(yui_attr::INDEX) {
            Some(i) => i
                .trim()
                .parse::<usize>()
                .or_else(|_| err(el, format_args!("index=\"{i}\": 0, 1, …")))?,
            None => self
                .places
                .iter()
                .find(|(s, ..)| *s == slot)
                .map_or(0, |(_, p, _)| {
                    p.iter().map(|(i, _)| i + 1).max().unwrap_or(0)
                }),
        };
        let place = (index, at(el)?);
        let framed = flag(el, yui_attr::FRAME);
        match self.places.iter_mut().find(|(s, ..)| *s == slot) {
            Some((_, p, frame)) => {
                if p.iter().any(|(i, _)| *i == index) {
                    return err(el, format_args!("{slot}'s place {index} is laid out twice"));
                }
                p.push(place);
                *frame |= framed;
            }
            None => {
                self.places.push((slot.clone(), vec![place], framed));
                self.parts.push(LayoutPart::Slots {
                    slot,
                    at: Vec::new(),
                    frame: false,
                });
            }
        }
        Ok(())
    }

    /// The parts, each slot's places in order (0, 1, … with no gap).
    fn finish(mut self) -> Result<Vec<LayoutPart>, String> {
        for part in &mut self.parts {
            if let LayoutPart::Slots { slot, at, frame } = part
                && let Some((_, p, f)) = self.places.iter().find(|(s, ..)| s == slot)
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
        Ok(self.parts)
    }
}

/// `<Image source uv>` (or `w h` from the image's corner).
fn image(el: &Element) -> Result<LayoutPart, String> {
    let source = needs(el, yui_attr::SOURCE, "")?.to_string();
    let uv = match el.attr(yui_attr::UV) {
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
        None => [
            0.0,
            0.0,
            num(el, yui_attr::W, None)?,
            num(el, yui_attr::H, None)?,
        ],
    };
    Ok(LayoutPart::Image {
        source,
        uv,
        at: at(el)?,
    })
}

/// `<BlockSlot block size>`: a block state in 3D.
fn block(el: &Element) -> Result<LayoutPart, String> {
    let block = needs(
        el,
        yui_attr::BLOCK,
        " (a block state: minecraft:brewing_stand[has_bottle_0=true])",
    )?;
    Ok(LayoutPart::Block {
        block: yog_ids::qualify(block.trim().trim_start_matches(yog_ids::reference::BLOCK)),
        at: at(el)?,
        size: num(el, yui_attr::SIZE, Some(ICON_PX))?,
    })
}
