//! A `.yblock`: its properties checked, a game kind's limits, its models,
//! states, item form and drops written.

use serde_json::{Value, json};
use yog_data::Path as At;
use yog_data::field;
use yog_formats::content::BlockProps as Props;
use yog_formats::content::{BlockFile, BlockKind, BlockShape, Drops, TriggeredBy};

use super::{Emit, Parts, drops_table, kind_drops, states};
use crate::content::model::{KindBlock, kind_models, model};
use crate::content::{Ctx, Entry, Loc, datapack, pretty, valid_id};

/// The block's properties onto `entry`; its models, states, item form,
/// loot table and tool tags written.
pub(super) fn compile(
    cx: &mut Ctx,
    loc: Loc,
    parts: &Parts,
    entry: &mut Entry,
    emit: &mut Emit,
) -> Option<()> {
    let b = parts.block.clone().unwrap_or_default();
    let kind_of = parts.kind.as_deref().and_then(BlockKind::built_in);
    props(cx, loc, &b, kind_of, entry);
    if let Some(k) = kind_of {
        check_kind(cx, loc, parts, k, entry);
    }
    check_kind_fields(cx, loc, &b, kind_of);
    let drops = match &b.drops {
        None | Some(Drops::Itself) => match kind_of {
            Some(k) => kind_drops(&entry.id, k),
            None => drops_table(&entry.id, None),
        },
        Some(Drops::Nothing) => Value::Null,
        Some(Drops::Item(d)) => drops_table(d.item.as_deref().unwrap_or(&entry.id), d.count),
    };
    let render_layer = b.render_layer.as_ref().map(yog_formats::content::word);
    // Every `.yblock` has a model: its schema requires one.
    let block_model = parts.model.as_ref()?;
    // Its models and states: a kind's, or its own.
    let item_parent = match kind_of {
        Some(k) => {
            let kind = KindBlock {
                kind: k,
                base: b.base.as_deref(),
                render_layer: render_layer.as_deref(),
            };
            kind_models(cx, loc, &kind, block_model, emit)
        }
        None => {
            let own = states::Own {
                parts,
                block: &b,
                model: block_model,
                render_layer: render_layer.as_deref(),
            };
            Some(states::compile(cx, loc, &own, entry, emit)?)
        }
    };
    item_form(cx, loc, parts, item_parent, emit);
    write_drops(cx, &drops, emit);
    // Tools and tiers are block tags: the block joins each.
    // (The tool level's tags come once every `.ydef` is read: `tool_level_tags`.)
    for tag in &entry.block.tools {
        emit.out
            .tags
            .entry(datapack::tag_file(yog_ids::kind::BLOCK, tag))
            .or_default()
            .insert(entry.id.clone(), true);
    }
    Some(())
}

/// What the block is: how hard, loud and bright, which tools mine it, whether
/// it's solid, its shape and inventory.
fn props(cx: &mut Ctx, loc: Loc, b: &Props, kind_of: Option<&BlockKind>, entry: &mut Entry) {
    let bat = At::root().key(field!(BlockFile, block));
    let p = &mut entry.block;
    p.hardness = b.hardness;
    p.resistance = b.resistance;
    p.sound = b.sound.as_ref().map(yog_formats::content::word);
    p.light = b.light.unwrap_or(0);
    p.tools = b.tool.iter().map(|t| t.0.clone()).collect();
    p.tool_level = b
        .tool_level
        .clone()
        .map(|l| (l, loc(&bat.key(field!(Props, tool_level)))));
    p.requires_tool = b.requires_tool;
    p.no_collision = !b
        .collision
        .or(kind_of.and_then(|k| k.collision))
        .unwrap_or(true);
    p.no_occlusion = !b
        .occludes
        .or(kind_of.and_then(|k| k.occludes))
        .unwrap_or(true);
    p.slipperiness = b.slipperiness;
    p.redstone = b.redstone;
    p.accepts_items = b.accepts_items;
    p.connect_groups = b.connect_groups.clone();
    if let Some(BlockShape::Box(bx)) = b.shape {
        p.shape = Some(bx);
    }
    if let Some(inv) = &b.inventory {
        let at = bat.key(field!(Props, inventory));
        if valid_id(inv) && inv.contains(yog_ids::sep::NAMESPACE) {
            p.inventory = Some(inv.clone());
            cx.inventories
                .push((cx.file.clone(), loc(&at), inv.clone()));
        } else {
            cx.err_at(loc, &at, format!("`{inv}`: the id of an inventory screen — a .yui <Screen id=\"mymod:name\"> with <Slot>s"));
        }
    }
    if p.tool_level.is_some() && p.tools.is_empty() {
        cx.warn_at(
            loc,
            &bat,
            "tool_level without tool — any tool of that level mines it",
        );
    }
}

