//! Recipes: shaped, shapeless, cooking and the rest, as the game reads them; startup grants.

use crate::crafting::recipe_kinds;

/// How a cooking recipe cooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cooking {
    Smelting,
    Blasting,
    Smoking,
    Campfire,
}

impl Cooking {
    /// Every one, in [`recipe_kinds::ids::COOKING`]'s order.
    pub const ALL: [Cooking; 4] = [
        Cooking::Smelting,
        Cooking::Blasting,
        Cooking::Smoking,
        Cooking::Campfire,
    ];

    /// Its kind of recipe (`smelting`).
    pub fn word(self) -> &'static str {
        recipe_kinds::ids::COOKING[self as usize]
    }

    /// The cooking of kind `id`, if `id` is one.
    pub fn of_kind(id: &str) -> Option<Cooking> {
        Cooking::ALL.into_iter().find(|c| c.word() == id)
    }
}

/// What a recipe is made of besides its result.
#[derive(Debug, Clone, PartialEq)]
pub enum RecipeKind {
    /// A crafting grid: rows of key characters (space = empty).
    Shaped {
        pattern: Vec<String>,
        key: Vec<(char, Vec<String>)>,
    },
    /// Crafting, in any arrangement.
    Shapeless {
        ingredients: Vec<Vec<String>>,
    },
    Cooking {
        method: Cooking,
        ingredient: Vec<String>,
        experience: f32,
        cook_time: u32,
    },
    Stonecutting {
        ingredient: Vec<String>,
    },
    /// A smithing table: `base` upgraded with `addition`, `template` used up.
    Smithing {
        template: Vec<String>,
        base: Vec<String>,
        addition: Vec<String>,
    },
}

/// A recipe, in Yog's own format — what a `.yrecipe` says. The host writes
/// the recipe JSON of its Minecraft version from it.
///
/// An ingredient is an item id (`minecraft:stick`), an item tag (`#c:dusts`)
/// or — the `_any` builders — any of several.
///
/// ```
/// # use yog_registry::{Cooking, Recipe};
/// Recipe::shaped("yog:ruby_sword", "yog:ruby_sword", 1)
///     .row(" R ").row(" R ").row(" S ")
///     .key('R', "yog:ruby").key('S', "minecraft:stick");
/// Recipe::cooking("yog:ruby", Cooking::Blasting, "#c:ruby_ores", "yog:ruby", 1).experience(0.7);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub id: String,
    pub kind: RecipeKind,
    pub result: String,
    pub count: u32,
    /// Recipes of a group share a slot in the recipe book.
    pub group: Option<String>,
    /// The recipe book's tab (`building`, `equipment`, `misc`, …).
    pub category: Option<String>,
}

/// The game's cooking times (ticks): a furnace, a blast furnace or smoker,
/// a campfire; and the experience a cooking recipe gives unless it says.
const SMELT_TICKS: u32 = 200;
const FAST_COOK_TICKS: u32 = 100;
const CAMPFIRE_TICKS: u32 = 600;
pub const COOK_EXPERIENCE: f64 = 0.1;

impl Recipe {
    fn of(id: impl Into<String>, kind: RecipeKind, result: impl Into<String>, count: u32) -> Self {
        Recipe {
            id: id.into(),
            kind,
            result: result.into(),
            count,
            group: None,
            category: None,
        }
    }

    pub fn shaped(id: impl Into<String>, result: impl Into<String>, count: u32) -> Self {
        Self::of(
            id,
            RecipeKind::Shaped {
                pattern: Vec::new(),
                key: Vec::new(),
            },
            result,
            count,
        )
    }

    pub fn shapeless(id: impl Into<String>, result: impl Into<String>, count: u32) -> Self {
        Self::of(
            id,
            RecipeKind::Shapeless {
                ingredients: Vec::new(),
            },
            result,
            count,
        )
    }

    /// Cooking `ingredient` (100 % of a vanilla furnace: 0.1 experience,
    /// 200 ticks — see [`Self::experience`], [`Self::cook_time`]).
    pub fn cooking(
        id: impl Into<String>,
        method: Cooking,
        ingredient: impl Into<String>,
        result: impl Into<String>,
        count: u32,
    ) -> Self {
        let cook_time = match method {
            Cooking::Smelting => SMELT_TICKS,
            Cooking::Blasting | Cooking::Smoking => FAST_COOK_TICKS,
            Cooking::Campfire => CAMPFIRE_TICKS,
        };
        let kind = RecipeKind::Cooking {
            method,
            ingredient: vec![ingredient.into()],
            experience: COOK_EXPERIENCE as f32,
            cook_time,
        };
        Self::of(id, kind, result, count)
    }

    pub fn stonecutting(
        id: impl Into<String>,
        ingredient: impl Into<String>,
        result: impl Into<String>,
        count: u32,
    ) -> Self {
        Self::of(
            id,
            RecipeKind::Stonecutting {
                ingredient: vec![ingredient.into()],
            },
            result,
            count,
        )
    }

    pub fn smithing(
        id: impl Into<String>,
        template: impl Into<String>,
        base: impl Into<String>,
        addition: impl Into<String>,
        result: impl Into<String>,
    ) -> Self {
        let kind = RecipeKind::Smithing {
            template: vec![template.into()],
            base: vec![base.into()],
            addition: vec![addition.into()],
        };
        Self::of(id, kind, result, 1)
    }

