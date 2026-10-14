//! What the hosts register — items, blocks, inventories, recipes,
//! dimensions — as one Yog Data
//! blob (`yog_data::binary`), read by `dev.yog.data.YogContent` in
//! java-common: one call and one format instead of a TSV dialect per kind.

use yog_data::{Map, Value};

use crate::RuntimeHandlers;

fn put(m: &mut Map, key: &str, v: impl Into<Value>) {
    m.insert(key, v.into());
}

fn float(f: f32) -> Value {
    Value::Float(f as f64)
}

fn floats(fs: &[f32]) -> Value {
    Value::List(fs.iter().map(|f| float(*f)).collect())
}

fn strings(ss: &[String]) -> Value {
    Value::List(ss.iter().map(|s| Value::from(s.as_str())).collect())
}

/// A JSON value (recipes and dimensions still arrive from mods as JSON) as Yog Data.
fn from_json(j: &serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => n.as_i64().map(Value::Int).unwrap_or_else(|| Value::Float(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(a) => Value::List(a.iter().map(from_json).collect()),
        serde_json::Value::Object(o) => Value::Map(o.iter().map(|(k, v)| (k.clone(), from_json(v))).collect()),
    }
}

fn parsed(json: &str) -> Value {
    serde_json::from_str(json).map(|j| from_json(&j)).unwrap_or(Value::Null)
}

/// The registered content: `{ items, blocks, inventories, recipes, dimensions }`.
pub fn snapshot(h: &RuntimeHandlers) -> Value {
    let items = h
        .items
        .iter()
        .map(|d| {
            let mut m = Map::new();
            put(&mut m, "id", d.id.as_str());
            put(&mut m, "max_stack", d.max_stack as i64);
            if let Some(n) = &d.name {
                put(&mut m, "name", n.as_str());
            }
            if let Some(t) = &d.tooltip {
                put(&mut m, "tooltip", t.as_str());
            }
            put(&mut m, "max_damage", d.max_damage as i64);
            put(&mut m, "fire_resistant", d.fire_resistant);
            put(&mut m, "fuel_ticks", d.fuel_ticks as i64);
            if let Some(f) = &d.food {
                let mut food = Map::new();
                put(&mut food, "nutrition", f.nutrition as i64);
                food.insert("saturation", float(f.saturation));
                put(&mut food, "always_edible", f.can_always_eat);
                m.insert("food", Value::Map(food));
            }
            Value::Map(m)
        })
        .collect();
    let blocks = h
        .blocks
        .iter()
        .map(|d| {
            let mut m = Map::new();
            put(&mut m, "id", d.id.as_str());
            m.insert("hardness", float(d.hardness));
            m.insert("resistance", float(d.resistance));
            if let Some(n) = &d.name {
                put(&mut m, "name", n.as_str());
            }
            if let Some(sh) = &d.shape {
                m.insert("shape", floats(sh));
            }
            put(&mut m, "light", d.light_level as i64);
            if let Some(s) = &d.sound {
                put(&mut m, "sound", s.as_str());
            }
            put(&mut m, "requires_tool", d.requires_tool);
            put(&mut m, "no_collision", d.no_collision);
            m.insert("slipperiness", float(d.slipperiness));
            put(&mut m, "connects", d.connects);
            m.insert("connect_groups", strings(&d.connect_groups));
            if let Some(inv) = &d.inventory_id {
                put(&mut m, "inventory", inv.as_str());
            }
            put(&mut m, "redstone", d.redstone_source);
            Value::Map(m)
        })
        .collect();
    let inventories = h
        .inventories
        .iter()
        .map(|d| {
            let mut m = Map::new();
            put(&mut m, "id", d.id.as_str());
            put(&mut m, "slot_count", d.slot_count as i64);
            let slots = d
                .resolved_layout()
                .iter()
                .map(|s| {
                    let mut slot = Map::new();
                    slot.insert("at", floats(&[s.x, s.y]));
                    slot.insert("accepts", strings(&s.accepts));
                    Value::Map(slot)
                })
                .collect();
            m.insert("slots", Value::List(slots));
            put(&mut m, "include_player_inventory", d.include_player_inventory);
            m.insert("player_inv", floats(&[d.player_inv_offset.0, d.player_inv_offset.1]));
            if let Some(t) = &d.background_texture {
                put(&mut m, "background_texture", t.as_str());
            }
            put(&mut m, "title", d.title.as_str());
            if let Some((w, hh)) = d.background_size {
                m.insert("background_size", floats(&[w, hh]));
            }
            Value::Map(m)
        })
        .collect();
    let recipes = h
        .recipes
        .iter()
        .map(|(ns, name, json)| {
            let mut m = Map::new();
            put(&mut m, "namespace", ns.as_str());
            put(&mut m, "name", name.as_str());
            m.insert("recipe", parsed(json));
            Value::Map(m)
        })
        .collect();
    let dimensions = h
        .dimensions
        .iter()
        .map(|(id, json)| {
            let mut m = Map::new();
            put(&mut m, "id", id.as_str());
            m.insert("def", parsed(json));
            Value::Map(m)
        })
        .collect();
    let mut root = Map::new();
    root.insert("items", Value::List(items));
    root.insert("blocks", Value::List(blocks));
    root.insert("inventories", Value::List(inventories));
    root.insert("recipes", Value::List(recipes));
    root.insert("dimensions", Value::List(dimensions));
    Value::Map(root)
}
