//! `.yrecipe` files: checked against their kind and written as the game's.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use crate::content::Ctx;
use crate::content::Recipe;
use crate::content::RecipeKind;
use crate::content::refs;
use crate::content::valid_namespace;
use crate::content::valid_path;


/// Checks one `.yrecipe` (see `content::recipe`): its schema, then what it
/// names — the mod's content, every Minecraft version's, other mods'
/// declared ids (`known`).
pub(super) fn compile_recipe(
    cx: &mut Ctx,
    text: &str,
    path: &Path,
    mod_ns: Option<&str>,
    known: &refs::Known,
    kinds: &HashMap<String, yog_formats::def::RecipeKindDef>,
    brews: &mut Vec<(String, yog_registry::Brew)>,
) -> Option<Recipe> {
    use yog_formats::recipe::{Body, RecipeFile};
    use yog_data::Path as At;

    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let line = |p: &At| doc.locate(text, p).line as usize;
    let read = match yog_data::schema::from_doc::<RecipeFile>(text, &doc) {
        Ok(r) => r,
        Err(errors) => {
            for e in errors.0 {
                let l = e.at.map_or(1, |a| a.line as usize);
                if e.path.is_root() { cx.err(l, e.message) } else { cx.err(l, format!("{}: {}", e.path, e.message)) }
            }
            return None;
        }
    };
    let mut ok = true;
    for f in &read.refs {
        if let Some(kind) = refs::RefKind::from_word(&f.value.kind)
            && let Err(e) = known.check(kind, &f.value.id)
        {
            cx.err(line(&f.path), format!("{}: {e}", f.path));
            ok = false;
        }
    }
    for f in &read.tags {
        if let Err(e) = known.check_tag(&f.value.kind, &f.value.id) {
            cx.err(line(&f.path), format!("{}: {e}", f.path));
            ok = false;
        }
    }
    // What's only on some versions or loaders keeps the recipe out of the others.
    if ok {
        for (missing, why) in recipe_gaps(&read.refs, &read.tags) {
            cx.warn(line(&At::root()), format!("left out on {}: {why}", refs::describe_hosts(&missing)));
        }
    }
    let RecipeFile { common: c, body } = read.value;
    let ty = body.kind().to_string();
    let root = At::root();
    if let Some(f) = c.format
        && f != 1
    {
        cx.err(line(&root.key("format")), "format must be 1 (this yog understands format 1)");
        ok = false;
    }
    // Its id: as written, else the file's name; the mod's namespace by default.
    let raw = c.id.clone().unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    let id_line = line(&root.key("id"));
    let (ns, p) = match raw.split_once(':') {
        Some((ns, p)) => (ns.to_string(), p.to_string()),
        None => match mod_ns {
            Some(ns) => (ns.to_string(), raw.clone()),
            None => {
                cx.err(id_line, "the mod id isn't a valid Minecraft namespace (a-z, 0-9, `_`, `-`, `.`), so give the namespace: \"id\": \"mynamespace:name\"");
                return None;
            }
        },
    };
    if !valid_namespace(&ns) || !valid_path(&p) {
        cx.err(id_line, format!("id `{ns}:{p}` isn't a valid Minecraft id — lower case a-z, 0-9, `_`, `-`, `.` (and `/` in the path)"));
        return None;
    }
    if !(1..=64).contains(&c.count) {
        cx.err(line(&root.key("count")), format!("count: {} is out of range (1..=64)", c.count));
        ok = false;
    }
    let kind = match body {
        Body::Shaped(s) => {
            let width = s.pattern.first().map_or(0, |r| r.chars().count());
            if s.pattern.is_empty() || s.pattern.len() > 3 || width == 0 || width > 3 || s.pattern.iter().any(|r| r.chars().count() != width) {
                cx.err(line(&root.key("pattern")), "pattern must be 1–3 rows of the same 1–3 characters each");
                return None;
            }
            let mut key = Vec::new();
            for (k, ing) in &s.key.0 {
                let at = line(&root.key("key").key(k));
                let mut chars = k.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) if ch != ' ' => {
                        if !s.pattern.iter().any(|r| r.contains(ch)) {
                            cx.warn(at, format!("key {k:?} isn't used by the pattern"));
                        }
                        key.push((ch, ing.ids()));
                    }
                    _ => {
                        cx.err(at, format!("key {k:?}: one character other than space"));
                        ok = false;
                    }
                }
            }
            for ch in s.pattern.iter().flat_map(|r| r.chars()).filter(|c| *c != ' ') {
                if !s.key.0.iter().any(|(k, _)| k.chars().eq([ch])) {
                    cx.err(line(&root.key("pattern")), format!("pattern uses {ch:?}, which key doesn't define"));
                    return None;
                }
            }
            RecipeKind::Shaped { pattern: s.pattern, key }
        }
        Body::Shapeless(s) => {
            if s.ingredients.is_empty() || s.ingredients.len() > yog_formats::recipe::MAX_SHAPELESS {
                cx.err(line(&root.key("ingredients")), format!("ingredients must be 1–{} items or tags", yog_formats::recipe::MAX_SHAPELESS));
                return None;
            }
            RecipeKind::Shapeless(s.ingredients.iter().map(|i| i.ids()).collect())
        }
        Body::Cooking(ty, k) => {
            use yog_formats::recipe::{COOK_TIME, EXPERIENCE};
            let kind = yog_registry::recipe_kinds::game_kind(&ty).expect("a cooking body is one of the game's kinds");
            let method = yog_registry::Cooking::of_kind(&ty).expect("a cooking body is a cooking kind");
            if !EXPERIENCE.contains(&k.experience) {
                cx.err(line(&root.key("experience")), format!("experience: {}–{}", EXPERIENCE.start(), EXPERIENCE.end()));
                ok = false;
            }
            let default_time = kind.default_of("cook_time").map_or(*COOK_TIME.start(), |t| t as u32);
            let cook_time = k.cook_time.unwrap_or(default_time);
            if !COOK_TIME.contains(&cook_time) {
                cx.err(line(&root.key("cook_time")), format!("cook_time: {}–{} ticks", COOK_TIME.start(), COOK_TIME.end()));
                ok = false;
            }
            RecipeKind::Cooking { method, input: k.ingredient.ids(), experience: k.experience, cook_time }
        }
        Body::Stonecutting(k) => RecipeKind::Stonecutting(k.ingredient.ids()),
        Body::Smithing(k) => RecipeKind::Smithing { template: k.template.ids(), base: k.base.ids(), addition: k.addition.ids() },
        Body::Brewing(k) => {
            if c.count != 1 {
                cx.err(line(&root.key("count")), "brewing recipes make one potion");
                ok = false;
            }
            if c.category.is_some() {
                cx.err(line(&root.key("category")), "brewing recipes don't have a recipe-book category");
                ok = false;
            }
            if ok {
                brews.push((c.result.id.clone(), yog_registry::Brew { from: k.from, with: k.ingredient }));
            }
            return None;
        }
        Body::Custom(kind, k) => {
            // The mod's own kind of recipe: `<mod id>:<kind>`.
            let own = mod_ns.is_some_and(|m| kind.split_once(':').is_some_and(|(n, p)| n == m && valid_path(p)));
            if !own {
                cx.err(
                    line(&root.key("type")),
                    format!("type `{kind}`: one of {}, or the mod's own kind `{}:<kind>`", yog_formats::recipe::TYPES.join(", "), mod_ns.unwrap_or("mymod")),
                );
                return None;
            }
            if c.category.is_some() {
                cx.err(line(&root.key("category")), format!("category is for the game's recipe types; {kind} takes \"slots\" and \"values\""));
                ok = false;
            }
            // What its kind declares (`.ydef` recipe_kind).
            let Some(def) = kinds.get(&kind) else {
                let file = kind.split_once(':').map_or(kind.as_str(), |(_, k)| k);
                cx.err(line(&root.key("type")), format!("type `{kind}`: no such kind — declare it: content/{file}.ydef, {{ kind: \"recipe_kind\", id: \"{file}\", slots: {{ … }} }}"));
                return None;
            };
            for (name, ings) in &k.slots.0 {
                let at = root.key("slots").key(name);
                match def.slots.0.iter().find(|(n, _)| n == name) {
                    None => {
                        let names: Vec<&str> = def.slots.0.iter().map(|(n, _)| n.as_str()).collect();
                        cx.err(line(&at), format!("slots.{name}: {kind} has no slot `{name}` ({})", names.join(", ")));
                        ok = false;
                    }
                    Some((_, spec)) if ings.len() > spec.max as usize => {
                        cx.err(line(&at), format!("slots.{name}: at most {} ingredient(s), found {}", spec.max, ings.len()));
                        ok = false;
                    }
                    _ => {}
                }
            }
            for (name, spec) in &def.slots.0 {
                if spec.required && !k.slots.0.iter().any(|(n, ings)| n == name && !ings.is_empty()) {
                    cx.err(line(&root.key("slots")), format!("slots: {kind} needs `{name}`"));
                    ok = false;
                }
            }
            for (name, v) in &k.values.0 {
                let at = root.key("values").key(name);
                let fits = match def.values.0.iter().find(|(n, _)| n == name).map(|(_, t)| *t) {
                    None => {
                        let names: Vec<&str> = def.values.0.iter().map(|(n, _)| n.as_str()).collect();
                        cx.err(line(&at), format!("values.{name}: {kind} has no value `{name}` ({})", names.join(", ")));
                        ok = false;
                        continue;
                    }
                    Some(yog_formats::def::ValueType::Number) => v.as_f64().is_some(),
                    Some(yog_formats::def::ValueType::Bool) => v.as_bool().is_some(),
                    Some(yog_formats::def::ValueType::Text) => v.as_str().is_some(),
                };
                if !fits {
                    cx.err(line(&at), format!("values.{name}: a {}", yog_formats::content::word(&def.values.0.iter().find(|(n, _)| n == name).unwrap().1)));
                    ok = false;
                }
            }
            let slots = k.slots.0.iter().map(|(name, ings)| (name.clone(), ings.iter().map(|i| i.ids()).collect())).collect();
            let values = k.values.0.iter().map(|(name, v)| (name.clone(), serde_json::from_str(&yog_data::text::to_string(v)).unwrap_or(Value::Null))).collect();
            RecipeKind::Custom { kind, slots, values }
        }
    };
    ok.then(|| Recipe {
        id: format!("{ns}:{p}"),
        ty,
        kind,
        result: c.result.id.clone(),
        count: c.count,
        group: c.group,
        category: c.category,
        requires: c.requires,
    })
}

