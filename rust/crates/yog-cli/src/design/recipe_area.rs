//! A `<RecipeArea kind="…">` on an inventory screen, as `yog build` makes
//! it (yog-cli's `recipe_area`): the kind's layout — its places and result
//! real slots, numbered after the screen's own, its pictures and stations
//! drawn — in a canvas of the layout's size.

use std::collections::HashMap;
use std::path::Path;

use yog_formats::book::layout::RecipeLayout;

use super::components::INNER;
use super::markup::YuiNode;

fn node(uid: String, kind: &str, attrs: &[(&str, String)]) -> YuiNode {
    YuiNode {
        uid,
        kind: kind.into(),
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<HashMap<_, _>>(),
        children: Vec::new(),
    }
}

fn max_slot(n: &YuiNode) -> Option<usize> {
    let own = (n.kind == yog_ids::yui::SLOT)
        .then(|| {
            n.attr(yog_ids::yui_attr::INDEX)
                .and_then(|i| i.trim().parse().ok())
        })
        .flatten();
    n.children.iter().filter_map(max_slot).chain(own).max()
}

/// Expands the `<RecipeArea>`s in `root` with the layouts of the mod at
/// `mod_root`; one whose kind has none stays an empty box of its own size.
pub fn expand(root: &mut YuiNode, mod_root: Option<&Path>) {
    if !has_area(root) {
        return;
    }
    let (layouts, ns) = match mod_root {
        Some(r) => (
            crate::design::mod_ui::recipe_layouts(r)
                .into_iter()
                .map(|(_, l)| l)
                .collect(),
            crate::design::mod_ui::ns(r),
        ),
        None => (Vec::new(), yog_ids::ns::MINECRAFT.to_string()),
    };
    let mut next = max_slot(root).map_or(0, |m| m + 1);
    walk(root, &layouts, &ns, &mut next);
}

fn has_area(n: &YuiNode) -> bool {
    n.kind == yog_ids::yui::RECIPE_AREA || n.children.iter().any(has_area)
}

fn walk(n: &mut YuiNode, layouts: &[RecipeLayout], ns: &str, next: &mut usize) {
    for c in &mut n.children {
        if c.kind == yog_ids::yui::RECIPE_AREA {
            area(c, layouts, ns, next);
        } else {
            walk(c, layouts, ns, next);
        }
    }
}

fn area(a: &mut YuiNode, layouts: &[RecipeLayout], ns: &str, next: &mut usize) {
    let kind = a
        .attr(yog_ids::yui_attr::KIND)
        .map(str::trim)
        .unwrap_or_default();
    let kind = if kind.contains(':') {
        kind.to_string()
    } else {
        format!("{ns}:{kind}")
    };
    a.kind = yog_ids::yui::CANVAS.into();
    let Some(layout) = layouts.iter().find(|l| l.kinds.contains(&kind)) else {
        // No layout (yet): a slot-sized box where it'll go.
        a.attrs
            .entry(yog_ids::yui_attr::W.into())
            .or_insert_with(|| "18".into());
        a.attrs
            .entry(yog_ids::yui_attr::H.into())
            .or_insert_with(|| "18".into());
        a.attrs
            .entry(yog_ids::yui_attr::BG.into())
            .or_insert_with(|| "#40000000".into());
        return;
    };
    a.attrs
        .insert(yog_ids::yui_attr::W.into(), layout.size[0].to_string());
    a.attrs
        .insert(yog_ids::yui_attr::H.into(), layout.size[1].to_string());
    let uid = a.uid.clone();
    let area = yog_book::recipe_area::screen_area(layout, *next);
    for (i, e) in area.elements.into_iter().enumerate() {
        a.children
            .push(node(format!("{uid}{INNER}r{i}"), e.kind, &e.attrs));
    }
    *next = area.next;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_area_becomes_its_layouts_slots_after_the_screens() {
        let dir = std::env::temp_dir().join(format!("yog-idle-recipe-area-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"m\"\n").unwrap();
        std::fs::write(
            dir.join("src/press.yui"),
            r#"<Component recipe_layout="press" width="60" height="20"><RecipeSlot slot="input" x="1" y="1"/><RecipeResult x="41" y="1"/></Component>"#,
        )
        .unwrap();
        let slot = node("s".into(), "Slot", &[("index", "3".into())]);
        let area = node("a".into(), "RecipeArea", &[("kind", "press".into())]);
        let mut root = node("c0".into(), "Screen", &[]);
        root.children = vec![slot, area];
        expand(&mut root, Some(&dir));
        let a = &root.children[1];
        assert_eq!((a.kind.as_str(), a.attr("w")), ("Canvas", Some("60")));
        let idx: Vec<&str> = a.children.iter().filter_map(|c| c.attr("index")).collect();
        assert_eq!(idx, ["4", "5"]);
        assert!(
            a.children.iter().all(|c| c.uid.contains(INNER)),
            "not the document's"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
