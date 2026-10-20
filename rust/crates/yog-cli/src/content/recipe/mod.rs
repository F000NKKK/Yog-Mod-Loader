//! `.yrecipe` files: checked against their kind and written as the game's.

mod body;
mod custom;
mod gaps;

use std::collections::HashMap;
use std::path::Path;

use yog_data::Path as At;
use yog_data::field;
use yog_formats::recipe::{Body, Common, RecipeFile};
use yog_ids::units::MAX_STACK;

use crate::content::file::{check_format, file_id};
use crate::content::{Ctx, Loc, Recipe, RecipeKind, refs};

/// What a recipe is checked against: the mod's namespace, the ids it may
/// name, the mod's kinds of recipe; where its brewing recipes go.
pub(super) struct Against<'a> {
    pub(super) mod_ns: Option<&'a str>,
    pub(super) known: &'a refs::Known,
    pub(super) kinds: &'a HashMap<String, yog_formats::def::RecipeKindDef>,
    pub(super) brews: &'a mut Vec<(String, yog_registry::Brew)>,
}

/// Checks one `.yrecipe` (see `content::recipe`): its schema, then what it
/// names — the mod's content, every Minecraft version's, other mods'
/// declared ids (`known`).
pub(super) fn compile_recipe(
    cx: &mut Ctx,
    text: &str,
    path: &Path,
    against: &mut Against,
) -> Option<Recipe> {
    let doc = parse(cx, text)?;
    let loc_fn = |p: &At| doc.locate(text, p).line as usize;
    let loc: Loc = &loc_fn;
    let read = match yog_data::schema::from_doc::<RecipeFile>(text, &doc) {
        Ok(r) => r,
        Err(errors) => {
            cx.schema_errors(errors);
            return None;
        }
    };
    // Fine while nothing it says is a problem.
    let problems = cx.errors.len();
    names_there(cx, loc, against.known, &read.refs, &read.tags);
    let RecipeFile { common: c, body } = read.value;
    let ty = body.kind().to_string();
    let root = At::root();
    let id = check_common(cx, loc, &c, path, against.mod_ns)?;
    let kind = match body {
        Body::Shaped(s) => body::shaped(cx, loc, s)?,
        Body::Shapeless(s) => body::shapeless(cx, loc, &s)?,
        Body::Cooking(ty, k) => body::cooking(cx, loc, &ty, k)?,
        Body::Stonecutting(k) => RecipeKind::Stonecutting(k.ingredient.ids()),
        Body::Smithing(k) => RecipeKind::Smithing {
            template: k.template.ids(),
            base: k.base.ids(),
            addition: k.addition.ids(),
        },
        Body::Brewing(k) => {
            body::brewing(cx, loc, &c);
            let brew = yog_registry::Brew {
                from: k.from,
                with: k.ingredient,
            };
            if cx.errors.len() == problems {
                against.brews.push((c.result.id.clone(), brew));
            }
            return None;
        }
        Body::Custom(kind, k) => {
            if c.category.is_some() {
                let at = root.key(field!(Common, category));
                cx.err(loc(&at), format!("category is for the game's recipe types; {kind} takes \"slots\" and \"values\""));
            }
            custom::custom(cx, loc, kind, &k, against)?
        }
    };
    (cx.errors.len() == problems).then(|| Recipe {
        id,
        ty,
        kind,
        result: c.result.id.clone(),
        count: c.count,
        group: c.group,
        category: c.category,
        requires: c.requires,
    })
}

/// The file as Yog Data text; its problem when it won't parse.
fn parse(cx: &mut Ctx, text: &str) -> Option<yog_data::text::Doc> {
    yog_data::text::parse(text)
        .map_err(|e| cx.err(e.at.line as usize, e.message))
        .ok()
}

/// What every recipe says: its format, id (returned) and count.
fn check_common(
    cx: &mut Ctx,
    loc: Loc,
    c: &Common,
    path: &Path,
    mod_ns: Option<&str>,
) -> Option<String> {
    let root = At::root();
    check_format(cx, loc, &root.key(field!(Common, format)), c.format);
    let (ns, p) = file_id(
        cx,
        loc(&root.key(field!(Common, id))),
        c.id.as_deref(),
        path,
        mod_ns,
    )?;
    if !(1..=MAX_STACK).contains(&c.count) {
        let at = root.key(field!(Common, count));
        cx.err(
            loc(&at),
            format!("count: {} is out of range (1..={MAX_STACK})", c.count),
        );
    }
    Some(yog_ids::join(&ns, &p))
}

/// Every id and tag the recipe names checked; when they're all known,
/// what's only on some versions or loaders keeps the recipe out of the
/// others (warned).
fn names_there(
    cx: &mut Ctx,
    loc: Loc,
    known: &refs::Known,
    ids: &[yog_data::schema::Found<yog_data::Ref>],
    tags: &[yog_data::schema::Found<yog_data::Ref>],
) {
    let mut ok = true;
    for f in ids {
        if let Some(kind) = refs::RefKind::from_word(&f.value.kind)
            && let Err(e) = known.check(kind, &f.value.id)
        {
            cx.err(loc(&f.path), format!("{}: {e}", f.path));
            ok = false;
        }
    }
    for f in tags {
        if let Err(e) = known.check_tag(&f.value.kind, &f.value.id) {
            cx.err(loc(&f.path), format!("{}: {e}", f.path));
            ok = false;
        }
    }
    if ok {
        for (missing, why) in gaps::recipe_gaps(ids, tags) {
            cx.warn(
                loc(&At::root()),
                format!("left out on {}: {why}", refs::describe_hosts(&missing)),
            );
        }
    }
}
