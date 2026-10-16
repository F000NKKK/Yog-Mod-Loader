//! The `.yrecipe` format: a recipe of the game's kinds (`shaped`,
//! `shapeless`, cooking, `stonecutting`, `smithing`, brewing) or of the mod's own
//! (`mymod:infusion`, with named `slots` and `values`), in Yog Data. Items
//! are references (`@item:minecraft:stick`), tags `@tag:item:c:dusts`.

use yog_data::schema::{Field, StructShape, read_both};
use yog_data::with::RefTo;
use yog_data::{ItemId, ItemIdOrTag, OneOrTag, Ordered, Reader, Schema, Seg, Shape, Value, ValueRef};

/// The game's recipe types.
pub const TYPES: [&str; 8] = [
    "shaped",
    "shapeless",
    "smelting",
    "blasting",
    "smoking",
    "campfire_cooking",
    "stonecutting",
    "smithing",
    "brewing",
];

/// What every recipe has.
#[derive(Schema)]
pub struct Common {
    /// The file format; 1.
    pub format: Option<u32>,
    /// Its id (`mymod:ruby_block`); the file's name in the mod's namespace by default.
    pub id: Option<String>,
    /// The item it makes, or the potion it brews.
    #[data(with = "RefTo<(yog_data::kinds::Item, yog_data::kinds::Potion)>")]
    pub result: String,
    /// How many (1–64).
    #[data(default = "one")]
    pub count: u32,
    /// Recipes of a group share a recipe-book slot.
    pub group: Option<String>,
    /// The recipe book's tab (`building`, `equipment`, `misc`, …).
    pub category: Option<String>,
    /// Mods it needs — Yog or Java mod ids (`pehkui`): without every one of
    /// them loaded it isn't there (an optional integration).
    #[data(default)]
    pub requires: Vec<String>,
}

fn one() -> u32 {
    1
}

/// One ingredient: an item or a tag, or a list of them (any of them does).
pub struct Ingredient(pub Vec<ItemIdOrTag>);

impl Ingredient {
    /// As the generated code and the runtime write it: ids, `#tag`s.
    pub fn ids(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|i| match i {
                OneOrTag::One(id) => id.id.clone(),
                OneOrTag::Tag(t) => format!("#{}", t.id),
            })
            .collect()
    }
}

impl Schema for Ingredient {
    fn shape() -> Shape {
        ItemIdOrTag::shape()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        match v {
            ValueRef::List([]) => {
                cx.problem("an empty list: at least one item or tag");
                None
            }
            ValueRef::List(_) => Vec::<ItemIdOrTag>::read(v, cx).map(Ingredient),
            _ => ItemIdOrTag::read(v, cx).map(|i| Ingredient(vec![i])),
        }
    }
    fn write(&self) -> Value {
        match self.0.as_slice() {
            [one] => one.write(),
            many => Value::List(many.iter().map(Schema::write).collect()),
        }
    }
}

/// A crafting grid: `pattern` rows of `key` characters (space: empty).
#[derive(Schema)]
pub struct Shaped {
    /// 1–3 rows of the same 1–3 characters.
    pub pattern: Vec<String>,
    /// What each character stands for.
    pub key: Ordered<Ingredient>,
}

/// Crafting ingredients in any arrangement.
#[derive(Schema)]
pub struct Shapeless {
    /// 1–9 ingredients.
    pub ingredients: Vec<Ingredient>,
}

/// A furnace, blast furnace, smoker or campfire.
#[derive(Schema)]
pub struct Cooking {
    pub ingredient: Ingredient,
    /// Experience it gives.
    #[data(default = "experience")]
    pub experience: f64,
    /// Ticks it takes (the station's default if left out).
    pub cook_time: Option<u32>,
}

fn experience() -> f64 {
    0.1
}

#[derive(Schema)]
pub struct Stonecutting {
    pub ingredient: Ingredient,
}

#[derive(Schema)]
pub struct Smithing {
    pub template: Ingredient,
    pub base: Ingredient,
    pub addition: Ingredient,
}

/// A brewing-stand recipe. `result` is the potion id in the common fields.
#[derive(Schema)]
pub struct Brewing {
    /// The potion in the bottle before brewing.
    #[data(with = "RefTo<yog_data::kinds::Potion>")]
    pub from: String,
    /// The item added to the brewing stand.
    #[data(with = "RefTo<yog_data::kinds::Item>")]
    pub ingredient: String,
}

