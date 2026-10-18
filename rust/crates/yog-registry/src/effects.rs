//! Attributes, mob effects and potions a mod adds: data (`.ydef` kinds
//! `attribute`, `effect`, `potion`), the host registers each with the game.

use yog_data::kinds::Item;
use yog_data::with::RefTo;

// What these definitions name: `@attribute:mymod:grid_zoom`,
// `@effect:mymod:clarity`, `@item:yog:potion/awkward`.
yog_data::kind! {
    Attribute = "attribute",
    Effect = "effect",
    Potion = "potion",
}

/// A number every player has (`hexcasting:grid_zoom`) — what effects
/// change (`EffectDef::modifiers`) and the mod reads
/// (`Server::entity_attribute_value`).
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct AttributeDef {
    pub id: String,
    /// Its value with nothing changing it.
    pub default: f64,
    pub min: f64,
    pub max: f64,
}

/// Whether an effect is good or bad for whoever has it (its color in the
/// inventory, what milk and beacons do with it).
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCategory {
    Beneficial,
    Harmful,
    Neutral,
}

/// How a modifier changes an attribute.
#[derive(yog_data::Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierOperation {
    /// Adds `amount`.
    Add,
    /// Adds `amount` × the base value.
    MultiplyBase,
    /// Multiplies the value by `1 + amount`.
    MultiplyTotal,
}

/// An attribute an effect changes while it lasts — by `amount` for each
/// level.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct EffectModifier {
    /// `@attribute:mymod:grid_zoom`, or the game's (`minecraft:generic.movement_speed`).
    #[data(with = "RefTo<Attribute>")]
    pub attribute: String,
    pub amount: f64,
    pub operation: ModifierOperation,
}

/// A mob effect (`hexcasting:enlarge_grid`): its kind, its color (its
/// particles, its potions) and the attributes it changes. Its icon is the
/// mod's `textures/mob_effect/<name>.png`, its name `effect.<ns>.<name>`.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct EffectDef {
    pub id: String,
    pub category: EffectCategory,
    /// `0xRRGGBB`.
    pub color: u32,
    #[data(default)]
    pub modifiers: Vec<EffectModifier>,
}

/// An effect a potion gives.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct PotionEffect {
    /// `@effect:mymod:clarity`, or the game's.
    #[data(with = "RefTo<Effect>")]
    pub effect: String,
    /// Ticks.
    pub duration: u32,
    /// 0 is level I.
    #[data(default)]
    pub amplifier: u32,
}

/// A brewing stand recipe making a potion: one potion and an ingredient.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct Brew {
    /// The potion item brewed from (`@item:yog:potion/awkward`).
    #[data(with = "RefTo<Item>")]
    pub from: String,
    /// The ingredient.
    #[data(with = "RefTo<Item>")]
    pub with: String,
}

/// A potion (`hexcasting:enlarge_grid_long`): the effects it gives, and the
/// brewing that makes it. Its names are the game's `item.minecraft.potion.
/// effect.<name>` (splash, lingering and tipped arrow alike).
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct PotionDef {
    pub id: String,
    /// The name its translations go by (`item.minecraft.potion.effect.<name>`):
    /// its id's by default — a longer or stronger kind shares the plain one's.
    pub name: Option<String>,
    pub effects: Vec<PotionEffect>,
    #[data(default)]
    pub brewed: Vec<Brew>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use yog_data::Schema;

    #[test]
    fn effects_and_potions_cross_as_yog_data() {
        let e = EffectDef {
            id: "mymod:clarity".into(),
            category: EffectCategory::Beneficial,
            color: 0xc875ff,
            modifiers: vec![EffectModifier { attribute: "mymod:grid_zoom".into(), amount: 0.25, operation: ModifierOperation::MultiplyTotal }],
        };
        let back = yog_data::read_bytes::<EffectDef>(&yog_data::binary::encode(&e.write())).unwrap();
        assert_eq!(back, e);
        assert_eq!(yog_data::text::to_string(&e.write()).contains("\"multiply_total\"") && yog_data::text::to_string(&e.write()).contains("\"beneficial\""), true, "{}", yog_data::text::to_string(&e.write()));
        let p = PotionDef {
            id: "mymod:clarity".into(),
            name: None,
            effects: vec![PotionEffect { effect: "mymod:clarity".into(), duration: 3600, amplifier: 0 }],
            brewed: vec![Brew { from: "minecraft:awkward".into(), with: "mymod:dust".into() }],
        };
        assert_eq!(yog_data::read_bytes::<PotionDef>(&yog_data::binary::encode(&p.write())).unwrap(), p);
    }
}
