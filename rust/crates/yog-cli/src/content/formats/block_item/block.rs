//! A `.yblock`: its properties checked, a game kind's limits, its models,
//! states, item form and drops written.

use serde_json::{Value, json};
use yog_data::Path as At;
use yog_data::field;
use yog_formats::content::BlockProps as Props;
use yog_formats::content::{BlockFile, BlockKind, BlockShape, Drops, TriggeredBy};

use crate::content::formats::block_item::{Emit, Parts, drops_table, kind_drops, states};
use crate::content::formats::model::{KindBlock, kind_models, model};
use crate::content::pipeline::ctx::{Ctx, Loc};
use crate::content::pipeline::entry::Entry;
use crate::content::{pipeline::datapack, pretty, valid_id};

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
        kind_props(parts, k, entry);
    }
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
    // An inventory that isn't a screen's id is yog_formats::rules' to report.
    if let Some(inv) = &b.inventory
        && valid_id(inv)
        && inv.contains(yog_ids::sep::NAMESPACE)
    {
        let at = bat.key(field!(Props, inventory));
        p.inventory = Some(inv.clone());
        cx.inventories
            .push((cx.file.clone(), loc(&at), inv.clone()));
    }
}

/// A kind of the game's: what it's made of and its set (what it doesn't
/// take is yog_formats::rules' to report).
fn kind_props(parts: &Parts, k: &BlockKind, entry: &mut Entry) {
    let none = Props::default();
    let b = parts.block.as_ref().unwrap_or(&none);
    let p = &mut entry.block;
    p.kind = k.name.clone();
    p.base = b.base.clone();
    p.set = b.set.clone();
    p.press_ticks = b.press_ticks;
    p.mobs_only = b.triggered_by == Some(TriggeredBy::Mobs);
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
        (Some(im), _) => {
            model(cx, loc, &At::root().key(field!(BlockFile, item_model)), im).map(|m| m.json)
        }
    };
    match item_model {
        Some(im) => emit.item_model(cx, &im),
        None => {
            // The definition still names the item's model.
            let def = json!({ crate::consts::item_def::MODEL: {
                crate::consts::item_def::TYPE: crate::consts::item_def::PLAIN,
                crate::consts::item_def::MODEL: yog_ids::join(&emit.ns, &format!("{}{}", yog_ids::res::ITEM_DIR, emit.p)),
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
