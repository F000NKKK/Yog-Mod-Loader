//! A `.ydef` advancement: the game's JSON for it, in the newest format and
//! the old one for the versions that read that.

use serde_json::{Map, Value, json};
use yog_data::Path as At;
use yog_data::{Diagnostic, OneOrTag};
use yog_formats::def::{AdvancementDef, CriterionDef};
use yog_ids::advancement as adv;
use yog_ids::kind::{ITEM, TAG as TAG_FIELD};
use yog_ids::ns::MINECRAFT;
use yog_ids::sep::{DOT, TAG};

use crate::content::Lang;
use crate::content::Output;
use crate::content::formats::block_item::text_key;
use crate::content::pipeline::ctx::Ctx;
use crate::content::pipeline::datapack;
use crate::content::pretty;
use crate::content::refs;

/// The version advancements took their current format in (1.20.5): an
/// icon's `id`, item predicates' single `items`.
const NEW_FORMAT: (u32, u32, u32) = (1, 20, 5);

/// Minecraft versions before [`NEW_FORMAT`] write advancements the old way:
/// an icon's `item`, item predicates' `items` lists and `tag`s.
fn old_advancement_format(version: &str) -> bool {
    let mut p = version.split(DOT).map(|n| n.parse::<u32>().unwrap_or(0));
    (
        p.next().unwrap_or(0),
        p.next().unwrap_or(0),
        p.next().unwrap_or(0),
    ) < NEW_FORMAT
}

/// The advancement's fields problems are at.
const FRAME: &str = yog_data::field!(AdvancementDef, frame);
const TITLE: &str = yog_data::field!(AdvancementDef, title);
const DESCRIPTION: &str = yog_data::field!(AdvancementDef, description);
const CRITERIA: &str = yog_data::field!(AdvancementDef, criteria);

/// What the advancement's field `key` (its item `index` of a list) got wrong.
fn problem(key: &str, index: Option<usize>, message: impl std::fmt::Display) -> Diagnostic {
    let field = At::root().key(key);
    let path = index.map_or(field.clone(), |i| field.index(i));
    Diagnostic::error(
        yog_data::Atom::new(yog_ids::build_problem::CONTENT),
        message,
    )
    .at_path(path)
}

/// Advancement `id` (the mod's own, checked): its JSON in the newest format,
/// and the old one as an overlay for the versions that read that.
pub(super) fn advancement(
    cx: &mut Ctx,
    lang: &mut Lang,
    a: &AdvancementDef,
    id: &str,
) -> Result<Vec<Output>, Diagnostic> {
    if ![adv::TASK, adv::GOAL, adv::CHALLENGE].contains(&a.frame.as_str()) {
        return Err(problem(
            FRAME,
            None,
            format!(
                "`{}`: {}, {} or {}",
                a.frame,
                adv::TASK,
                adv::GOAL,
                adv::CHALLENGE
            ),
        ));
    }
    let shown = shown(cx, lang, a, id)?;
    let file = datapack::data_file(yog_ids::kind::ADVANCEMENT, id);
    let mut out = vec![(file.clone(), pretty(&written(a, &shown, Era::Components)?))];
    for version in refs::versions()
        .iter()
        .filter(|v| old_advancement_format(v))
    {
        out.push((
            datapack::for_version(version, &file),
            pretty(&written(a, &shown, Era::Legacy)?),
        ));
    }
    Ok(out)
}

/// Its title's translation key, its description as the game shows it, its parent's id.
struct Shown {
    title: String,
    description: Value,
    parent: Option<String>,
}

