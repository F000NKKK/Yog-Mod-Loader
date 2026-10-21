//! A block of Yog's own: its model, the ways it stands (rotations, an arm
//! towards each neighbour that connects, facing, its own properties) and
//! which state shows which model — the game's blockstate.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value, json};
use yog_data::Path as At;
use yog_data::field;
use yog_formats::content::BlockProps as Props;
use yog_formats::content::{BlockFile, BlockShape, Connect, Facing, Model, Rotation, States};
use yog_ids::block_state::{CEILING, FACE, FACING, FALSE, FLOOR, TRUE, WALL, WATERLOGGED};
use yog_ids::direction::{DOWN, EAST, NORTH, SOUTH, UP, WEST};
use yog_ids::model as m;
use yog_ids::sep::{PROPERTY_IS, PROPERTY_SEP};
use yog_ids::units::BLOCK_PIXELS;

use super::{Emit, Parts};
use crate::content::ctx::{Ctx, Loc};
use crate::content::entry::{BlockProps, Entry};
use crate::content::model::{model, turn, variant, with_render_type};

/// The four sides a block faces, and the six.
const HORIZONTAL: [&str; 4] = [NORTH, EAST, SOUTH, WEST];
const ALL_SIX: [&str; 6] = [NORTH, EAST, SOUTH, WEST, UP, DOWN];
/// What an attached block hangs on.
const FACES: [&str; 3] = [FLOOR, WALL, CEILING];
/// A yes/no property's values.
const YES_NO: [&str; 2] = [FALSE, TRUE];
/// The fewest values a property of the block's own has.
const MIN_VALUES: usize = 2;
/// The angles a model turns by.
const RIGHT_ANGLES: [u32; 4] = [0, 90, 180, 270];
/// The variant every state shows when there's one.
const EVERY_STATE: &str = "";

/// A block state rule: the property values it holds for, and the models it shows.
type StateRule = (Vec<(String, String)>, Vec<String>);

/// A block of Yog's own: the file's parts, its `block` and model.
pub(super) struct Own<'a> {
    pub(super) parts: &'a Parts,
    pub(super) block: &'a Props,
    pub(super) model: &'a Model,
    pub(super) render_layer: Option<&'a str>,
}

/// The block's model and states written; its model's id (the item's parent).
pub(super) fn compile(
    cx: &mut Ctx,
    loc: Loc,
    own: &Own,
    entry: &mut Entry,
    emit: &mut Emit,
) -> Option<String> {
    let main = main_model(cx, loc, own, entry, emit)?;
    let mut rotations = Vec::new();
    let mut arm = None;
    if let Some(s) = &own.parts.states {
        rotations = rotations_of(cx, loc, s, &main);
        if s.connect.is_some() {
            arm = connect(cx, loc, own, entry, emit);
        }
        facing_and_properties(cx, loc, s, &mut entry.block);
    }
    let states = all_properties(&entry.block);
    yes_no(cx, loc, own.block, &states, &mut entry.block);
    let rules = match &own.parts.states {
        Some(_) => variant_rules(cx, loc, own, &states, emit),
        None => Vec::new(),
    };
    let base = match rotations.len() {
        0 => variant(&main, 0.0, 0.0, false, None),
        1 => rotations.remove(0),
        _ => Value::Array(rotations),
    };
    let uvlock = own.parts.states.as_ref().is_some_and(|s| s.uvlock);
    let blockstate = match &arm {
        None if !states.is_empty() => every_state(&states, &rules, &main, uvlock),
        None => json!({ m::VARIANTS: { EVERY_STATE: base } }),
        Some((arm, uvlock)) => arms(base, arm, *uvlock),
    };
    emit.asset(cx, yog_ids::blockstate_file(&emit.p), &blockstate);
    Some(main)
}

