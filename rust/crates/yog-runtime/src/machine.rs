//! A machine's recipe (`yog_inventory::Machine`, a `.yui` `<RecipeArea>`):
//! the first recipe of its kind whose every slot's ingredients are what's
//! in that slot's places — in any order within the slot, the places left
//! over empty.

use yog_data::Value;
use yog_inventory::Machine;

/// A recipe that fits: its id, what it makes, and its values (JSON).
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub recipe: String,
    pub result: String,
    pub count: u32,
    pub values: String,
}

/// Every recipe of `m`'s kind that fits `items` (by inventory slot, `""`
/// empty), in order; `has_tag(item, tag)` answers `#tag` ingredients.
pub fn find<'r>(
    m: &'r Machine,
    recipes: impl IntoIterator<Item = (String, &'r Value)> + 'r,
    items: &'r [String],
    has_tag: &'r dyn Fn(&str, &str) -> bool,
) -> impl Iterator<Item = Found> + 'r {
    recipes.into_iter().filter_map(move |(id, r)| {
        if r.get("type")?.as_str()? != m.kind {
            return None;
        }
        let slots = r.get("slots").and_then(Value::as_map);
        // Every slot of the kind the machine has places for, and every slot the recipe fills.
        let mut names: Vec<&str> = m.places.iter().map(|p| p.slot.as_str()).collect();
        names.extend(slots.iter().flat_map(|s| s.keys()));
        names.sort_unstable();
        names.dedup();
        for name in names {
            let wants: Vec<Vec<&str>> = slots
                .and_then(|s| s.get(name))
                .and_then(Value::as_list)
                .unwrap_or_default()
                .iter()
                .map(|alts| alts.as_list().unwrap_or_default().iter().filter_map(Value::as_str).collect())
                .collect();
            let have: Vec<&str> = m
                .places
                .iter()
                .filter(|p| p.slot == name)
                .map(|p| items.get(p.inv).map_or("", String::as_str))
                .filter(|i| !i.is_empty())
                .collect();
            if !fits(&wants, &have, has_tag) {
                return None;
            }
        }
        Some(Found {
            recipe: id,
            result: r.get("result")?.as_str()?.to_string(),
            count: r.get("count").and_then(Value::as_i64).unwrap_or(1).max(1) as u32,
            values: r.get("values").map_or_else(|| "{}".to_string(), |v| yog_data::json::to_json(v).to_string()),
        })
    })
}

/// Whether `have` is `wants` one for one, in some order.
fn fits(wants: &[Vec<&str>], have: &[&str], has_tag: &dyn Fn(&str, &str) -> bool) -> bool {
    fn takes(alts: &[&str], item: &str, has_tag: &dyn Fn(&str, &str) -> bool) -> bool {
        alts.iter().any(|a| match a.strip_prefix('#') {
            Some(tag) => has_tag(item, tag),
            None => *a == item,
        })
    }
    fn assign(wants: &[Vec<&str>], have: &[&str], used: &mut [bool], has_tag: &dyn Fn(&str, &str) -> bool) -> bool {
        let Some((first, rest)) = wants.split_first() else { return true };
        for (i, item) in have.iter().enumerate() {
            if !used[i] && takes(first, item, has_tag) {
                used[i] = true;
                if assign(rest, have, used, has_tag) {
                    return true;
                }
                used[i] = false;
            }
        }
        false
    }
    wants.len() == have.len() && assign(wants, have, &mut vec![false; have.len()], has_tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_inventory::MachinePlace;

    fn recipe(json: &str) -> Value {
        yog_data::json::from_json_text(json).unwrap()
    }

    #[test]
    fn the_first_recipe_whose_slots_hold_their_ingredients() {
        let m = Machine {
            kind: "m:press".into(),
            places: vec![
                MachinePlace { slot: "input".into(), index: 0, inv: 2 },
                MachinePlace { slot: "input".into(), index: 1, inv: 3 },
                MachinePlace { slot: "die".into(), index: 0, inv: 4 },
            ],
            result: 5,
        };
        let plate = recipe(r##"{"type":"m:press","slots":{"input":[["minecraft:iron_ingot"],["#c:ingots"]],"die":[["m:flat_die"]]},"values":{"energy":4},"result":"m:plate","count":2}"##);
        let other = recipe(r#"{"type":"m:other","slots":{},"result":"m:x"}"#);
        let recipes = [("m:plate".to_string(), &plate), ("m:x".to_string(), &other)];
        let tag = |item: &str, tag: &str| tag == "c:ingots" && item.ends_with("_ingot");
        let items = |v: [&str; 5]| -> Vec<String> { v.iter().map(|s| s.to_string()).collect() };

        let ok = items(["", "", "minecraft:gold_ingot", "minecraft:iron_ingot", "m:flat_die"]);
        let found: Vec<Found> = find(&m, recipes.clone(), &ok, &tag).collect();
        assert_eq!(found, [Found { recipe: "m:plate".into(), result: "m:plate".into(), count: 2, values: r#"{"energy":4}"#.into() }]);

        let no_die = items(["", "", "minecraft:gold_ingot", "minecraft:iron_ingot", ""]);
        assert_eq!(find(&m, recipes.clone(), &no_die, &tag).count(), 0, "a slot short");
        let one = items(["", "", "", "minecraft:iron_ingot", "m:flat_die"]);
        assert_eq!(find(&m, recipes.clone(), &one, &tag).count(), 0, "an ingredient short");
        let wrong = items(["", "", "minecraft:stick", "minecraft:iron_ingot", "m:flat_die"]);
        assert_eq!(find(&m, recipes, &wrong, &tag).count(), 0, "not an ingot");
    }
}
