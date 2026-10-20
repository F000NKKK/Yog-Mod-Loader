//! Recipes as a book shows them: read from Yog's recipe format (the ids
//! resolved, as `yog_registry::Recipe` has them) into the slots and values
//! a layout ([`yog_formats::book::layout`]) draws.

use std::collections::BTreeMap;

use yog_data::Value;
use yog_registry::recipe_kinds::{self, GRID_SIDE, fields};

/// A recipe as a book shows it: its kind (which layout draws it), each
/// slot's ingredients (each one's alternatives; empty: none), result and values.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RecipeVis {
    pub kind: String,
    pub slots: BTreeMap<String, Vec<Vec<String>>>,
    pub output: String,
    pub count: u32,
    /// `time`, `experience`, a mod kind's `values` — for a layout's texts.
    pub values: BTreeMap<String, String>,
}

/// An ingredient: an id, a tag (shown `#ns:path`), or a list of those (any of them).
fn ingredient(v: &Value) -> Vec<String> {
    match v {
        Value::Str(s) => vec![s.clone()],
        Value::Ref(r) => vec![r.id.clone()],
        Value::Tag(r) => vec![format!("{}{}", yog_ids::sep::TAG_SHOWN, r.id)],
        Value::List(a) => a.iter().flat_map(ingredient).collect(),
        _ => Vec::new(),
    }
}

/// A value as a layout's text shows it.
fn text(x: &Value) -> String {
    match x {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        x => x.as_str().map(str::to_owned).unwrap_or_default(),
    }
}

/// A recipe in Yog's own format, as the book shows it.
pub(crate) fn parse_recipe(v: &Value) -> Option<RecipeVis> {
    let kind = v.get(fields::TYPE)?.as_str()?.to_owned();
    let output = match v.get(fields::RESULT)? {
        Value::Ref(r) => r.id.clone(),
        r => r.as_str()?.to_owned(),
    };
    let count = v
        .get(fields::COUNT)
        .and_then(|c| c.as_i64())
        .unwrap_or(1)
        .max(0) as u32;
    let (slots, values) = match recipe_kinds::game_kind(&kind) {
        Some(def) => game_kind(v, def)?,
        None => mod_kind(v),
    };
    Some(RecipeVis {
        kind,
        slots,
        output,
        count,
        values,
    })
}

type Slots = BTreeMap<String, Vec<Vec<String>>>;
type Values = BTreeMap<String, String>;

/// The game's kind: its slots where its definition says they're written,
/// its values (or their defaults). `None`: a required slot is missing.
fn game_kind(v: &Value, def: &recipe_kinds::RecipeKindDef) -> Option<(Slots, Values)> {
    let mut slots = BTreeMap::new();
    for (name, spec) in &def.slots.0 {
        let places = if spec.grid {
            grid(v)?
        } else {
            match (v.get(spec.field.as_deref()?), spec.max) {
                // Several places: a list of them.
                (Some(list), max) if max > 1 => list.as_list()?.iter().map(ingredient).collect(),
                // One place: an ingredient (or a list of alternatives).
                (Some(x), _) => vec![ingredient(x)],
                (None, _) if spec.required => return None,
                (None, _) => vec![Vec::new()],
            }
        };
        slots.insert(name.clone(), places);
    }
    let mut values = BTreeMap::new();
    for (name, _) in &def.values.0 {
        if let Some(x) = v
            .get(name)
            .map(text)
            .or_else(|| def.default_of(name).map(|d| d.to_string()))
        {
            values.insert(name.clone(), x);
        }
    }
    Some((slots, values))
}

/// A shaped recipe's cells by position on the grid: its pattern's keys looked up.
fn grid(v: &Value) -> Option<Vec<Vec<String>>> {
    let key = v.get(fields::KEY)?.as_map()?;
    let mut grid = vec![Vec::new(); GRID_SIDE * GRID_SIDE];
    for (r, row) in v
        .get(fields::PATTERN)?
        .as_list()?
        .iter()
        .filter_map(|p| p.as_str())
        .take(GRID_SIDE)
        .enumerate()
    {
        let mut buf = [0u8; 4];
        for (c, ch) in row.chars().take(GRID_SIDE).enumerate() {
            grid[r * GRID_SIDE + c] = key
                .get(ch.encode_utf8(&mut buf))
                .map(ingredient)
                .unwrap_or_default();
        }
    }
    Some(grid)
}

/// A mod's own kind: named slots, values.
fn mod_kind(v: &Value) -> (Slots, Values) {
    let map = |field| {
        v.get(field)
            .and_then(|s| s.as_map())
            .into_iter()
            .flat_map(|m| m.iter())
    };
    let slots = map(fields::SLOTS)
        .map(|(name, list)| {
            (
                name.to_string(),
                list.as_list()
                    .into_iter()
                    .flatten()
                    .map(ingredient)
                    .collect(),
            )
        })
        .collect();
    let values = map(fields::VALUES)
        .map(|(name, x)| (name.to_string(), text(x)))
        .collect();
    (slots, values)
}

/// An icon's id as the item renderer takes it: `yog:item/ruby` → `yog:ruby`.
pub(crate) fn normalize_item_id(id: &str) -> String {
    let (ns, path) = yog_ids::split(id);
    let name = path
        .trim_start_matches(yog_ids::res::ITEM_DIR)
        .trim_start_matches(yog_ids::res::BLOCK_DIR);
    yog_ids::join(ns, name)
}
