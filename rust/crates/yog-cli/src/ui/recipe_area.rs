//! `<RecipeArea kind="crusher" x="…" y="…"/>` in an inventory screen: the
//! kind's layout (its `.yui` `recipe_layout`, see `yog_formats::book::layout`) made
//! real — its places become the container's slots (numbered after the
//! screen's own), its result an output slot, its pictures and stations
//! drawn — and a `<Machine>` that tells the game which slot is which: the
//! block makes recipes of the kind like a crafting table.

use yog_formats::book::layout::RecipeLayout;

use crate::ui::yui::Element;
use yog_data::Diagnostic;

fn el(name: &str, attrs: &[(&str, String)], line: usize) -> Element {
    Element {
        name: name.into(),
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
        children: Vec::new(),
        line,
    }
}

/// The highest `<Slot index>` in `root`.
fn max_slot(root: &Element) -> Option<usize> {
    let own = (root.name == yog_ids::yui::SLOT)
        .then(|| {
            root.attr(yog_ids::yui_attr::INDEX)
                .and_then(|i| i.trim().parse().ok())
        })
        .flatten();
    root.children.iter().filter_map(max_slot).chain(own).max()
}

/// Expands every `<RecipeArea>` in `root`; `layouts`: the mod's, `ns` its
/// namespace (for a kind written without one).
pub(crate) fn expand(
    root: &mut Element,
    layouts: &[RecipeLayout],
    ns: &str,
) -> Result<(), Diagnostic> {
    let mut next = max_slot(root).map_or(0, |m| m + 1);
    walk(root, layouts, ns, &mut next)
}

fn walk(
    el_: &mut Element,
    layouts: &[RecipeLayout],
    ns: &str,
    next: &mut usize,
) -> Result<(), Diagnostic> {
    for child in &mut el_.children {
        if child.name == yog_ids::yui::RECIPE_AREA {
            *child = area(child, layouts, ns, next)?;
        } else {
            walk(child, layouts, ns, next)?;
        }
    }
    Ok(())
}

fn area(
    a: &Element,
    layouts: &[RecipeLayout],
    ns: &str,
    next: &mut usize,
) -> Result<Element, Diagnostic> {
    let line = a.line;
    let kind = a
        .attr(yog_ids::yui_attr::KIND)
        .map(str::trim)
        .unwrap_or_default();
    if kind.is_empty() {
        return Err(Diagnostic::from(
            "<RecipeArea> needs kind=\"…\" (a kind of recipe the mod declares)".to_string(),
        )
        .at_line(line));
    }
    let kind = if kind.contains(':') {
        kind.to_string()
    } else {
        format!("{ns}:{kind}")
    };
    let Some(layout) = layouts.iter().find(|l| l.kinds.contains(&kind)) else {
        return Err(Diagnostic::from(format!("<RecipeArea kind=\"{kind}\">: no layout for it — a .yui <Component recipe_layout=\"…\">")).at_line(line));
    };
    let mut canvas = el(
        yog_ids::yui::CANVAS,
        &[
            (yog_ids::yui_attr::W, layout.size[0].to_string()),
            (yog_ids::yui_attr::H, layout.size[1].to_string()),
        ],
        line,
    );
    for k in ["x", "y", "Grid.Row", "Grid.Column", "margin"] {
        if let Some(v) = a.attr(k) {
            canvas.attrs.push((k.to_string(), v.to_string()));
        }
    }
    let area = yog_book::recipe_area::screen_area(layout, *next);
    for e in &area.elements {
        let attrs: Vec<(&str, String)> = e.attrs.iter().map(|(k, v)| (*k, v.clone())).collect();
        canvas.children.push(el(e.kind, &attrs, line));
    }
    *next = area.next;
    let places: Vec<String> = area
        .places
        .iter()
        .map(|(slot, i, at)| format!("{slot}/{i}={at}"))
        .collect();
    let result = area.result;
    let Some(result) = result else {
        return Err(Diagnostic::from(format!("<RecipeArea kind=\"{kind}\">: its layout has no <RecipeResult> — where would the result go?")).at_line(line));
    };
    canvas.children.push(el(
        yog_ids::yui::MACHINE,
        &[
            (yog_ids::yui_attr::KIND, kind),
            (yog_ids::yui_attr::RESULT, result.to_string()),
            (yog_ids::yui_attr::PLACES, places.join(",")),
        ],
        line,
    ));
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recipe_area_becomes_slots_after_the_screens_own_and_a_machine() {
        let layout = yog_formats::book::layout::compile(
            &yog_formats::yui::parse(
                r#"<Component recipe_layout="crusher" width="60" height="24">
                    <RecipeSlot slot="input" x="4" y="4"/><RecipeSlot slot="input" x="22" y="4"/>
                    <ItemSlot item="minecraft:grindstone" x="30" y="4"/><RecipeResult x="44" y="4"/>
                </Component>"#,
            )
            .unwrap(),
            "m",
        )
        .unwrap();
        let mut screen = crate::ui::yui::parse_xml(
            r#"<Screen width="176" height="166"><Slot index="0"/><Slot index="1"/><RecipeArea kind="crusher" x="10" y="20"/><PlayerInventory/></Screen>"#,
        )
        .unwrap();
        expand(&mut screen, &[layout], "m").unwrap();
        let canvas = &screen.children[2];
        assert_eq!(
            (canvas.name.as_str(), canvas.attr("x")),
            ("Canvas", Some("10"))
        );
        let slots: Vec<&str> = canvas
            .children
            .iter()
            .filter(|c| c.name == "Slot")
            .filter_map(|c| c.attr("index"))
            .collect();
        assert_eq!(slots, ["2", "3", "4"], "after the screen's own");
        let machine = canvas
            .children
            .iter()
            .find(|c| c.name == "Machine")
            .unwrap();
        assert_eq!(machine.attr("kind"), Some("m:crusher"));
        assert_eq!(machine.attr("result"), Some("4"));
        assert_eq!(machine.attr("places"), Some("input/0=2,input/1=3"));
        let mut bad =
            crate::ui::yui::parse_xml(r#"<Screen><RecipeArea kind="nope"/></Screen>"#).unwrap();
        assert!(
            expand(&mut bad, &[], "m")
                .unwrap_err()
                .message
                .contains("no layout for it")
        );
    }
}
