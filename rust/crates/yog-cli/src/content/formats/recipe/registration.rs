//! One checked `.yrecipe`, and the code that registers it.

use yog_data::{Map, Value};

use crate::content::formats::defs::data::definition_path;
use crate::content::pipeline::source::{float, included};

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
        values: Vec<(String, Value)>,
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
    /// A recipe of the mod's own kind as the code hands it over: its file
    /// under `obj/generated/` and Yog's recipe in it, as Yog Data
    /// (the host keeps it out of the game's data). `None` for the game's kinds.
    pub(in crate::content) fn data_file(&self) -> Option<(String, Vec<u8>)> {
        let RecipeKind::Custom {
            kind,
            slots,
            values,
        } = &self.kind
        else {
            return None;
        };
        let recipe = self.custom_data(kind, slots, values);
        Some((self.data_path(), yog_data::binary::encode(&recipe)))
    }

    /// Where [`Recipe::data_file`] goes.
    fn data_path(&self) -> String {
        definition_path(yog_ids::kind::RECIPE, &self.id)
    }

    /// A recipe of the mod's own kind: Yog's recipe.
    fn custom_data(
        &self,
        kind: &str,
        slots: &[(String, Vec<Vec<String>>)],
        values: &[(String, Value)],
    ) -> Value {
        use yog_data::{Schema, field};
        use yog_formats::recipe::{Common, Custom, TYPE_FIELD};
        let mut o = Map::new();
        o.insert(TYPE_FIELD, kind.into());
        let slots: Map = slots.iter().map(|(k, v)| (k.as_str(), v.write())).collect();
        o.insert(field!(Custom, slots), slots.into());
        if !values.is_empty() {
            let values: Map = values
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            o.insert(field!(Custom, values), values.into());
        }
        o.insert(field!(Common, result), self.result.as_str().into());
        if self.count != 1 {
            o.insert(field!(Common, count), i64::from(self.count).into());
        }
        if let Some(g) = &self.group {
            o.insert(field!(Common, group), g.as_str().into());
        }
        o.into()
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
            RecipeKind::Custom { .. } => {
                return format!(
                    "registry.add_recipe_data({:?}, {});",
                    self.id,
                    included(&self.data_path())
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
