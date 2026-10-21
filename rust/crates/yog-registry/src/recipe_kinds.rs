//! Kinds of recipe: what slots a recipe of the kind fills and what values
//! it has. A mod declares its own (`.ydef` `kind: "recipe_kind"`); the
//! game's are the same definitions, built in as data ([`GAME`],
//! `data/recipe_kinds.ydata`) — the one list every tool reads them from
//! (the build, books, the IDE).

use yog_sync::LazyLock;

use yog_data::{Ordered, Schema};

/// One slot of a kind: the places a recipe puts ingredients in.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct SlotSpec {
    /// What editors call it (`Template`); its name by default.
    pub name: Option<String>,
    /// What goes in it, for editors' hints.
    pub description: Option<String>,
    /// At most this many (each one item or a list of alternatives).
    #[data(default = "one", range = "1..=64")]
    pub max: u32,
    /// A recipe of the kind must fill it.
    #[data(default)]
    pub required: bool,
    /// The game's kinds: the `.yrecipe` field it's written in (`ingredient`,
    /// `from`…); a mod's kinds write theirs under `slots`.
    pub field: Option<String>,
    /// The places are a crafting grid's, by position: the recipe writes
    /// them as `pattern` rows and a `key` (a shaped recipe).
    #[data(default)]
    pub grid: bool,
}

fn one() -> u32 {
    1
}

/// What a recipe's value is.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Number,
    Text,
    Bool,
}

/// A kind of recipe: `{ kind: "recipe_kind", id: "infusion", slots: { core:
/// { required: true }, ring: { max: 8 } }, values: { time: "number" },
/// defaults: { time: 100 } }`. A `.yui` `<Component recipe_layout="infusion">`
/// shows it.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct RecipeKindDef {
    /// `infusion` (the mod's namespace is added) or `mymod:infusion`; the
    /// game's are bare (`smelting`).
    pub id: String,
    /// What editors call it (`Smelting`); its id by default.
    pub name: Option<String>,
    #[data(default)]
    pub slots: Ordered<SlotSpec>,
    #[data(default)]
    pub values: Ordered<ValueType>,
    /// A number value's value when a recipe doesn't say.
    #[data(default)]
    pub defaults: Ordered<f64>,
}

impl RecipeKindDef {
    /// Slot `name`'s spec.
    pub fn slot(&self, name: &str) -> Option<&SlotSpec> {
        self.slots.0.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }

    /// Value `name`'s default, if it has one.
    pub fn default_of(&self, name: &str) -> Option<f64> {
        self.defaults
            .0
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
    }
}

/// The game's kinds' ids — what the code that handles a kind of its own
/// (a typed body, a host's recipe JSON) names it by. The data ([`GAME`])
/// has exactly these, in this order.
pub mod ids {
    pub use yog_ids::recipe_kind::{BLASTING, BREWING, CAMPFIRE_COOKING, SHAPED, SHAPELESS, SMELTING, SMITHING, SMOKING, STONECUTTING};

    /// Every one, in [`super::GAME`]'s order.
    pub const ALL: &[&str] = yog_ids::recipe_kind::ALL_VALUES;
    /// The cooking kinds: one input, a cook time, experience.
    pub const COOKING: [&str; 4] = [SMELTING, BLASTING, SMOKING, CAMPFIRE_COOKING];
}

/// The fields every `.yrecipe` (and its compiled form) has, whatever its
/// kind; a game kind's slots name theirs in [`SlotSpec::field`].
pub mod fields {
    pub use yog_ids::recipe_field::{COUNT, KEY, PATTERN, RESULT, SLOTS, TYPE, VALUES};
}

/// A crafting grid's side: a grid slot has `GRID_SIDE²` places, row by row.
pub const GRID_SIDE: usize = 3;

/// The game's kinds of recipe.
pub static GAME: LazyLock<Vec<RecipeKindDef>> = LazyLock::new(|| {
    yog_data::built_in::<Vec<RecipeKindDef>>(
        "yog-registry's recipe kinds",
        include_str!("../data/recipe_kinds.ydata"),
    )
});

/// The game's kind `id` (`smelting`).
pub fn game_kind(id: &str) -> Option<&'static RecipeKindDef> {
    GAME.iter().find(|k| k.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_kinds_are_data() {
        let data: Vec<&str> = GAME.iter().map(|k| k.id.as_str()).collect();
        assert_eq!(data, ids::ALL, "the data and the code name the same kinds");
        let smelting = game_kind(ids::SMELTING).unwrap();
        assert_eq!(
            smelting.slot("input").and_then(|s| s.field.as_deref()),
            Some("ingredient")
        );
        assert_eq!(smelting.default_of("cook_time"), Some(200.0));
        assert!(game_kind(ids::SHAPED).unwrap().slot("grid").unwrap().grid);
        let brewing = game_kind(ids::BREWING).unwrap();
        assert_eq!(
            brewing.slot("from").and_then(|s| s.field.as_deref()),
            Some("from")
        );
    }
}
