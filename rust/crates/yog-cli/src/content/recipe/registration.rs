//! One checked `.yrecipe`, and the code that registers it.

use serde_json::{Map, Value, json};

use crate::content::source::float;

/// What a recipe takes.
#[derive(Debug, Clone)]
pub(in crate::content) enum RecipeKind {
    /// A crafting grid pattern: rows of key characters (space = empty);
    /// each key any of several items.
    Shaped {
        pattern: Vec<String>,
        key: Vec<(char, Vec<String>)>,
    },
    /// Crafting ingredients in any arrangement.
    Shapeless(Vec<Vec<String>>),
    Cooking {
        method: yog_registry::Cooking,
        input: Vec<String>,
        experience: f64,
        cook_time: u32,
    },
    Stonecutting(Vec<String>),
    Smithing {
        template: Vec<String>,
        base: Vec<String>,
        addition: Vec<String>,
    },
    /// The mod's own kind of recipe (`mymod:infusion`), for its machines
    /// and books — not the game's: named slots, each a list of ingredients
    /// (an ingredient any of several items), and values (`time`, `energy`…).
    Custom {
        kind: String,
        slots: Vec<(String, Vec<Vec<String>>)>,
        values: serde_json::Map<String, Value>,
    },
}

/// One checked `.yrecipe`.
#[derive(Debug, Clone)]
pub(in crate::content) struct Recipe {
    pub(in crate::content) id: String,
    /// Its `type`: `shaped`, `smelting`, …, or the mod's own kind.
    pub(in crate::content) ty: String,
    pub(in crate::content) kind: RecipeKind,
    pub(in crate::content) result: String,
    pub(in crate::content) count: u32,
    pub(in crate::content) group: Option<String>,
    pub(in crate::content) category: Option<String>,
    /// Mods it needs (`requires`): registered only when they're all loaded.
    pub(in crate::content) requires: Vec<String>,
}

impl Recipe {
    /// A recipe of the mod's own kind: Yog's recipe JSON as is (the host
    /// keeps it out of the game's data).
    fn custom_json(
        &self,
        kind: &str,
        slots: &[(String, Vec<Vec<String>>)],
        values: &Map<String, Value>,
    ) -> Value {
        use yog_data::field;
        use yog_formats::recipe::{Common, Custom, TYPE_FIELD};
        let mut o = Map::new();
        o.insert(TYPE_FIELD.into(), kind.into());
        o.insert(
            field!(Custom, slots).into(),
            Value::Object(slots.iter().map(|(k, v)| (k.clone(), json!(v))).collect()),
        );
        if !values.is_empty() {
            o.insert(field!(Custom, values).into(), Value::Object(values.clone()));
        }
        o.insert(field!(Common, result).into(), self.result.clone().into());
        if self.count != 1 {
            o.insert(field!(Common, count).into(), self.count.into());
        }
        if let Some(g) = &self.group {
            o.insert(field!(Common, group).into(), g.clone().into());
        }
        Value::Object(o)
    }

    /// The `registry.add_recipe(...)` call registering it.
    pub(in crate::content) fn registration(&self) -> String {
        let list = |items: &[String]| {
            let quoted: Vec<String> = items.iter().map(|i| format!("{i:?}.into()")).collect();
            format!("vec![{}]", quoted.join(", "))
        };
        let kind = match &self.kind {
            RecipeKind::Shaped { pattern, key } => {
                let rows: Vec<String> = pattern.iter().map(|r| format!("{r:?}.into()")).collect();
                let keys: Vec<String> = key
                    .iter()
                    .map(|(c, items)| format!("({c:?}, {})", list(items)))
                    .collect();
                format!(
                    "Shaped {{ pattern: vec![{}], key: vec![{}] }}",
                    rows.join(", "),
                    keys.join(", ")
                )
            }
            RecipeKind::Shapeless(ingredients) => {
                let all: Vec<String> = ingredients.iter().map(|i| list(i)).collect();
                format!("Shapeless {{ ingredients: vec![{}] }}", all.join(", "))
            }
            RecipeKind::Cooking {
                method,
                input,
                experience,
                cook_time,
            } => format!(
                "Cooking {{ method: ::yog_api::Cooking::{method:?}, ingredient: {}, experience: {}_f32, cook_time: {cook_time} }}",
                list(input),
                float(*experience)
            ),
            RecipeKind::Stonecutting(input) => {
                format!("Stonecutting {{ ingredient: {} }}", list(input))
            }
            RecipeKind::Custom {
                kind,
                slots,
                values,
            } => {
                return format!(
                    "registry.add_recipe_json({:?}, {:?});",
                    self.id,
                    self.custom_json(kind, slots, values).to_string()
                );
            }
            RecipeKind::Smithing {
                template,
                base,
                addition,
            } => format!(
                "Smithing {{ template: {}, base: {}, addition: {} }}",
                list(template),
                list(base),
                list(addition)
            ),
        };
        let opt = |o: &Option<String>| {
            o.as_ref()
                .map_or("None".to_string(), |v| format!("Some({v:?}.into())"))
        };
        format!(
            "registry.add_recipe(::yog_api::Recipe {{ id: {:?}.into(), kind: ::yog_api::RecipeKind::{kind}, result: {:?}.into(), count: {}, group: {}, category: {} }});",
            self.id,
            self.result,
            self.count,
            opt(&self.group),
            opt(&self.category)
        )
    }
}