/// The block's own model: its render layer, its bounds as the collision box
/// when `shape` is `auto`.
fn main_model(
    cx: &mut Ctx,
    loc: Loc,
    own: &Own,
    entry: &mut Entry,
    emit: &mut Emit,
) -> Option<String> {
    let mut built = model(
        cx,
        loc,
        &At::root().key(field!(BlockFile, model)),
        own.model,
        false,
    )?;
    with_render_type(&mut built.json, own.render_layer);
    if matches!(own.block.shape, None | Some(BlockShape::Auto)) {
        let full = f64::from(BLOCK_PIXELS);
        entry.block.shape = built
            .bounds
            .filter(|b| *b != [0.0, 0.0, 0.0, full, full, full])
            .map(|b| {
                [
                    b[0].max(0.0),
                    b[1].max(0.0),
                    b[2].max(0.0),
                    b[3].min(full),
                    b[4].min(full),
                    b[5].min(full),
                ]
            });
    }
    let path = format!("{}{}", yog_ids::res::BLOCK_DIR, emit.p);
    emit.asset(cx, yog_ids::model_file(&path), &built.json);
    Some(yog_ids::join(&emit.ns, &path))
}

/// The ways it stands: each a turn of its model, picked by weight.
fn rotations_of(cx: &mut Ctx, loc: Loc, s: &States, main: &str) -> Vec<Value> {
    let Some(rs) = &s.rotations else {
        return Vec::new();
    };
    let at = At::root()
        .key(field!(BlockFile, states))
        .key(field!(States, rotations));
    if rs.is_empty() {
        cx.err_at(
            loc,
            &at,
            "a non-empty list of { x: 0, y: 90, weight: 1, uvlock: false }",
        );
    }
    let mut out = Vec::new();
    for (i, r) in rs.iter().enumerate() {
        let here = at.index(i);
        let mut angle = |k: &str, v: Option<u32>| match v {
            None => 0.0,
            Some(n) if RIGHT_ANGLES.contains(&n) => f64::from(n),
            Some(_) => {
                cx.err_at(loc, &here.key(k), "must be 0, 90, 180 or 270");
                0.0
            }
        };
        let (x, y) = (
            angle(field!(Rotation, x), r.x),
            angle(field!(Rotation, y), r.y),
        );
        out.push(variant(main, x, y, r.uvlock, r.weight));
    }
    out
}

/// Arms towards the neighbours that connect: the arm's model; the block
/// connects (to its own kind when it names no groups).
fn connect(
    cx: &mut Ctx,
    loc: Loc,
    own: &Own,
    entry: &mut Entry,
    emit: &mut Emit,
) -> Option<(String, bool)> {
    let c = own.parts.states.as_ref()?.connect.as_ref()?;
    let at = At::root()
        .key(field!(BlockFile, states))
        .key(field!(States, connect))
        .key(field!(Connect, arm));
    let arm = model(cx, loc, &at, &c.arm, false).map(|am| {
        let mut json = am.json;
        with_render_type(&mut json, own.render_layer);
        let path = format!("{}{}{ARM}", yog_ids::res::BLOCK_DIR, emit.p);
        emit.asset(cx, yog_ids::model_file(&path), &json);
        (yog_ids::join(&emit.ns, &path), c.uvlock)
    });
    entry.block.connects = true;
    if entry.block.connect_groups.is_empty() {
        entry.block.connect_groups.push(entry.id.clone());
    }
    arm
}

/// What the arm's model's name adds to the block's.
const ARM: &str = "_arm";

