//! The game's kinds of recipe: a crafting pattern, shapeless crafting,
//! cooking (what each must keep is yog_formats::rules' to say).

use yog_data::field;
use yog_formats::recipe::{COOK_TIME, Cooking, Shaped, Shapeless};

use crate::content::formats::recipe::registration::RecipeKind;

/// A pattern's empty place.
const EMPTY: char = ' ';

/// A crafting grid pattern and its key's ingredients.
pub(super) fn shaped(s: Shaped) -> RecipeKind {
    let key = s
        .key
        .0
        .iter()
        .filter_map(|(k, ing)| {
            let ch = k.chars().next().filter(|c| *c != EMPTY)?;
            Some((ch, yog_formats::recipe::ids(ing)))
        })
        .collect();
    RecipeKind::Shaped {
        pattern: s.pattern,
        key,
    }
}

/// Crafting ingredients in any arrangement.
pub(super) fn shapeless(s: &Shapeless) -> RecipeKind {
    RecipeKind::Shapeless(s.ingredients.iter().map(yog_formats::recipe::ids).collect())
}

/// A furnace's (or a smoker's, a blast furnace's, a campfire's) recipe of
/// kind `ty`; the kind's own ticks by default.
pub(super) fn cooking(ty: &str, k: Cooking) -> Option<RecipeKind> {
    let kind = yog_registry::recipe_kinds::game_kind(ty)?;
    let method = yog_registry::Cooking::of_kind(ty)?;
    let default_time = kind
        .default_of(field!(Cooking, cook_time))
        .map_or(*COOK_TIME.start(), |t| t as u32);
    Some(RecipeKind::Cooking {
        method,
        input: yog_formats::recipe::ids(&k.ingredient),
        experience: k.experience,
        cook_time: k.cook_time.unwrap_or(default_time),
    })
}
