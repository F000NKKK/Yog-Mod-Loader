//! `.ybook` files and `.yui` recipe layouts: checked, references resolved.

use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use crate::content::BookFile;
use crate::content::Compiled;
use crate::content::Ctx;
use crate::content::OreOut;
use crate::content::defs::advancement;
use crate::content::defs::content_def;
use crate::content::defs::damage_type;
use crate::content::defs::dimension;
use crate::content::defs::ore;
use crate::content::defs::tag_entries;
use crate::content::defs::tree;
use crate::content::pretty;
use crate::content::refs;
use crate::content::table;
use crate::content::valid_id;
use crate::content::valid_path;


/// Checks one `.ybook` (`yog_formats::book::Book`'s schema), whose references
/// (`@item:minecraft:stick`, see `yog_formats::book::refs`) must name what exists
/// — in this mod, on every Minecraft version, or among the other mods'
/// ids `yog.toml` declares — and come out as plain ids. Texts may be
/// `@<Set>.<name>` (a `.yres` text), `@item:<id>` / `@block:<id>` /
/// `@entity:<id>` (its name) or `@@…` (a literal `@`); a book with text
/// references is shown in the player's language. An `id` without a
/// namespace gets the mod's.
pub(super) fn compile_book(
    cx: &mut Ctx,
    text: &str,
    mod_ns: Option<&str>,
    known: &refs::Known,
    layouts: &[yog_formats::book::layout::RecipeLayout],
    recipe_kinds: &HashMap<String, String>,
    page_types: &[yog_formats::book::page_type::PageType],
) -> Option<BookFile> {
    use refs::RefKind;
    use yog_data::schema::{Resolve, Text};
    use yog_data::{Path, Ref, Res};

    struct Check<'a> {
        known: &'a refs::Known,
        texts: &'a crate::yres::Refs,
        entries: HashSet<String>,
        categories: HashSet<String>,
        text_refs: bool,
    }

    impl Resolve for Check<'_> {
        fn reference(&mut self, r: &Ref, _: &Path) -> Result<String, String> {
            let own = match r.kind.as_str() {
                "entry" => Some(&self.entries),
                "category" => Some(&self.categories),
                _ => None,
            };
            match (own, RefKind::from_word(&r.kind)) {
                (Some(set), _) if set.contains(&r.id) => Ok(r.id.clone()),
                (Some(_), _) => Err(format!("no {} {} in this book", r.kind, r.id)),
                (None, Some(kind)) => self.known.check(kind, &r.id).map(|()| r.id.clone()),
                (None, None) => Err(format!("`@{}:` names nothing a book shows", r.kind)),
            }
        }

        fn text(&mut self, t: &Text, _: &Path) -> Result<String, String> {
            // A name: the game's translation of the item, block or entity — one that's there.
            if let Text::NameOf(r) = t {
                let kind = RefKind::from_word(&r.kind)
                    .filter(|_| matches!(r.kind.as_str(), yog_ids::kind::ITEM | yog_ids::kind::BLOCK | yog_ids::kind::ENTITY))
                    .ok_or_else(|| format!("`@{}:{}` has no name to show", r.kind, r.id))?;
                self.known.check(kind, &r.id)?;
            }
            if let Text::Literal(s) = t {
                return Ok(s.clone());
            }
            let key = self.texts.key_of(t)?.unwrap_or_default();
            self.text_refs = true;
            Ok(key)
        }

        fn words(&mut self, r: &Res) -> Option<String> {
            self.texts.text(r).ok().map(|(_, t)| t.clone())
        }
    }

    // Entries and categories may name each other in any order: their ids first.
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let ids = |key: &str| -> HashSet<String> {
        doc.value.get(key).and_then(|v| v.as_list()).unwrap_or_default().iter().filter_map(|e| e.get("id")?.as_str().map(str::to_string)).collect()
    };
    let mut check = Check { known, texts: cx.refs, entries: ids("entries"), categories: ids("categories"), text_refs: false };
    let mut book = match yog_data::from_doc_with::<yog_formats::book::Book>(text, &doc, &mut check) {
        Ok(read) => read.value,
        Err(errors) => {
            for e in errors.0 {
                let line = e.at.map_or(1, |a| a.line as usize);
                if e.path.is_root() {
                    cx.err(line, e.message);
                } else {
                    cx.err(line, format!("{}: {}", e.path, e.message));
                }
            }
            return None;
        }
    };
    if !book.id.contains(':') {
        book.id = format!("{}:{}", mod_ns.unwrap_or("minecraft"), book.id);
    }
    // References to entries and categories always have a namespace, so their ids must too.
    let ns = mod_ns.unwrap_or("minecraft");
    let unnamespaced: Vec<String> = (book.categories.iter().enumerate().map(|(i, c)| (format!("categories[{i}].id"), &c.id)))
        .chain(book.entries.iter().enumerate().map(|(i, e)| (format!("entries[{i}].id"), &e.id)))
        .filter(|(_, id)| !id.contains(':'))
        .map(|(path, id)| format!("{path}: `{id}`: write its namespace (`{ns}:{id}`)"))
        .collect();
    if !unnamespaced.is_empty() {
        for e in unnamespaced {
            cx.err(1, e);
        }
        return None;
    }
    if !valid_id(&book.id) {
        cx.err(1, format!("book id `{}`: not a valid id", book.id));
        return None;
    }
    // Pages of a mod's own kind: their fields checked by their type and
    // resolved like the rest of the book; the types they use go with it.
    let mut used: Vec<yog_formats::book::page_type::PageType> = Vec::new();
    let mut fields_ok = true;
    for (e, entry) in book.entries.iter_mut().enumerate() {
        for (p, page) in entry.pages.iter_mut().enumerate() {
            let yog_formats::book::BookPage::Custom(c) = page else { continue };
            use yog_formats::book::page_type::PropKind as K;
            use yog_data::with::With;
            let at = Path::root().key("entries").index(e).key("pages").index(p);
            let line = |path: &Path| doc.locate(text, path).line as usize;
            let Some(t) = page_types.iter().find(|t| t.id == c.kind) else {
                if c.kind.split(':').next() == mod_ns {
                    cx.err(line(&at.key("type")), format!("{}.type: no page type {} — a .yui component with book_page=\"{}\"", at, c.kind, c.kind));
                    fields_ok = false;
                } else {
                    cx.warn(line(&at.key("type")), format!("{}.type: {} is another mod's page type — not checked here", at, c.kind));
                }
                continue;
            };
            for (k, _) in c.fields.iter() {
                if k != "anchor" && !t.properties.iter().any(|p| p.name == k) {
                    let names: Vec<&str> = t.properties.iter().map(|p| p.name.as_str()).collect();
                    cx.err(line(&at.key(k)), format!("{}: {} has no field `{k}` ({})", at.key(k), c.kind, names.join(", ")));
                    fields_ok = false;
                }
            }
            for prop in &t.properties {
                let path = at.key(&prop.name);
                let Some(v) = c.fields.get(&prop.name).cloned() else {
                    if prop.default.is_none() {
                        cx.err(line(&at), format!("{at}: missing `{}` ({})", prop.name, c.kind));
                        fields_ok = false;
                    }
                    continue;
                };
                let arena = yog_data::Bump::new();
                let r = v.borrow(&arena);
                let mut reader = yog_data::Reader::with_resolver(&mut check);
                let read = match prop.kind {
                    K::Text => yog_data::with::TextOf::read(&r, &mut reader).map(yog_data::Value::Str),
                    K::BookText => yog_formats::book::refs::BookText::read(&r, &mut reader).map(yog_data::Value::Str),
                    K::Item => yog_data::with::RefTo::<(yog_data::kinds::Item, yog_data::kinds::Block)>::read(&r, &mut reader).map(yog_data::Value::Str),
                    K::Image => yog_formats::book::refs::Icon::read(&r, &mut reader).map(yog_data::Value::Str),
                    K::Number => <f64 as yog_data::Schema>::read(&r, &mut reader).map(yog_data::Value::Float),
                    K::Bool => <bool as yog_data::Schema>::read(&r, &mut reader).map(yog_data::Value::Bool),
                    K::Data => Some(v.clone()),
                };
                let problems = std::mem::take(&mut reader.problems);
                drop(reader);
                for pr in problems {
                    cx.err(line(&path), format!("{path}: {}", pr.message));
                    fields_ok = false;
                }
                if let Some(out) = read {
                    c.fields.insert(prop.name.as_str(), out);
                }
            }
            if !used.contains(t) {
                used.push(t.clone());
            }
        }
    }
    if !fields_ok {
        return None;
    }
    book.page_types = used;
    book.i18n |= check.text_refs;
    // Every recipe a page shows needs a layout for its kind (the game's are built in).
    let mut ok = true;
    for (e, entry) in book.entries.iter().enumerate() {
        for (p, page) in entry.pages.iter().enumerate() {
            if let yog_formats::book::BookPage::Recipe { recipe, .. } = page
                && let Some(kind) = recipe_kinds.get(recipe)
                && yog_formats::book::layout::find(layouts, kind).is_none()
            {
                let path = yog_data::Path::root().key("entries").index(e).key("pages").index(p).key("recipe");
                let short = kind.split_once(':').map_or(kind.as_str(), |(_, k)| k);
                cx.err(
                    doc.locate(text, &path).line as usize,
                    format!("{path}: no layout for recipe kind {kind} — lay it out in a .yui: <Component recipe_layout=\"{short}\" width=\"…\" height=\"…\"> (see yog_formats::book::layout)"),
                );
                ok = false;
            }
        }
    }
    if !ok {
        return None;
    }
    book.layouts = layouts.to_vec();
    Some(BookFile { id: book.id.clone(), data: yog_data::binary::encode(&yog_data::Schema::write(&book)) })
}

