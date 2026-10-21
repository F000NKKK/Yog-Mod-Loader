//! The mod's own kind of recipe (`mymod:infusion`, a `.ydef` recipe_kind):
//! its slots and values as its kind declares them.

use yog_data::Path as At;
use yog_data::field;
use yog_formats::def::{RecipeKindDef, ValueType};
use yog_formats::recipe::{Custom, TYPE_FIELD, TYPES};

use super::Against;
use crate::content::ctx::{Ctx, Loc};
use crate::content::recipe::registration::RecipeKind;
use crate::content::valid_path;

/// A recipe of kind `kind`: the mod's own (`<mod id>:<kind>`), declared.
pub(super) fn custom(
    cx: &mut Ctx,
    loc: Loc,
    kind: String,
    k: &Custom,
    against: &Against,
) -> Option<RecipeKind> {
    let at = At::root().key(TYPE_FIELD);
    let (kind_ns, kind_path) = yog_ids::split(&kind);
    let own = kind.contains(yog_ids::sep::NAMESPACE)
        && against.mod_ns == Some(kind_ns)
        && valid_path(kind_path);
    if !own {
        let mod_ns = against.mod_ns.unwrap_or("mymod");
        cx.err(
            loc(&at),
            format!(
                "type `{kind}`: one of {}, or the mod's own kind `{mod_ns}:<kind>`",
                TYPES.join(", ")
            ),
        );
        return None;
    }
    let Some(def) = against.kinds.get(&kind) else {
        cx.err(loc(&at), format!("type `{kind}`: no such kind — declare it: content/{kind_path}.ydef, {{ kind: \"recipe_kind\", id: \"{kind_path}\", slots: {{ … }} }}"));
        return None;
    };
    check_slots(cx, loc, &kind, k, def);
    check_values(cx, loc, &kind, k, def);
    let slots = k
        .slots
        .0
        .iter()
        .map(|(name, ings)| {
            (
                name.clone(),
                ings.iter().map(yog_formats::recipe::ids).collect(),
            )
        })
        .collect();
    let values = k
        .values
        .0
        .iter()
        .map(|(name, v)| (name.clone(), v.clone()))
        .collect();
    Some(RecipeKind::Custom {
        kind,
        slots,
        values,
    })
}

/// Each slot one its kind has, within its count; the kind's required ones given.
fn check_slots(cx: &mut Ctx, loc: Loc, kind: &str, k: &Custom, def: &RecipeKindDef) {
    let slots = At::root().key(field!(Custom, slots));
    for (name, ings) in &k.slots.0 {
        let at = slots.key(name);
        match def.slots.0.iter().find(|(n, _)| n == name) {
            None => {
                let names: Vec<&str> = def.slots.0.iter().map(|(n, _)| n.as_str()).collect();
                cx.err(
                    loc(&at),
                    format!(
                        "slots.{name}: {kind} has no slot `{name}` ({})",
                        names.join(", ")
                    ),
                );
            }
            Some((_, spec)) if ings.len() > spec.max as usize => {
                cx.err(
                    loc(&at),
                    format!(
                        "slots.{name}: at most {} ingredient(s), found {}",
                        spec.max,
                        ings.len()
                    ),
                );
            }
            _ => {}
        }
    }
    for (name, spec) in &def.slots.0 {
        if spec.required
            && !k
                .slots
                .0
                .iter()
                .any(|(n, ings)| n == name && !ings.is_empty())
        {
            cx.err(loc(&slots), format!("slots: {kind} needs `{name}`"));
        }
    }
}

/// Each value one its kind has, of its type.
fn check_values(cx: &mut Ctx, loc: Loc, kind: &str, k: &Custom, def: &RecipeKindDef) {
    let values = At::root().key(field!(Custom, values));
    for (name, v) in &k.values.0 {
        let at = values.key(name);
        let Some(ty) = def
            .values
            .0
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| *t)
        else {
            let names: Vec<&str> = def.values.0.iter().map(|(n, _)| n.as_str()).collect();
            cx.err(
                loc(&at),
                format!(
                    "values.{name}: {kind} has no value `{name}` ({})",
                    names.join(", ")
                ),
            );
            continue;
        };
        let fits = match ty {
            ValueType::Number => v.as_f64().is_some(),
            ValueType::Bool => v.as_bool().is_some(),
            ValueType::Text => v.as_str().is_some(),
        };
        if !fits {
            cx.err(
                loc(&at),
                format!("values.{name}: a {}", yog_formats::content::word(&ty)),
            );
        }
    }
}
