//! A `.yitem`: its properties checked, its model and definition written.

use yog_data::Path as At;
use yog_data::field;
use yog_formats::content::ItemProps as Props;
use yog_formats::content::{HangsProps, ItemFile};

use super::{Emit, Parts};
use crate::content::model::model;
use crate::content::ctx::{Ctx, Loc};
use crate::content::entry::Entry;

/// The item's properties onto `entry`, its model (none for a potion-only
/// file without one) and definition written.
pub(super) fn compile(
    cx: &mut Ctx,
    loc: Loc,
    parts: &Parts,
    entry: &mut Entry,
    emit: &mut Emit,
) -> Option<()> {
    let root = At::root();
    if let Some(it) = &parts.item {
        props(cx, loc, it, entry);
    }
    let at = root.key(field!(ItemFile, model));
    if entry.potion.is_some() {
        if let Some(item_model) = parts.model.as_ref() {
            let m = model(cx, loc, &at, item_model, true)?;
            emit.asset(
                cx,
                yog_ids::model_file(&format!("{}{}", yog_ids::res::ITEM_DIR, emit.p)),
                &m.json,
            );
        }
        return Some(());
    }
    let Some(item_model) = parts.model.as_ref() else {
        cx.err_at(loc, &at, "an item without potion effects needs a model");
        return None;
    };
    let m = model(cx, loc, &at, item_model, true)?;
    emit.item_model(cx, &m.json);
    Some(())
}

/// What the item is — stack, durability, fuel, food, a tool or armor, worn,
/// hung — and what doesn't go together.
fn props(cx: &mut Ctx, loc: Loc, it: &Props, entry: &mut Entry) {
    let at = At::root().key(field!(ItemFile, item));
    let p = &mut entry.item;
    p.stack_size = it.stack_size;
    p.durability = it.durability.unwrap_or(0);
    p.fire_resistant = it.fire_resistant;
    p.fuel_ticks = it.fuel_ticks.unwrap_or(0);
    p.food = it
        .food
        .as_ref()
        .map(|f| (f.nutrition, f.saturation, f.always_edible));
    p.tool = it.tool.as_ref().map(|t| {
        (
            t.kind.clone(),
            t.material.clone(),
            t.attack_damage,
            t.attack_speed,
        )
    });
    p.armor = it
        .armor
        .as_ref()
        .map(|a| (a.slot, a.material.clone()));
    p.wearable = it.wearable;
    p.hangs = it.hangs.as_ref().map(|h| (h.entity.clone(), h.size));
    if it
        .hangs
        .as_ref()
        .and_then(|h| h.size)
        .is_some_and(|[w, h]| w == 0 || h == 0)
    {
        let size = at.key(field!(Props, hangs)).key(field!(HangsProps, size));
        cx.err_at(
            loc,
            &size,
            "a hanging thing is at least a block wide and high",
        );
    }
    if p.wearable.is_some() && p.armor.is_some() {
        cx.err_at(
            loc,
            &at.key(field!(Props, wearable)),
            "armor is worn already — leave wearable out",
        );
    }
    if p.tool.is_some() && p.armor.is_some() {
        cx.err_at(
            loc,
            &at.key(field!(Props, armor)),
            "an item is a tool or armor, not both",
        );
    }
    if p.durability > 0 && p.stack_size.is_some_and(|s| s > 1) {
        cx.warn_at(
            loc,
            &at.key(field!(Props, stack_size)),
            "ignored: an item with durability stacks to 1",
        );
    }
}