/// Checks one `.ydef` of kind `recipe_kind` or `recipe_layout`: the items
/// its layout draws exist, the kinds it lays out are the game's or the
/// mod's own. A kind of recipe, and the layout (of its kinds).
pub(super) fn compile_layout(
    cx: &mut Ctx,
    text: &str,
    mod_ns: Option<&str>,
    known: &refs::Known,
    ores: &mut Vec<OreOut>,
    dimensions: &mut Vec<(String, Vec<u8>)>,
    tables: &mut Vec<table::Table>,
    loot: &mut Vec<(String, Vec<u8>)>,
    definitions: &mut Vec<(&'static str, String, Vec<u8>)>,
    advancements: &mut Vec<String>,
    out: &mut Compiled,
) -> Option<yog_formats::def::RecipeKindDef> {
    use yog_formats::def::Def;
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let read = match yog_data::schema::from_doc::<Def>(text, &doc) {
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
        // A tag's `optional` entries are other mods' — there or not.
        if matches!(read.value, Def::Tag(_)) && f.path.to_string().starts_with("optional") {
            continue;
        }
        if let Some(kind) = refs::RefKind::from_word(&f.value.kind)
            && let Err(e) = known.check(kind, &f.value.id)
        {
            cx.err(doc.locate(text, &f.path).line as usize, format!("{}: {e}", f.path));
            ok = false;
        }
    }
    for f in &read.tags {
        if matches!(read.value, Def::Tag(_)) && f.path.to_string().starts_with("optional") {
            continue;
        }
        if let Err(e) = known.check_tag(&f.value.kind, &f.value.id) {
            cx.err(doc.locate(text, &f.path).line as usize, format!("{}: {e}", f.path));
            ok = false;
        }
    }
    let ns = mod_ns.unwrap_or("minecraft");
    let kind = match read.value {
        Def::RecipeKind(mut k) => {
            if !k.id.contains(':') {
                k.id = format!("{ns}:{}", k.id);
            }
            if k.id.split_once(':').is_none_or(|(n, p)| n != ns || !valid_path(p)) {
                cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, format!("id: `{}`: the mod's own kind, `{ns}:<kind>`", k.id));
                ok = false;
            }
            k
        }
        Def::Ore(o) => {
            if ok {
                ores.push(ore(o, ns));
            }
            return None;
        }
        Def::Dimension(d) => {
            if ok {
                dimensions.push(dimension(d, ns));
            }
            return None;
        }
        Def::Tag(t) => {
            let line = doc.locate(text, &yog_data::Path::root().key("values")).line as usize;
            match tag_entries(&t, ns) {
                Ok((path, entries)) => out.tags.entry(path).or_default().extend(entries),
                Err(problems) => problems.into_iter().for_each(|p| cx.err(line, p)),
            }
            return None;
        }
        Def::DamageType(d) => {
            match damage_type(&d, ns) {
                Ok((file, tags)) => {
                    out.outputs.push(file);
                    for (path, id) in tags {
                        out.tags.entry(path).or_default().insert(id, true);
                    }
                }
                Err(e) => cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, e),
            }
            return None;
        }
        Def::Tree(t) => {
            match tree(&t, ns) {
                Ok(file) => {
                    if ok {
                        out.outputs.push(file);
                    }
                }
                Err(e) => cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, e),
            }
            return None;
        }
        Def::LootModifier(mut m) => {
            m.id = yog_ids::qualify_in(ns, &m.id);
            if m.id.split_once(':').is_none_or(|(n, p)| n != ns || !valid_path(p)) {
                cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, format!("id: `{}`: the mod's own, `{ns}:<name>`", m.id));
                ok = false;
            }
            if ok {
                use yog_data::Schema;
                loot.push((m.id.clone(), yog_data::binary::encode(&m.write())));
            }
            return None;
        }
        // A table of its own: an empty one in the datapack (so it exists for
        // `/loot` and other tables), filled by a modifier on it.
        Def::Advancement(a) => {
            let at = |key: &str| doc.locate(text, &yog_data::Path::root().key(key)).line as usize;
            match advancement(cx, &mut out.lang, &a, ns) {
                Ok(files) if ok => {
                    out.outputs.extend(files);
                    advancements.push(yog_ids::qualify_in(ns, &a.id));
                }
                Ok(_) => {}
                Err((key, e)) => cx.err(at(key), format!("{key}: {e}")),
            }
            return None;
        }
        // Registered with the game: the host reads them from the content.
        Def::Attribute(mut a) => {
            a.id = yog_ids::qualify_in(ns, &a.id);
            content_def(cx, &doc, text, ns, yog_ids::kind::ATTRIBUTE, a.id.clone(), &a, ok, definitions);
            return None;
        }
        Def::Effect(mut e) => {
            e.id = yog_ids::qualify_in(ns, &e.id);
            content_def(cx, &doc, text, ns, yog_ids::kind::EFFECT, e.id.clone(), &e, ok, definitions);
            return None;
        }
        Def::LootTable(t) => {
            let id = yog_ids::qualify_in(ns, &t.id);
            let Some(path) = id.strip_prefix(&format!("{ns}:")).filter(|p| valid_path(p)) else {
                cx.err(doc.locate(text, &yog_data::Path::root().key("id")).line as usize, format!("id: `{id}`: the mod's own, `{ns}:<name>`"));
                return None;
            };
            if ok {
                use yog_data::Schema;
                out.outputs.push((format!("data/{ns}/loot_table/{path}.json"), pretty(&json!({ "type": "minecraft:generic", "pools": [] }))));
                let fill = yog_registry::LootModifier { id: id.clone(), tables: vec![id.clone()], add: t.add, scale: Vec::new() };
                loot.push((id, yog_data::binary::encode(&fill.write())));
            }
            return None;
        }
        Def::Table(t) => {
            match table::check(&cx.file, t, cx.refs, known) {
                Ok(t) => tables.push(t),
                Err(errors) => {
                    for e in errors {
                        // `rows.wood.size: …` — at its line.
                        let path = e.split_once(':').map_or("", |(p, _)| p);
                        let at = path.split('.').fold(yog_data::Path::root(), |at, k| at.key(k));
                        cx.err(doc.locate(text, &at).line as usize, e);
                    }
                }
            }
            return None;
        }
        _ => return None,
    };
    ok.then_some(kind)
}

