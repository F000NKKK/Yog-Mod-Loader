//! The game's kinds of recipe: a crafting pattern, shapeless crafting,
//! cooking, brewing.

use yog_data::Path as At;
use yog_data::field;
use yog_formats::recipe::{
    COOK_TIME, Common, Cooking, EXPERIENCE, MAX_SHAPELESS, Shaped, Shapeless,
};
use yog_registry::recipe_kinds::GRID_SIDE;

use crate::content::ctx::{Ctx, Loc};
use crate::content::recipe::registration::RecipeKind;

/// A crafting grid pattern: 1–3 rows of the same 1–3 characters, each
/// character a key (space: an empty place).
pub(super) fn shaped(cx: &mut Ctx, loc: Loc, s: Shaped) -> Option<RecipeKind> {
    let root = At::root();
    let width = s.pattern.first().map_or(0, |r| r.chars().count());
    let rows_fit = (1..=GRID_SIDE).contains(&s.pattern.len()) && (1..=GRID_SIDE).contains(&width);
    if !rows_fit || s.pattern.iter().any(|r| r.chars().count() != width) {
        cx.err(
            loc(&root.key(field!(Shaped, pattern))),
            "pattern must be 1–3 rows of the same 1–3 characters each",
        );
        return None;
    }
    let mut key = Vec::new();
    for (k, ing) in &s.key.0 {
        let at = loc(&root.key(field!(Shaped, key)).key(k));
        let mut chars = k.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) if ch != EMPTY => {
                if !s.pattern.iter().any(|r| r.contains(ch)) {
                    cx.warn(at, format!("key {k:?} isn't used by the pattern"));
                }
                key.push((ch, ing.ids()));
            }
            _ => cx.err(at, format!("key {k:?}: one character other than space")),
        }
    }
    for ch in s
        .pattern
        .iter()
        .flat_map(|r| r.chars())
        .filter(|c| *c != EMPTY)
    {
        if !s.key.0.iter().any(|(k, _)| k.chars().eq([ch])) {
            cx.err(
                loc(&root.key(field!(Shaped, pattern))),
                format!("pattern uses {ch:?}, which key doesn't define"),
            );
            return None;
        }
    }
    Some(RecipeKind::Shaped {
        pattern: s.pattern,
        key,
    })
}

/// A pattern's empty place.
const EMPTY: char = ' ';

/// Crafting ingredients in any arrangement: 1 to a grid's worth.
pub(super) fn shapeless(cx: &mut Ctx, loc: Loc, s: &Shapeless) -> Option<RecipeKind> {
    if s.ingredients.is_empty() || s.ingredients.len() > MAX_SHAPELESS {
        let at = At::root().key(field!(Shapeless, ingredients));
        cx.err(
            loc(&at),
            format!("ingredients must be 1–{MAX_SHAPELESS} items or tags"),
        );
        return None;
    }
    Some(RecipeKind::Shapeless(
        s.ingredients.iter().map(|i| i.ids()).collect(),
    ))
}

/// A furnace's (or a smoker's, a blast furnace's, a campfire's) recipe of
/// kind `ty`: its experience and ticks within the game's bounds; the
/// kind's own ticks by default.
pub(super) fn cooking(cx: &mut Ctx, loc: Loc, ty: &str, k: Cooking) -> Option<RecipeKind> {
    let root = At::root();
    let kind = yog_registry::recipe_kinds::game_kind(ty)?;
    let method = yog_registry::Cooking::of_kind(ty)?;
    if !EXPERIENCE.contains(&k.experience) {
        let at = root.key(field!(Cooking, experience));
        cx.err(
            loc(&at),
            format!("experience: {}–{}", EXPERIENCE.start(), EXPERIENCE.end()),
        );
    }
    let default_time = kind
        .default_of(field!(Cooking, cook_time))
        .map_or(*COOK_TIME.start(), |t| t as u32);
    let cook_time = k.cook_time.unwrap_or(default_time);
    if !COOK_TIME.contains(&cook_time) {
        let at = root.key(field!(Cooking, cook_time));
        cx.err(
            loc(&at),
            format!("cook_time: {}–{} ticks", COOK_TIME.start(), COOK_TIME.end()),
        );
    }
    Some(RecipeKind::Cooking {
        method,
        input: k.ingredient.ids(),
        experience: k.experience,
        cook_time,
    })
}

/// A brewing stand's recipe: one potion, no recipe-book category.
pub(super) fn brewing(cx: &mut Ctx, loc: Loc, c: &Common) {
    let root = At::root();
    if c.count != 1 {
        cx.err(
            loc(&root.key(field!(Common, count))),
            "brewing recipes make one potion",
        );
    }
    if c.category.is_some() {
        cx.err(
            loc(&root.key(field!(Common, category))),
            "brewing recipes don't have a recipe-book category",
        );
    }
}
