//! A block's `block`: a tool level with a tool, an inventory named by a
//! screen's id, and what a block of a game kind takes (the kind's fields,
//! its base) and doesn't (Yog's own fields, states).

use yog_data::{Diagnostic, Path, field};

use super::{error, warning};
use crate::content::{BlockFile, BlockKind, BlockProps};

/// Block file `f`'s `block` and kind rules.
pub(crate) fn block_rules(f: &BlockFile, out: &mut Vec<Diagnostic>) {
    let none = BlockProps::default();
    let b = f.block.as_ref().unwrap_or(&none);
    let at = Path::root().key(field!(BlockFile, block));
    let kind = f.kind.as_deref().and_then(BlockKind::built_in);
    if b.tool_level.is_some() && b.tool.is_empty() {
        out.push(warning(
            at.clone(),
            "tool_level without tool — any tool of that level mines it",
        ));
    }
    if let Some(inv) = &b.inventory
        && !(yog_ids::valid_id(inv) && inv.contains(yog_ids::sep::NAMESPACE))
    {
        out.push(error(
            at.key(field!(BlockProps, inventory)),
            format!("`{inv}`: the id of an inventory screen — a .yui <Screen id=\"mymod:name\"> with <Slot>s"),
        ));
    }
    if let Some(k) = kind {
        kind_rules(f, b, k, &at, out);
    }
    kind_field_rules(b, kind, &at, out);
}

/// A block of game kind `k`: its base given, none of Yog's own fields or states.
fn kind_rules(f: &BlockFile, b: &BlockProps, k: &BlockKind, at: &Path, out: &mut Vec<Diagnostic>) {
    let own = [
        (field!(BlockProps, shape), b.shape.is_some()),
        (
            field!(BlockProps, connect_groups),
            !b.connect_groups.is_empty(),
        ),
        (field!(BlockProps, inventory), b.inventory.is_some()),
        (field!(BlockProps, redstone), b.redstone),
        (field!(BlockProps, powered), b.powered.is_some()),
        (field!(BlockProps, comparator), b.comparator.is_some()),
        (field!(BlockProps, accepts_items), b.accepts_items),
    ];
    for (name, _) in own.iter().filter(|(_, given)| *given) {
        out.push(error(
            at.key(name),
            format!("{name} doesn't go with kind {} (the game's block)", k.name),
        ));
    }
    if f.states.is_some() {
        out.push(error(
            Path::root().key(field!(BlockFile, states)),
            format!("kind {} has the game's states — leave states out", k.name),
        ));
    }
    if k.takes(field!(BlockProps, base)) && b.base.is_none() {
        out.push(error(
            at.clone(),
            format!(
                "kind {} is made from a block — base: \"@block:mymod:planks\"",
                k.name
            ),
        ));
    }
}

/// The `block` fields only some kinds take, taken only by them.
fn kind_field_rules(
    b: &BlockProps,
    kind: Option<&BlockKind>,
    at: &Path,
    out: &mut Vec<Diagnostic>,
) {
    let given = [
        (field!(BlockProps, base), b.base.is_some()),
        (field!(BlockProps, set), b.set.is_some()),
        (field!(BlockProps, press_ticks), b.press_ticks.is_some()),
        (field!(BlockProps, triggered_by), b.triggered_by.is_some()),
    ];
    for (name, _) in given.iter().filter(|(_, given)| *given) {
        match kind {
            Some(k) if k.takes(name) => {}
            Some(k) => out.push(error(
                at.key(name),
                format!("kind {} doesn't take {name}", k.name),
            )),
            None => out.push(error(
                at.key(name),
                "only a block of a kind takes it (kind: \"stairs\", \"door\"…)",
            )),
        }
    }
}