    /// A row of a shaped recipe's pattern.
    pub fn row(mut self, pattern: impl Into<String>) -> Self {
        if let RecipeKind::Shaped { pattern: p, .. } = &mut self.kind {
            p.push(pattern.into());
        }
        self
    }

    /// What `symbol` stands for in a shaped recipe.
    pub fn key(self, symbol: char, item: impl Into<String>) -> Self {
        self.key_any(symbol, [item.into()])
    }

    /// `symbol` stands for any of `items`.
    pub fn key_any(
        mut self,
        symbol: char,
        items: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        if let RecipeKind::Shaped { key, .. } = &mut self.kind {
            key.push((symbol, items.into_iter().map(Into::into).collect()));
        }
        self
    }

    /// One more ingredient of a shapeless recipe.
    pub fn ingredient(self, item: impl Into<String>) -> Self {
        self.ingredient_any([item.into()])
    }

    /// One more ingredient of a shapeless recipe: any of `items`.
    pub fn ingredient_any(mut self, items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        if let RecipeKind::Shapeless { ingredients } = &mut self.kind {
            ingredients.push(items.into_iter().map(Into::into).collect());
        }
        self
    }

    /// Experience a cooking recipe gives.
    pub fn experience(mut self, xp: f32) -> Self {
        if let RecipeKind::Cooking { experience, .. } = &mut self.kind {
            *experience = xp;
        }
        self
    }

    /// A cooking recipe's time in ticks.
    pub fn cook_time(mut self, ticks: u32) -> Self {
        if let RecipeKind::Cooking { cook_time, .. } = &mut self.kind {
            *cook_time = ticks;
        }
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// The recipe in Yog's own format (the `.yrecipe` shape) — what
    /// `Registry::add_recipe` hands the runtime.
    pub fn to_data(&self) -> yog_data::Value {
        use yog_data::{Map, Value};
        let ing = |items: &[String]| match items {
            [one] => Value::from(one.as_str()),
            many => Value::List(many.iter().map(|i| Value::from(i.as_str())).collect()),
        };
        let mut m = Map::with_capacity(8);
        let ty = |m: &mut Map, t: &str| m.insert("type", Value::from(t));
        match &self.kind {
            RecipeKind::Shaped { pattern, key } => {
                ty(&mut m, "shaped");
                m.insert(
                    "pattern",
                    Value::List(pattern.iter().map(|r| Value::from(r.as_str())).collect()),
                );
                let mut keys = Map::with_capacity(key.len());
                for (c, items) in key {
                    keys.insert(c.to_string(), ing(items));
                }
                m.insert("key", Value::Map(keys));
            }
            RecipeKind::Shapeless { ingredients } => {
                ty(&mut m, "shapeless");
                m.insert(
                    "ingredients",
                    Value::List(ingredients.iter().map(|i| ing(i)).collect()),
                );
            }
            RecipeKind::Cooking {
                method,
                ingredient,
                experience,
                cook_time,
            } => {
                ty(&mut m, method.word());
                m.insert("ingredient", ing(ingredient));
                // As written (`0.7`, not f32's 0.699999988…).
                m.insert(
                    "experience",
                    Value::Float(experience.to_string().parse().unwrap_or(0.0)),
                );
                m.insert("cook_time", Value::Int(*cook_time as i64));
            }
            RecipeKind::Stonecutting { ingredient } => {
                ty(&mut m, "stonecutting");
                m.insert("ingredient", ing(ingredient));
            }
            RecipeKind::Smithing {
                template,
                base,
                addition,
            } => {
                ty(&mut m, "smithing");
                m.insert("template", ing(template));
                m.insert("base", ing(base));
                m.insert("addition", ing(addition));
            }
        }
        m.insert("result", Value::from(self.result.as_str()));
        if self.count != 1 {
            m.insert("count", Value::Int(self.count as i64));
        }
        if let Some(g) = &self.group {
            m.insert("group", Value::from(g.as_str()));
        }
        if let Some(c) = &self.category {
            m.insert("category", Value::from(c.as_str()));
        }
        Value::Map(m)
    }

    /// Split `namespace:name` from the recipe id.
    pub fn ns_name(&self) -> (&str, &str) {
        yog_ids::split(&self.id)
    }
}

// ── BookRecipe (like Patchouli's shapeless_book_recipe) ──────────────────────

// ── ItemModifier ─────────────────────────────────────────────────────────────

// ── StartupGrant ─────────────────────────────────────────────────────────────

/// Grant items/books to every player once when they first join.
/// This is the Yog-side replacement for `grant_patchi_book.json`.
#[derive(yog_data::Schema, Debug, Clone)]
pub struct StartupGrant {
    pub id: String,
    pub items: Vec<String>,
    pub book: Option<String>,
    pub command: Option<String>,
}

impl StartupGrant {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            book: None,
            command: None,
        }
    }

    pub fn item(mut self, item_id: impl Into<String>) -> Self {
        self.items.push(item_id.into());
        self
    }

    pub fn book(mut self, book: impl Into<String>) -> Self {
        self.book = Some(book.into());
        self
    }

    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }
}