/// The mod's recipe layouts (`.yui` `<Component recipe_layout="…">`),
/// checked: each kind the game's or one the mod declares, laid out once;
/// a mod kind's slots its own; stations known items.
pub(super) fn recipe_layouts(
    root: &Path,
    ns: &str,
    kinds: &HashMap<String, yog_formats::def::RecipeKindDef>,
    known: &refs::Known,
    errors: &mut Vec<String>,
) -> Vec<yog_formats::book::layout::RecipeLayout> {
    use yog_formats::book::layout::LayoutPart;
    let mut laid_out: HashMap<String, String> = HashMap::new();
    let mut out = Vec::new();
    for (shown, l) in crate::book_page::find_layouts(root, ns, errors) {
        let mut ok = true;
        for k in &l.kinds {
            let game = yog_formats::recipe::TYPES.contains(&k.as_str());
            if !game && !kinds.contains_key(k) {
                errors.push(format!(
                    "{shown}: recipe_layout: `{k}`: one of {}, or a kind the mod declares (a .ydef of kind recipe_kind)",
                    yog_formats::recipe::TYPES.join(", ")
                ));
                ok = false;
            }
            if let Some(other) = laid_out.insert(k.clone(), shown.clone()) {
                errors.push(format!("{shown}: recipe kind {k} is also laid out by {other}"));
                ok = false;
            }
        }
        for part in &l.parts {
            match part {
                LayoutPart::Slots { slot, .. } => {
                    for k in l.kinds.iter().filter_map(|k| kinds.get(k)) {
                        if !k.slots.0.iter().any(|(s, _)| s == slot) {
                            let all: Vec<&str> = k.slots.0.iter().map(|(s, _)| s.as_str()).collect();
                            errors.push(format!("{shown}: <RecipeSlot slot=\"{slot}\">: {} has no slot `{slot}` — {}", k.id, all.join(", ")));
                            ok = false;
                        }
                    }
                }
                LayoutPart::Item { item, .. } => {
                    if let Err(e) = known.check(refs::RefKind::Item, item) {
                        errors.push(format!("{shown}: <ItemSlot item=\"{item}\">: {e}"));
                        ok = false;
                    }
                }
                _ => {}
            }
        }
        if ok {
            out.push(l);
        }
    }
    out
}
