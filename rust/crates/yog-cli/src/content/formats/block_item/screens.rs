//! The mod's inventory screens: what a block's `inventory` may name.

use std::collections::BTreeSet;
use std::path::Path;

use yog_ids::{Format, yui, yui_attr};

/// Ids of the `.yui` screens under `dir` that lay out an inventory (have
/// `<Slot>`s or a `<PlayerInventory>`).
pub(in crate::content) fn inventory_screens(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            out.extend(inventory_screens(&path));
        } else if Format::Ui.is(&path)
            && let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(root) = yog_formats::yui::parse(&text)
            && root.name == yui::SCREEN
            && has_slots(&root)
            && let Some(id) = root.attr(yui_attr::ID)
        {
            out.insert(id.to_string());
        }
    }
    out
}

/// Whether `el` lays out an inventory's slots, anywhere inside it.
fn has_slots(el: &yog_formats::yui::Element) -> bool {
    el.name == yui::SLOT || el.name == yui::PLAYER_INVENTORY || el.children.iter().any(has_slots)
}
