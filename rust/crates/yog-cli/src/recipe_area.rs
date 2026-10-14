//! `<RecipeArea kind="crusher" x="…" y="…"/>` in an inventory screen: the
//! kind's layout (its `.yui` `recipe_layout`, see `yog_book::layout`) made
//! real — its places become the container's slots (numbered after the
//! screen's own), its result an output slot, its pictures and stations
//! drawn — and a `<Machine>` that tells the game which slot is which: the
//! block makes recipes of the kind like a crafting table.

use yog_book::layout::{LayoutPart, RecipeLayout};

use crate::yui::Element;

fn el(name: &str, attrs: &[(&str, String)], line: usize) -> Element {
    Element { name: name.into(), attrs: attrs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(), children: Vec::new(), line }
}

/// The highest `<Slot index>` in `root`.
fn max_slot(root: &Element) -> Option<usize> {
    let own = (root.name == "Slot").then(|| root.attr("index").and_then(|i| i.trim().parse().ok())).flatten();
    root.children.iter().filter_map(max_slot).chain(own).max()
}

/// Expands every `<RecipeArea>` in `root`; `layouts`: the mod's, `ns` its
/// namespace (for a kind written without one).
pub fn expand(root: &mut Element, layouts: &[RecipeLayout], ns: &str) -> Result<(), String> {
    let mut next = max_slot(root).map_or(0, |m| m + 1);
    walk(root, layouts, ns, &mut next)
}

fn walk(el_: &mut Element, layouts: &[RecipeLayout], ns: &str, next: &mut usize) -> Result<(), String> {
    for child in &mut el_.children {
        if child.name == "RecipeArea" {
            *child = area(child, layouts, ns, next)?;
        } else {
            walk(child, layouts, ns, next)?;
        }
    }
    Ok(())
}

fn area(a: &Element, layouts: &[RecipeLayout], ns: &str, next: &mut usize) -> Result<Element, String> {
    let line = a.line;
    let kind = a.attr("kind").map(str::trim).unwrap_or_default();
    if kind.is_empty() {
        return Err(format!("line {line}: <RecipeArea> needs kind=\"…\" (a kind of recipe the mod declares)"));
    }
    let kind = if kind.contains(':') { kind.to_string() } else { format!("{ns}:{kind}") };
    let Some(layout) = layouts.iter().find(|l| l.kinds.contains(&kind)) else {
        return Err(format!("line {line}: <RecipeArea kind=\"{kind}\">: no layout for it — a .yui <Component recipe_layout=\"…\">"));
    };
    let mut canvas = el("Canvas", &[("w", layout.size[0].to_string()), ("h", layout.size[1].to_string())], line);
    for k in ["x", "y", "Grid.Row", "Grid.Column", "margin"] {
        if let Some(v) = a.attr(k) {
            canvas.attrs.push((k.to_string(), v.to_string()));
        }
    }
    let pos = |p: [f32; 2], d: f32| [("x", (p[0] - d).to_string()), ("y", (p[1] - d).to_string())];
    let mut places: Vec<String> = Vec::new();
    let mut result = None;
    for part in &layout.parts {
        match part {
            LayoutPart::Image { source, uv, at } => {
                let [u, v, w, h] = *uv;
                let [x, y] = pos(*at, 0.0);
                canvas.children.push(el(
                    "Image",
                    &[("source", source.clone()), ("uv", format!("{u} {v} {w} {h}")), ("w", w.to_string()), ("h", h.to_string()), ("stretch", "Fill".into()), x, y],
                    line,
                ));
            }
            // A slot frame is 18 px with the item at +1.
            LayoutPart::Slots { slot, at, .. } => {
                for (i, p) in at.iter().enumerate() {
                    let [x, y] = pos(*p, 1.0);
                    canvas.children.push(el("Slot", &[("index", next.to_string()), x, y], line));
                    places.push(format!("{slot}/{i}={next}"));
                    *next += 1;
                }
            }
            LayoutPart::Result { at, .. } => {
                let [x, y] = pos(*at, 1.0);
                canvas.children.push(el("Slot", &[("index", next.to_string()), x, y], line));
                result = Some(*next);
                *next += 1;
            }
            LayoutPart::Item { item, at } => {
                let [x, y] = pos(*at, 0.0);
                canvas.children.push(el("Image", &[("source", format!("item:{item}")), ("w", "16".into()), ("h", "16".into()), x, y], line));
            }
            // Bound to a recipe's values, a text has nothing to show here.
            LayoutPart::Text { text, at } if !text.contains('{') => {
                let [x, y] = pos(*at, 0.0);
                canvas.children.push(el("Label", &[("text", text.clone()), ("color", "#FF404040".into()), ("shadow", "false".into()), x, y], line));
            }
            LayoutPart::Text { .. } | LayoutPart::Count { .. } => {}
        }
    }
    let Some(result) = result else {
        return Err(format!("line {line}: <RecipeArea kind=\"{kind}\">: its layout has no <RecipeResult> — where would the result go?"));
    };
    canvas.children.push(el("Machine", &[("kind", kind), ("result", result.to_string()), ("places", places.join(","))], line));
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recipe_area_becomes_slots_after_the_screens_own_and_a_machine() {
        let layout = yog_book::layout::compile(
            &yog_book::ui::parse(
                r#"<Component recipe_layout="crusher" width="60" height="24">
                    <RecipeSlot slot="input" x="4" y="4"/><RecipeSlot slot="input" x="22" y="4"/>
                    <ItemSlot item="minecraft:grindstone" x="30" y="4"/><RecipeResult x="44" y="4"/>
                </Component>"#,
            )
            .unwrap(),
            "m",
        )
        .unwrap();
        let mut screen = crate::yui::parse_xml(
            r#"<Screen width="176" height="166"><Slot index="0"/><Slot index="1"/><RecipeArea kind="crusher" x="10" y="20"/><PlayerInventory/></Screen>"#,
        )
        .unwrap();
        expand(&mut screen, &[layout], "m").unwrap();
        let canvas = &screen.children[2];
        assert_eq!((canvas.name.as_str(), canvas.attr("x")), ("Canvas", Some("10")));
        let slots: Vec<&str> = canvas.children.iter().filter(|c| c.name == "Slot").filter_map(|c| c.attr("index")).collect();
        assert_eq!(slots, ["2", "3", "4"], "after the screen's own");
        let machine = canvas.children.iter().find(|c| c.name == "Machine").unwrap();
        assert_eq!(machine.attr("kind"), Some("m:crusher"));
        assert_eq!(machine.attr("result"), Some("4"));
        assert_eq!(machine.attr("places"), Some("input/0=2,input/1=3"));
        let mut bad = crate::yui::parse_xml(r#"<Screen><RecipeArea kind="nope"/></Screen>"#).unwrap();
        assert!(expand(&mut bad, &[], "m").unwrap_err().contains("no layout for it"));
    }
}