fn shown(cx: &mut Ctx, lang: &mut Lang, a: &AdvancementDef, id: &str) -> Result<Shown, Diagnostic> {
    let (ns, path) = yog_ids::split(id);
    let base_key = [
        yog_ids::kind::ADVANCEMENT,
        ns,
        &path.replace(yog_ids::sep::PATH, DOT),
    ]
    .join(DOT);
    let title =
        text_key(cx, lang, &a.title, ns, base_key.clone()).map_err(|e| problem(TITLE, None, e))?;
    let description = match &a.description {
        Some(d) => {
            json!({ "translate": text_key(cx, lang, d, ns, format!("{base_key}.desc")).map_err(|e| problem(DESCRIPTION, None, e))? })
        }
        None => json!({ "text": "" }),
    };
    let parent = a.parent.as_ref().map(|p| yog_ids::qualify_in(ns, p));
    Ok(Shown {
        title,
        description,
        parent,
    })
}

/// Which of the game's advancement formats: items as `{"item": …}` (before
/// 1.20.5's item components) or as `{"id": …}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Era {
    Legacy,
    Components,
}

/// A tab's background as a texture id (`minecraft:block/calcite`, the
/// format since 1.21.5; hosts of older versions make it a file again), a
/// texture file's (`minecraft:textures/block/calcite.png`) included.
pub(super) fn background(bg: &str) -> String {
    let (ns, path) = yog_ids::split(bg);
    yog_ids::texture_of_file(path).map_or_else(|| bg.to_owned(), |t| yog_ids::join(ns, t))
}

/// The advancement's JSON, in `era`'s format.
fn written(a: &AdvancementDef, shown: &Shown, era: Era) -> Result<Value, Diagnostic> {
    let old = era == Era::Legacy;
    let mut criteria = Map::new();
    let mut requirements = Vec::new();
    for (i, c) in a.criteria.iter().enumerate() {
        let name = format!("c{i}");
        criteria.insert(
            name.clone(),
            criterion(c, era)
                .ok_or_else(|| problem(CRITERIA, Some(i), "one of granted, has_item, used_item"))?,
        );
        requirements.push(json!([name]));
    }
    let mut display = json!({
        "icon": if old { json!({ ITEM: a.icon }) } else { json!({ "id": a.icon }) },
        "title": { "translate": shown.title },
        "description": shown.description,
        "frame": a.frame,
        "hidden": a.hidden,
        "show_toast": true,
        "announce_to_chat": true,
    });
    if let Some(bg) = &a.background {
        display[yog_ids::advancement_field::BACKGROUND] = Value::String(background(bg));
    }
    let mut v = json!({ yog_ids::advancement_field::DISPLAY: display, "criteria": criteria, "requirements": requirements });
    if let Some(parent) = &shown.parent {
        v["parent"] = Value::String(parent.clone());
    }
    Ok(v)
}

/// One way to earn it, as the game's criterion; none unless it's exactly one way.
fn criterion(c: &CriterionDef, era: Era) -> Option<Value> {
    let old = era == Era::Legacy;
    let ways =
        u8::from(c.granted) + u8::from(c.has_item.is_some()) + u8::from(c.used_item.is_some());
    if ways != 1 {
        return None;
    }
    let item_predicate = |item: &str| {
        if old {
            json!({ "items": [item] })
        } else {
            json!({ "items": item })
        }
    };
    let game = |id: &str| yog_ids::qualify_in(MINECRAFT, id);
    Some(if c.granted {
        json!({ "trigger": adv::IMPOSSIBLE })
    } else if let Some(item) = &c.has_item {
        let predicate = match item {
            OneOrTag::One(i) => item_predicate(&game(&i.id)),
            OneOrTag::Tag(t) if old => json!({ TAG_FIELD: game(&t.id) }),
            OneOrTag::Tag(t) => json!({ "items": format!("{TAG}{}", game(&t.id)) }),
        };
        json!({ "trigger": adv::INVENTORY_CHANGED, "conditions": { "items": [predicate] } })
    } else {
        let item = c
            .used_item
            .as_ref()
            .map(|i| game(&i.id))
            .unwrap_or_default();
        json!({ "trigger": adv::CONSUME_ITEM, "conditions": { ITEM: item_predicate(&item) } })
    })
}
