//! Where a recipe can't be: the versions and loaders without something it names.

use std::collections::{BTreeMap, BTreeSet};

use yog_data::Ref;
use yog_data::schema::Found;

use crate::content::refs::{self, RefKind};

/// A reference in a recipe list, and where it is (`None`: everywhere).
type PlacedRef = (String, Option<BTreeSet<String>>);

/// The hosts (`<loader>/<version>`) a recipe naming `items` and `tags` can't
/// be on, with why: an item or tag one hasn't got. Alternatives (`["a",
/// "#b"]`, one list in the file) count as there when any of them is.
pub(super) fn recipe_gaps(
    items: &[Found<Ref>],
    tags: &[Found<yog_data::found::TagRef>],
) -> Vec<(BTreeSet<String>, String)> {
    // Where each reference is (`None`: everywhere), grouped by the list it's in.
    let mut lists: BTreeMap<String, Vec<PlacedRef>> = BTreeMap::new();
    for f in items {
        if let Some(on) = item_hosts(f) {
            lists
                .entry(list_of(&f.path))
                .or_default()
                .push((f.value.id.to_string(), on));
        }
    }
    for f in tags {
        let Some(on) = refs::tag_hosts(yog_ids::registry_of(&f.value.kind), &f.value.id) else {
            continue;
        };
        let shown = format!("{}{}", yog_ids::sep::TAG_SHOWN, f.value.id);
        lists.entry(list_of(&f.path)).or_default().push((shown, on));
    }
    lists.values().filter_map(|alts| gap(alts)).collect()
}

/// The list a reference is in: its path less a list index.
fn list_of(p: &yog_data::Path) -> String {
    let mut p = p.clone();
    if matches!(p.segments().last(), Some(yog_data::Seg::Index(_))) {
        p.pop();
    }
    p.to_string()
}

/// Where the game's item (or Yog's alias of a game potion) `f` is — `None`
/// inside: everywhere; none at all for anything else.
fn item_hosts(f: &Found<Ref>) -> Option<Option<BTreeSet<String>>> {
    let kind = RefKind::from_word(&f.value.kind)?;
    let id = &f.value.id;
    let potion_alias = kind == RefKind::Item && id.starts_with(yog_ids::yog::POTION_ALIAS);
    if !potion_alias && yog_ids::game_path(id).is_none() {
        return None;
    }
    let on: BTreeSet<String> = refs::hosts()
        .iter()
        .filter(|h| {
            yog_formats::tags::host_version(h).is_some_and(|v| {
                if potion_alias {
                    refs::is_vanilla_potion_in(id, v)
                } else {
                    refs::equivalent(kind, id, v).is_some()
                }
            })
        })
        .cloned()
        .collect();
    Some((on.len() < refs::hosts().len()).then_some(on))
}

/// The hosts one list of alternatives misses, and why — with a Yog tag
/// that's everywhere, when one of them is it.
fn gap(alts: &[PlacedRef]) -> Option<(BTreeSet<String>, String)> {
    if alts.iter().any(|(_, on)| on.is_none()) {
        return None;
    }
    let on: BTreeSet<String> = alts
        .iter()
        .flat_map(|(_, on)| on.iter().flatten().cloned())
        .collect();
    let missing: BTreeSet<String> = refs::hosts()
        .iter()
        .filter(|h| !on.contains(*h))
        .cloned()
        .collect();
    if missing.is_empty() {
        return None;
    }
    let names: Vec<&str> = alts.iter().map(|(id, _)| id.as_str()).collect();
    let item = yog_ids::kind::ITEM;
    let hint = alts
        .iter()
        .filter_map(|(id, _)| {
            id.strip_prefix(yog_ids::sep::TAG_SHOWN)
                .and_then(|t| refs::yog_tag_for(item, t))
        })
        .find(|y| refs::tag_hosts(item, y) == Some(None))
        .map(|y| {
            format!(
                " — {}{item}:{y} is on every version and loader",
                yog_ids::reference::TAG
            )
        })
        .unwrap_or_default();
    Some((missing, format!("{} isn't there{hint}", names.join(" / "))))
}