/// The hosts (`<loader>/<version>`) a recipe naming `refs` and `tags` can't
/// be on, with why: an item or tag one hasn't got. Alternatives (`["a",
/// "#b"]`, one list in the file) count as there when any of them is.
pub(super) fn recipe_gaps(items: &[yog_data::schema::Found<yog_data::Ref>], tags: &[yog_data::schema::Found<yog_data::Ref>]) -> Vec<(BTreeSet<String>, String)> {
    use refs::RefKind;
    // Where each reference is (`None`: everywhere), grouped by the list it's in.
    let mut lists: BTreeMap<String, Vec<(String, Option<BTreeSet<String>>)>> = BTreeMap::new();
    let parent = |p: &yog_data::Path| {
        let mut p = p.clone();
        if matches!(p.segments().last(), Some(yog_data::Seg::Index(_))) {
            p.pop();
        }
        p.to_string()
    };
    for f in items {
        let Some(kind) = RefKind::from_word(&f.value.kind) else { continue };
        let potion_alias = kind == RefKind::Item && f.value.id.starts_with("yog:potion/");
        if !potion_alias && !f.value.id.starts_with("minecraft:") {
            continue;
        }
        let on: BTreeSet<String> = refs::hosts()
            .iter()
            .filter(|h| {
                h.split_once('/').is_some_and(|(_, v)| {
                    if potion_alias {
                        refs::is_vanilla_potion_in(&f.value.id, v)
                    } else {
                        refs::equivalent(kind, &f.value.id, v).is_some()
                    }
                })
            })
            .cloned()
            .collect();
        let on = (on.len() < refs::hosts().len()).then_some(on);
        lists.entry(parent(&f.path)).or_default().push((f.value.id.clone(), on));
    }
    for f in tags {
        let Some(on) = refs::tag_hosts(yog_ids::registry_of(&f.value.kind), &f.value.id) else { continue };
        lists.entry(parent(&f.path)).or_default().push((format!("#{}", f.value.id), on));
    }
    let mut out = Vec::new();
    for alts in lists.values() {
        if alts.iter().any(|(_, on)| on.is_none()) {
            continue;
        }
        let on: BTreeSet<String> = alts.iter().flat_map(|(_, on)| on.iter().flatten().cloned()).collect();
        let missing: BTreeSet<String> = refs::hosts().iter().filter(|h| !on.contains(*h)).cloned().collect();
        if missing.is_empty() {
            continue;
        }
        let names: Vec<&str> = alts.iter().map(|(id, _)| id.as_str()).collect();
        // A Yog tag that's everywhere, if one is these.
        let hint = alts
            .iter()
            .filter_map(|(id, _)| id.strip_prefix('#').and_then(|t| refs::yog_tag_for("item", t)))
            .find(|y| refs::tag_hosts("item", y) == Some(None))
            .map(|y| format!(" — @tag:item:{y} is on every version and loader"))
            .unwrap_or_default();
        out.push((missing, format!("{} isn't there{hint}", names.join(" / "))));
    }
    out
}
