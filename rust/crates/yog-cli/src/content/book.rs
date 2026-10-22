//! `.ybook` files and `.yui` recipe layouts: checked, references resolved.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use yog_data::schema::{Resolve, Text};
use yog_data::with::With;
use yog_data::{Path as At, Ref, Res};
use yog_formats::book::layout::{LayoutPart, RecipeLayout};
use yog_formats::book::page_type::{CustomPage, OWN_FIELDS, PageType, PropKind};
use yog_formats::book::{Book, BookEntry, BookPage};
use yog_formats::def::RecipeKindDef;
use yog_ids::kind;

use crate::content::ctx::Ctx;
use crate::content::ctx::problem;
use crate::content::files::BookFile;
use crate::content::refs::{self, RefKind};
use crate::content::valid_id;
use crate::diagnostics::Diagnostic;

/// What a book is checked against: the ids it may name, how recipes are
/// shown, each recipe's kind (recipe → kind), the mod's kinds of page.
pub(super) struct BookAgainst<'a> {
    pub(super) known: &'a refs::Known,
    pub(super) layouts: &'a [RecipeLayout],
    pub(super) recipe_kinds: &'a HashMap<String, String>,
    pub(super) page_types: &'a [PageType],
}

/// Resolves what a book names: its own entries and categories, the
/// content it shows, its texts.
struct Check<'a> {
    known: &'a refs::Known,
    texts: &'a crate::yres::Refs,
    entries: HashSet<String>,
    categories: HashSet<String>,
    /// Whether a text is a `.yres` one: the book is then shown in the player's language.
    text_refs: bool,
}