/// A recipe of the mod's own kind: named slots and values, for its
/// machines and books.
#[derive(Schema)]
pub struct Custom {
    /// Slot name → its ingredients.
    pub slots: Ordered<Vec<Ingredient>>,
    /// Anything the machine reads: `time`, `energy`, …
    #[data(default)]
    pub values: Ordered<Value>,
}

pub enum Body {
    Shaped(Shaped),
    Shapeless(Shapeless),
    /// `smelting`, `blasting`, `smoking` or `campfire_cooking`.
    Cooking(String, Cooking),
    Stonecutting(Stonecutting),
    Smithing(Smithing),
    Brewing(Brewing),
    /// The kind (`mymod:infusion`).
    Custom(String, Custom),
}

/// A whole `.yrecipe`.
pub struct RecipeFile {
    pub common: Common,
    pub body: Body,
}

impl Schema for RecipeFile {
    fn shape() -> Shape {
        // `type` and the common fields; the kinds' own come with `type`.
        let mut fields = vec![Field {
            name: "type",
            doc: "The game's recipe type, or the mod's own kind (`mymod:infusion`).",
            shape: String::shape,
            required: true,
            range: None,
        }];
        if let Shape::Struct(c) = Common::shape() {
            fields.extend(c.fields);
        }
        Shape::Struct(StructShape {
            name: "Recipe",
            doc: "A recipe.",
            fields,
        })
    }

    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let map = cx.map(v)?;
        let ty = match map.get("type") {
            Some(ValueRef::Str(t)) => t.to_string(),
            Some(other) => {
                cx.at(Seg::Key("type".into()), |cx| {
                    cx.expected("a recipe type", other)
                });
                return None;
            }
            None => {
                cx.problem(format!(
                    "missing `type` ({}, or the mod's own `mymod:kind`)",
                    TYPES.join(", ")
                ));
                return None;
            }
        };
        let also = &["type"];
        let (common, body) =
            match ty.as_str() {
                "shaped" => {
                    read_both::<Common, Shaped>(v, cx, also).map(|(c, b)| (c, Body::Shaped(b)))?
                }
                "shapeless" => read_both::<Common, Shapeless>(v, cx, also)
                    .map(|(c, b)| (c, Body::Shapeless(b)))?,
                "smelting" | "blasting" | "smoking" | "campfire_cooking" => {
                    read_both::<Common, Cooking>(v, cx, also)
                        .map(|(c, b)| (c, Body::Cooking(ty.clone(), b)))?
                }
                "stonecutting" => read_both::<Common, Stonecutting>(v, cx, also)
                    .map(|(c, b)| (c, Body::Stonecutting(b)))?,
                "smithing" => read_both::<Common, Smithing>(v, cx, also)
                    .map(|(c, b)| (c, Body::Smithing(b)))?,
                "brewing" => read_both::<Common, Brewing>(v, cx, also)
                    .map(|(c, b)| (c, Body::Brewing(b)))?,
                own if own.contains(':') => read_both::<Common, Custom>(v, cx, also)
                    .map(|(c, b)| (c, Body::Custom(ty.clone(), b)))?,
                other => {
                    cx.at(Seg::Key("type".into()), |cx| cx.unknown_name(other, &TYPES));
                    return None;
                }
            };
        Some(RecipeFile { common, body })
    }

    fn write(&self) -> Value {
        let (ty, body) = match &self.body {
            Body::Shaped(b) => ("shaped".to_string(), b.write()),
            Body::Shapeless(b) => ("shapeless".to_string(), b.write()),
            Body::Cooking(t, b) => (t.clone(), b.write()),
            Body::Stonecutting(b) => ("stonecutting".to_string(), b.write()),
            Body::Smithing(b) => ("smithing".to_string(), b.write()),
            Body::Brewing(b) => ("brewing".to_string(), b.write()),
            Body::Custom(t, b) => (t.clone(), b.write()),
        };
        let mut m = yog_data::Map::new();
        m.insert("type", Value::from(ty));
        for part in [self.common.write(), body] {
            if let Value::Map(fields) = part {
                for (k, v) in fields {
                    m.insert(k, v);
                }
            }
        }
        Value::Map(m)
    }
}
