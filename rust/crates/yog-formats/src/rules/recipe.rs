//! A recipe's body: a crafting pattern its key fills, a grid's worth of
//! shapeless ingredients, cooking within the game's bounds, one potion
//! brewed, a recipe-book category only for the game's kinds.

use yog_data::{Diagnostic, Path, field};
use yog_registry::recipe_kinds::GRID_SIDE;

use super::{Rules, error, warning};
use crate::recipe::{
    Body, COOK_TIME, Common, Cooking, EXPERIENCE, MAX_SHAPELESS, RecipeFile, Shaped, Shapeless,
};

/// A pattern's empty place.
const EMPTY: char = ' ';

impl Rules for RecipeFile {
    fn rules(&self, out: &mut Vec<Diagnostic>) {
        let c = &self.common;
        match &self.body {
            Body::Shaped(s) => shaped(s, out),
            Body::Shapeless(s) => shapeless(s, out),
            Body::Cooking(ty, k) => cooking(ty, k, out),
            Body::Brewing(_) => brewing(c, out),
            Body::Custom(kind, _) if c.category.is_some() => out.push(error(
                Path::root().key(field!(Common, category)),
                format!(
                    "category is for the game's recipe types; {kind} takes \"slots\" and \"values\""
                ),
            )),
            _ => {}
        }
    }
}

/// 1–3 rows of the same 1–3 characters, each a key's (space: an empty place).
fn shaped(s: &Shaped, out: &mut Vec<Diagnostic>) {
    let pattern = Path::root().key(field!(Shaped, pattern));
    let width = s.pattern.first().map_or(0, |r| r.chars().count());
    let fits = (1..=GRID_SIDE).contains(&s.pattern.len()) && (1..=GRID_SIDE).contains(&width);
    if !fits || s.pattern.iter().any(|r| r.chars().count() != width) {
        out.push(error(
            pattern,
            "pattern must be 1–3 rows of the same 1–3 characters each",
        ));
        return;
    }
    for (k, _) in &s.key.0 {
        let at = Path::root().key(field!(Shaped, key)).key(k);
        let mut chars = k.chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) if ch != EMPTY => {
                if !s.pattern.iter().any(|r| r.contains(ch)) {
                    out.push(warning(at, format!("key {k:?} isn't used by the pattern")));
                }
            }
            _ => out.push(error(
                at,
                format!("key {k:?}: one character other than space"),
            )),
        }
    }
    let undefined = s
        .pattern
        .iter()
        .flat_map(|r| r.chars())
        .filter(|c| *c != EMPTY)
        .find(|ch| !s.key.0.iter().any(|(k, _)| k.chars().eq([*ch])));
    if let Some(ch) = undefined {
        out.push(error(
            pattern,
            format!("pattern uses {ch:?}, which key doesn't define"),
        ));
    }
}

/// 1 to a grid's worth of ingredients.
fn shapeless(s: &Shapeless, out: &mut Vec<Diagnostic>) {
    if s.ingredients.is_empty() || s.ingredients.len() > MAX_SHAPELESS {
        out.push(error(
            Path::root().key(field!(Shapeless, ingredients)),
            format!("ingredients must be 1–{MAX_SHAPELESS} items or tags"),
        ));
    }
}

/// Experience and ticks within the game's bounds (kind `ty`'s ticks by default).
fn cooking(ty: &str, k: &Cooking, out: &mut Vec<Diagnostic>) {
    if !EXPERIENCE.contains(&k.experience) {
        out.push(error(
            Path::root().key(field!(Cooking, experience)),
            format!("experience: {}–{}", EXPERIENCE.start(), EXPERIENCE.end()),
        ));
    }
    let default_time = yog_registry::recipe_kinds::game_kind(ty)
        .and_then(|kind| kind.default_of(field!(Cooking, cook_time)))
        .map_or(*COOK_TIME.start(), |t| t as u32);
    if !COOK_TIME.contains(&k.cook_time.unwrap_or(default_time)) {
        out.push(error(
            Path::root().key(field!(Cooking, cook_time)),
            format!("cook_time: {}–{} ticks", COOK_TIME.start(), COOK_TIME.end()),
        ));
    }
}

/// One potion, no recipe-book category.
fn brewing(c: &Common, out: &mut Vec<Diagnostic>) {
    if c.count != 1 {
        out.push(error(
            Path::root().key(field!(Common, count)),
            "brewing recipes make one potion",
        ));
    }
    if c.category.is_some() {
        out.push(error(
            Path::root().key(field!(Common, category)),
            "brewing recipes don't have a recipe-book category",
        ));
    }
}