/// Facing, attach, waterlogged and the block's own properties — which can't
/// go with rotations or an arm.
fn facing_and_properties(cx: &mut Ctx, loc: Loc, s: &States, b: &mut BlockProps) {
    let sat = At::root().key(field!(BlockFile, states));
    if s.facing.is_none() && !s.attach && !s.waterlogged && s.properties.0.is_empty() {
        if s.placement.is_some() || !s.models.0.is_empty() || !s.variants.0.is_empty() {
            cx.err_at(
                loc,
                &sat,
                "placement, models and variants need facing, attach, waterlogged or properties",
            );
        }
        return;
    }
    if s.rotations.is_some() || s.connect.is_some() {
        cx.err_at(
            loc,
            &sat,
            "facing, attach, waterlogged and properties can't go with rotations or connect",
        );
    }
    if s.attach {
        if s.facing == Some(Facing::All) {
            cx.err_at(
                loc,
                &sat.key(field!(States, facing)),
                "an attached block turns four ways — leave facing out",
            );
        }
        b.attach = true;
        b.facing = Some(Facing::Horizontal);
    } else {
        b.facing = s.facing;
    }
    if b.facing.is_some() {
        b.placement = s.placement.unwrap_or_default();
    } else if s.placement.is_some() {
        cx.err_at(
            loc,
            &sat.key(field!(States, placement)),
            "placement needs facing (or attach)",
        );
    }
    b.waterlogged = s.waterlogged;
    for (name, values) in &s.properties.0 {
        own_property(cx, loc, name, values);
        b.properties.push((name.clone(), values.clone()));
    }
}

/// A property of the block's own: named in lowercase, not one the game's
/// makes, two or more different values.
fn own_property(cx: &mut Ctx, loc: Loc, name: &str, values: &[String]) {
    let here = At::root()
        .key(field!(BlockFile, states))
        .key(field!(States, properties))
        .key(name);
    if !yog_ids::valid_name(name) {
        cx.err_at(
            loc,
            &here,
            format!("`{name}`: a property's name is lowercase letters, digits and _"),
        );
    } else if [FACING, FACE, WATERLOGGED].contains(&name) {
        cx.err_at(
            loc,
            &here,
            format!("`{name}` comes from facing, attach or waterlogged — name yours otherwise"),
        );
    }
    let mut seen = HashSet::new();
    if values.len() < MIN_VALUES
        || !values
            .iter()
            .all(|v| yog_ids::valid_name(v) && seen.insert(v.as_str()))
    {
        cx.err_at(
            loc,
            &here,
            "two or more different values, lowercase letters, digits and _",
        );
    }
}

/// A block's properties with every value — facing, face and waterlogged as
/// the host makes them — by name.
fn all_properties(b: &BlockProps) -> Vec<(String, Vec<String>)> {
    let words = |ws: &[&str]| ws.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    let mut all = Vec::new();
    match b.facing {
        Some(Facing::All) => all.push((FACING.to_string(), words(&ALL_SIX))),
        Some(Facing::Horizontal) => all.push((FACING.to_string(), words(&HORIZONTAL))),
        None => {}
    }
    if b.attach {
        all.push((FACE.to_string(), words(&FACES)));
    }
    if b.waterlogged {
        all.push((WATERLOGGED.to_string(), words(&YES_NO)));
    }
    all.extend(b.properties.iter().cloned());
    all.sort_by(|a, b| a.0.cmp(&b.0));
    all
}

/// The yes/no properties the host drives: kept on by redstone, read by a comparator.
fn yes_no(
    cx: &mut Ctx,
    loc: Loc,
    block: &Props,
    states: &[(String, Vec<String>)],
    b: &mut BlockProps,
) {
    let bat = At::root().key(field!(BlockFile, block));
    let is_yes_no = |name: &str| states.iter().any(|(n, v)| n == name && *v == YES_NO);
    let fields = [
        (field!(Props, powered), &block.powered, &mut b.powered),
        (
            field!(Props, comparator),
            &block.comparator,
            &mut b.comparator,
        ),
    ];
    for (key, given, slot) in fields {
        let Some(name) = given else { continue };
        if is_yes_no(name) {
            *slot = Some(name.clone());
        } else {
            cx.err_at(
                loc,
                &bat.key(key),
                format!("`{name}`: a yes/no property of states.properties ([\"false\", \"true\"])"),
            );
        }
    }
}