/// A kind of the game's: what it's made of and its set; nothing of Yog's own blocks.
fn check_kind(cx: &mut Ctx, loc: Loc, parts: &Parts, k: &BlockKind, entry: &mut Entry) {
    let none = Props::default();
    let b = parts.block.as_ref().unwrap_or(&none);
    let bat = At::root().key(field!(BlockFile, block));
    let p = &mut entry.block;
    p.kind = k.name.clone();
    p.base = b.base.clone();
    p.set = b.set.clone();
    p.press_ticks = b.press_ticks;
    p.mobs_only = b.triggered_by == Some(TriggeredBy::Mobs);
    let own = [
        (field!(Props, shape), b.shape.is_some()),
        (field!(Props, connect_groups), !b.connect_groups.is_empty()),
        (field!(Props, inventory), b.inventory.is_some()),
        (field!(Props, redstone), b.redstone),
        (field!(Props, powered), b.powered.is_some()),
        (field!(Props, comparator), b.comparator.is_some()),
        (field!(Props, accepts_items), b.accepts_items),
    ];
    for (field, _) in own.iter().filter(|(_, given)| *given) {
        cx.err_at(
            loc,
            &bat.key(field),
            format!("{field} doesn't go with kind {} (the game's block)", k.name),
        );
    }
    if parts.states.is_some() {
        cx.err_at(
            loc,
            &At::root().key(field!(BlockFile, states)),
            format!("kind {} has the game's states — leave states out", k.name),
        );
    }
    if k.takes(field!(Props, base)) && b.base.is_none() {
        cx.err_at(
            loc,
            &bat,
            format!(
                "kind {} is made from a block — base: \"@block:mymod:planks\"",
                k.name
            ),
        );
    }
}

/// The `block` fields only some kinds take, taken only by them.
fn check_kind_fields(cx: &mut Ctx, loc: Loc, b: &Props, kind_of: Option<&BlockKind>) {
    let bat = At::root().key(field!(BlockFile, block));
    let given = [
        (field!(Props, base), b.base.is_some()),
        (field!(Props, set), b.set.is_some()),
        (field!(Props, press_ticks), b.press_ticks.is_some()),
        (field!(Props, triggered_by), b.triggered_by.is_some()),
    ];
    for (field, _) in given.iter().filter(|(_, given)| *given) {
        match kind_of {
            Some(k) if k.takes(field) => {}
            Some(k) => cx.err_at(
                loc,
                &bat.key(field),
                format!("kind {} doesn't take {field}", k.name),
            ),
            None => cx.err_at(
                loc,
                &bat.key(field),
                "only a block of a kind takes it (kind: \"stairs\", \"door\"…)",
            ),
        }
    }
}

/// The item form: the block model (`parent`), or its own.
fn item_form(cx: &mut Ctx, loc: Loc, parts: &Parts, parent: Option<String>, emit: &mut Emit) {
    let item_model = match (&parts.item_model, parent) {
        (None, Some(parent)) => Some(json!({ yog_ids::model::PARENT: parent })),
        (None, None) => {
            let kind = parts.kind.as_deref().unwrap_or_default();
            cx.err_at(
                loc,
                &At::root(),
                format!("kind {kind} has no item model of its own — give item_model"),
            );
            None
        }
        (Some(im), _) => model(
            cx,
            loc,
            &At::root().key(field!(BlockFile, item_model)),
            im,
            true,
        )
        .map(|m| m.json),
    };
    match item_model {
        Some(im) => emit.item_model(cx, &im),
        None => {
            // The definition still names the item's model.
            let def = json!({ yog_ids::item_def::MODEL: {
                yog_ids::item_def::TYPE: yog_ids::item_def::PLAIN,
                yog_ids::item_def::MODEL: yog_ids::join(&emit.ns, &format!("{}{}", yog_ids::res::ITEM_DIR, emit.p)),
            } });
            emit.asset(cx, yog_ids::item_definition_file(&emit.p), &def);
        }
    }
}

/// The block's loot table (none when it drops nothing); a hand-written one
/// is in the way.
fn write_drops(cx: &mut Ctx, drops: &Value, emit: &mut Emit) {
    if drops.is_null() {
        return;
    }
    let id = yog_ids::join(&emit.ns, &format!("{}{}", yog_ids::loot::BLOCKS, emit.p));
    let files = datapack::data_files(yog_ids::kind::LOOT_TABLE, &id);
    if files.iter().any(|f| cx.root.join(f).is_file()) {
        cx.err(1, format!("{} is written by this file (block.drops) — delete the hand-written one, or set drops: \"none\"", files[0]));
    }
    emit.out.outputs.push((files[0].clone(), pretty(drops)));
}