impl Resolve for Check<'_> {
    fn reference(&mut self, r: &Ref, _: &At) -> Result<String, String> {
        let own = match r.kind.as_str() {
            kind::ENTRY => Some(&self.entries),
            kind::CATEGORY => Some(&self.categories),
            _ => None,
        };
        match (own, RefKind::from_word(&r.kind)) {
            (Some(set), _) if set.contains(&r.id) => Ok(r.id.clone()),
            (Some(_), _) => Err(format!("no {} {} in this book", r.kind, r.id)),
            (None, Some(kind)) => self.known.check(kind, &r.id).map(|()| r.id.clone()),
            (None, None) => Err(format!("`@{}:` names nothing a book shows", r.kind)),
        }
    }

    fn text(&mut self, t: &Text, _: &At) -> Result<String, String> {
        // A name: the game's translation of the item, block or entity — one that's there.
        if let Text::NameOf(r) = t {
            let kind = RefKind::from_word(&r.kind)
                .filter(|_| matches!(r.kind.as_str(), kind::ITEM | kind::BLOCK | kind::ENTITY))
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

/// The book's fields of entries, categories and pages.
const ENTRIES: &str = yog_data::field!(Book, entries);
const CATEGORIES: &str = yog_data::field!(Book, categories);
const PAGES: &str = yog_data::field!(BookEntry, pages);
const ID: &str = yog_data::field!(BookEntry, id);
/// A recipe page's recipe.
const RECIPE: &str = yog_ids::kind::RECIPE;

/// Where a line of the book is: the line a path's value is on.
type Line<'a> = &'a dyn Fn(&At) -> usize;

/// How a book's pages are read: where their lines are, what resolves them.
struct Pages<'a, 'k> {
    line: Line<'a>,
    check: &'a mut Check<'k>,
}

/// The page `p` of entry `e`.
fn page_at(e: usize, p: usize) -> At {
    At::root().key(ENTRIES).index(e).key(PAGES).index(p)
}

/// Checks one `.ybook` (`yog_formats::book::Book`'s schema), whose references
/// (`@item:minecraft:stick`, see `yog_formats::book::refs`) must name what exists
/// — in this mod, on every Minecraft version, or among the other mods'
/// ids `yog.toml` declares — and come out as plain ids. Texts may be
/// `@<Set>.<name>` (a `.yres` text), `@item:<id>` / `@block:<id>` /
/// `@entity:<id>` (its name) or `@@…` (a literal `@`); a book with text
/// references is shown in the player's language. An `id` without a
/// namespace gets the mod's.
pub(super) fn compile_book(cx: &mut Ctx, text: &str, against: &BookAgainst) -> Option<BookFile> {
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    // Entries and categories may name each other in any order: their ids first.
    let ids = |key: &str| -> HashSet<String> {
        doc.value
            .get(key)
            .and_then(|v| v.as_list())
            .unwrap_or_default()
            .iter()
            .filter_map(|e| e.get(ID)?.as_str().map(str::to_string))
            .collect()
    };
    let mut check = Check {
        known: against.known,
        texts: cx.refs,
        entries: ids(ENTRIES),
        categories: ids(CATEGORIES),
        text_refs: false,
    };
    let mut book = match yog_data::from_doc_with::<Book>(text, &doc, &mut check) {
        Ok(read) => read.value,
        Err(errors) => {
            cx.schema_errors(errors);
            return None;
        }
    };
    if !ids_namespaced(cx, &mut book) {
        return None;
    }
    let line = |path: &At| doc.locate(text, path).line as usize;
    let mut pages = Pages {
        line: &line,
        check: &mut check,
    };
    book.page_types = custom_pages(cx, &mut pages, &mut book, against.page_types)?;
    book.i18n |= check.text_refs;
    if !recipes_laid_out(cx, &line, &book, against) {
        return None;
    }
    book.layouts = against.layouts.to_vec();
    Some(BookFile {
        id: book.id.clone(),
        data: yog_data::binary::encode(&yog_data::Schema::write(&book)),
    })
}

/// The book's id with the mod's namespace when it has none, and valid;
/// its entries' and categories' ids namespaced (references to them always
/// are, so their ids must be too).
fn ids_namespaced(cx: &mut Ctx, book: &mut Book) -> bool {
    book.id = yog_ids::qualify_in(&cx.ns, &book.id);
    let categories = book
        .categories
        .iter()
        .enumerate()
        .map(|(i, c)| (At::root().key(CATEGORIES).index(i).key(ID), &c.id));
    let entries = book
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| (At::root().key(ENTRIES).index(i).key(ID), &e.id));
    let bare: Vec<String> = categories
        .chain(entries)
        .filter(|(_, id)| !id.contains(yog_ids::sep::NAMESPACE))
        .map(|(path, id)| {
            format!(
                "{path}: `{id}`: write its namespace (`{}`)",
                yog_ids::join(&cx.ns, id)
            )
        })
        .collect();
    let any_bare = !bare.is_empty();
    for e in bare {
        cx.err(1, e);
    }
    if any_bare {
        return false;
    }
    if !valid_id(&book.id) {
        cx.err(1, format!("book id `{}`: not a valid id", book.id));
        return false;
    }
    true
}

/// Pages of a mod's own kind: their fields checked by their type and
/// resolved like the rest of the book. The types they use, to go with it;
/// none when a page is wrong.
fn custom_pages(
    cx: &mut Ctx,
    pages: &mut Pages,
    book: &mut Book,
    types: &[PageType],
) -> Option<Vec<PageType>> {
    let mut used: Vec<PageType> = Vec::new();
    let mut ok = true;
    for (e, entry) in book.entries.iter_mut().enumerate() {
        for (p, page) in entry.pages.iter_mut().enumerate() {
            let BookPage::Custom(c) = page else { continue };
            let at = page_at(e, p);
            let Some(t) = types.iter().find(|t| t.id == c.kind) else {
                ok &= unknown_page_type(cx, pages.line, &at, &c.kind);
                continue;
            };
            ok &= page_fields(cx, pages, &at, c, t);
            if !used.contains(t) {
                used.push(t.clone());
            }
        }
    }
    ok.then_some(used)
}

/// A page of a type no `.yui` declares: wrong when it's the mod's own, not
/// checked when it's another mod's. Whether that's all right.
fn unknown_page_type(cx: &mut Ctx, line: Line, at: &At, kind: &str) -> bool {
    let ty = at.key(yog_ids::word::TYPE);
    if yog_ids::split(kind).0 == cx.ns {
        cx.err(
            line(&ty),
            format!("{ty}: no page type {kind} — a .yui component with book_page=\"{kind}\""),
        );
        false
    } else {
        cx.warn(
            line(&ty),
            format!("{ty}: {kind} is another mod's page type — not checked here"),
        );
        true
    }
}

/// A page's fields: each one its type has, given unless it has a default,
/// read as its kind. Whether they're all right.
fn page_fields(cx: &mut Ctx, pages: &mut Pages, at: &At, c: &mut CustomPage, t: &PageType) -> bool {
    let line = pages.line;
    let mut ok = true;
    for (k, _) in c.fields.iter() {
        if !OWN_FIELDS.contains(&k) && !t.properties.iter().any(|p| p.name == k) {
            let names: Vec<&str> = t.properties.iter().map(|p| p.name.as_str()).collect();
            cx.err(
                line(&at.key(k)),
                format!(
                    "{}: {} has no field `{k}` ({})",
                    at.key(k),
                    c.kind,
                    names.join(", ")
                ),
            );
            ok = false;
        }
    }
    for prop in &t.properties {
        let path = at.key(&prop.name);
        let Some(v) = c.fields.get(&prop.name).cloned() else {
            if prop.default.is_none() {
                cx.err(
                    line(at),
                    format!("{at}: missing `{}` ({})", prop.name, c.kind),
                );
                ok = false;
            }
            continue;
        };
        let (read, problems) = read_field(&v, prop.kind, pages.check);
        for pr in problems {
            cx.err(line(&path), format!("{path}: {pr}"));
            ok = false;
        }
        if let Some(out) = read {
            c.fields.insert(prop.name.as_str(), out);
        }
    }
    ok
}

/// A page field's value read as its kind, resolved like the book; and
/// what's wrong with it.
fn read_field(
    v: &yog_data::Value,
    kind: PropKind,
    check: &mut Check,
) -> (Option<yog_data::Value>, Vec<String>) {
    use yog_data::Value;
    use yog_data::with::{RefTo, TextOf};
    use yog_formats::book::refs::{BookText, Icon};
    let arena = yog_data::Bump::new();
    let r = v.borrow(&arena);
    let mut reader = yog_data::Reader::with_resolver(check);
    let read = match kind {
        PropKind::Text => TextOf::read(&r, &mut reader).map(Value::Str),
        PropKind::BookText => BookText::read(&r, &mut reader).map(Value::Str),
        PropKind::Item => {
            RefTo::<(yog_data::kinds::Item, yog_data::kinds::Block)>::read(&r, &mut reader)
                .map(Value::Str)
        }
        PropKind::Image => Icon::read(&r, &mut reader).map(Value::Str),
        PropKind::Number => <f64 as yog_data::Schema>::read(&r, &mut reader).map(Value::Float),
        PropKind::Bool => <bool as yog_data::Schema>::read(&r, &mut reader).map(Value::Bool),
        PropKind::Data => Some(v.clone()),
    };
    let problems = std::mem::take(&mut reader.problems)
        .into_iter()
        .map(|p| p.message)
        .collect();
    (read, problems)
}

/// Every recipe a page shows needs a layout for its kind (the game's are
/// built in). Whether they all have one.
fn recipes_laid_out(cx: &mut Ctx, line: Line, book: &Book, against: &BookAgainst) -> bool {
    let mut ok = true;
    for (e, entry) in book.entries.iter().enumerate() {
        for (p, page) in entry.pages.iter().enumerate() {
            if let BookPage::Recipe { recipe, .. } = page
                && let Some(kind) = against.recipe_kinds.get(recipe)
                && yog_formats::book::layout::find(against.layouts, kind).is_none()
            {
                let path = page_at(e, p).key(RECIPE);
                let short = yog_ids::split(kind).1;
                cx.err(
                    line(&path),
                    format!("{path}: no layout for recipe kind {kind} — lay it out in a .yui: <Component recipe_layout=\"{short}\" width=\"…\" height=\"…\"> (see yog_formats::book::layout)"),
                );
                ok = false;
            }
        }
    }
    ok
}

/// The mod's recipe layouts (`.yui` `<Component recipe_layout="…">`),
/// checked: each kind the game's or one the mod declares, laid out once;
/// a mod kind's slots its own; stations known items.
pub(super) fn recipe_layouts(
    root: &Path,
    ns: &str,
    kinds: &HashMap<String, RecipeKindDef>,
    known: &refs::Known,
    errors: &mut Vec<Diagnostic>,
) -> Vec<RecipeLayout> {
    let mut laid_out: HashMap<String, String> = HashMap::new();
    let mut out = Vec::new();
    for (shown, l) in crate::book_page::find_layouts(root, ns, errors) {
        let before = errors.len();
        layout_kinds(&shown, &l, kinds, &mut laid_out, errors);
        layout_parts(&shown, &l, kinds, known, errors);
        if errors.len() == before {
            out.push(l);
        }
    }
    out
}

/// A layout's kinds: the game's or the mod's, each laid out once.
fn layout_kinds(
    shown: &str,
    l: &RecipeLayout,
    kinds: &HashMap<String, RecipeKindDef>,
    laid_out: &mut HashMap<String, String>,
    errors: &mut Vec<Diagnostic>,
) {
    for k in &l.kinds {
        let game = yog_formats::recipe::TYPES.contains(&k.as_str());
        if !game && !kinds.contains_key(k) {
            errors.push(problem(format!("recipe_layout: `{k}`: one of {}, or a kind the mod declares (a .ydef of kind recipe_kind)",
                yog_formats::recipe::TYPES.join(", ")
            )).in_file(shown));
        }
        if let Some(other) = laid_out.insert(k.clone(), shown.to_string()) {
            errors.push(
                problem(format!("recipe kind {k} is also laid out by {other}")).in_file(shown),
            );
        }
    }
}

/// A layout's slots, each one its mod kinds have; its stations, known items.
fn layout_parts(
    shown: &str,
    l: &RecipeLayout,
    kinds: &HashMap<String, RecipeKindDef>,
    known: &refs::Known,
    errors: &mut Vec<Diagnostic>,
) {
    for part in &l.parts {
        match part {
            LayoutPart::Slots { slot, .. } => {
                for k in l.kinds.iter().filter_map(|k| kinds.get(k)) {
                    if !k.slots.0.iter().any(|(s, _)| s == slot) {
                        let all: Vec<&str> = k.slots.0.iter().map(|(s, _)| s.as_str()).collect();
                        errors.push(
                            problem(format!(
                                "<RecipeSlot slot=\"{slot}\">: {} has no slot `{slot}` — {}",
                                k.id,
                                all.join(", ")
                            ))
                            .in_file(shown),
                        );
                    }
                }
            }
            LayoutPart::Item { item, .. } => {
                if let Err(e) = known.check(RefKind::Item, item) {
                    errors.push(problem(format!("<ItemSlot item=\"{item}\">: {e}")).in_file(shown));
                }
            }
            _ => {}
        }
    }
}