/// Its other models, and which state shows which.
fn variant_rules(
    cx: &mut Ctx,
    loc: Loc,
    own: &Own,
    states: &[(String, Vec<String>)],
    emit: &mut Emit,
) -> Vec<StateRule> {
    let Some(s) = &own.parts.states else {
        return Vec::new();
    };
    let sat = At::root().key(field!(BlockFile, states));
    let mut named = HashMap::new();
    for (name, model_of) in &s.models.0 {
        if let Some(mut built) = model(
            cx,
            loc,
            &sat.key(field!(States, models)).key(name),
            model_of,
            false,
        ) {
            with_render_type(&mut built.json, own.render_layer);
            let path = format!(
                "{}{}{}{name}",
                yog_ids::res::BLOCK_DIR,
                emit.p,
                yog_ids::sep::PATH
            );
            emit.asset(cx, yog_ids::model_file(&path), &built.json);
            named.insert(name.clone(), yog_ids::join(&emit.ns, &path));
        }
    }
    let mut rules = Vec::new();
    for (key, names) in &s.variants.0 {
        let here = sat.key(field!(States, variants)).key(key);
        let mut when = Vec::new();
        for pair in key.split(PROPERTY_SEP).filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once(PROPERTY_IS).unwrap_or((pair, ""));
            if states
                .iter()
                .any(|(n, vs)| n == k && vs.iter().any(|x| x == v))
            {
                when.push((k.to_string(), v.to_string()));
            } else {
                cx.err_at(loc, &here, format!("`{pair}`: no such property and value"));
            }
        }
        let mut models = Vec::new();
        for name in &names.0 {
            match named.get(name) {
                Some(m) => models.push(m.clone()),
                None => cx.err_at(loc, &here, format!("`{name}`: not in states.models")),
            }
        }
        if names.0.is_empty() {
            cx.err_at(loc, &here, "a model name, or a list of them");
        } else if !models.is_empty() {
            rules.push((when, models));
        }
    }
    rules
}

/// Every state: its model (the first rule that holds, else the block's
/// own), turned by facing and face.
fn every_state(
    states: &[(String, Vec<String>)],
    rules: &[StateRule],
    main: &str,
    uvlock: bool,
) -> Value {
    let mut variants = Map::new();
    let mut combo: Vec<usize> = vec![0; states.len()];
    loop {
        let at: Vec<(&str, &str)> = states
            .iter()
            .zip(&combo)
            .map(|((n, vs), &i)| (n.as_str(), vs[i].as_str()))
            .collect();
        let value = |k: &str| at.iter().find(|(n, _)| *n == k).map(|(_, v)| *v);
        let models = rules
            .iter()
            .find(|(when, _)| when.iter().all(|(k, v)| value(k) == Some(v.as_str())))
            .map_or(vec![main.to_string()], |(_, m)| m.clone());
        let (x, y) = turn(value(FACING), value(FACE));
        let key: Vec<String> = at
            .iter()
            .map(|(n, v)| format!("{n}{PROPERTY_IS}{v}"))
            .collect();
        let mut picks: Vec<Value> = models
            .iter()
            .map(|m| variant(m, x, y, uvlock, None))
            .collect();
        let v = if picks.len() == 1 {
            picks.remove(0)
        } else {
            Value::Array(picks)
        };
        variants.insert(key.join(PROPERTY_SEP), v);
        if !next_state(&mut combo, states) {
            break;
        }
    }
    json!({ m::VARIANTS: variants })
}

/// `combo` to the next state, the last property fastest; false after the last.
fn next_state(combo: &mut [usize], states: &[(String, Vec<String>)]) -> bool {
    for i in (0..combo.len()).rev() {
        combo[i] += 1;
        if combo[i] < states[i].1.len() {
            return true;
        }
        combo[i] = 0;
    }
    false
}

/// The block's model always, and the arm towards each side that connects,
/// turned to face it.
fn arms(base: Value, arm: &str, uvlock: bool) -> Value {
    let mut parts = vec![json!({ m::APPLY: base })];
    for dir in ALL_SIX {
        let (x, y) = turn(Some(dir), None);
        parts.push(json!({ m::WHEN: { dir: TRUE }, m::APPLY: variant(arm, x, y, uvlock, None) }));
    }
    json!({ m::MULTIPART: parts })
}
