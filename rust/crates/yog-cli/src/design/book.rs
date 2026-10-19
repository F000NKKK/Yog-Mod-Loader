//! The `.ybook` designer's backend: the book drawn by yog-book itself —
//! the renderer the game runs — through the recording `YogGfxApi`, and
//! clicked through like a reader would (`docs/books.md`).
//!
//! Each open `.ybook` keeps its own reader (the page it's on); an edit
//! swaps the book under it without losing the place.

use yog_ids::Format;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use yog_book::BookRenderer;
use yog_formats::book::Book;

use super::assets::Resources;
use super::{gfx, images, raster};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookArgs {
    pub path: String,
    /// The `.ybook` as it is in the editor.
    pub text: String,
    pub screen_w: u32,
    pub screen_h: u32,
    pub scale: u32,
    #[serde(default)]
    pub action: Option<BookAction>,
    /// The language to read it in (`ru_ru`, …; none: the neutral texts).
    #[serde(default)]
    pub locale: Option<String>,
}

/// What the reader does before the book is drawn.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BookAction {
    /// A click at GUI pixels `x, y`.
    Click { x: f32, y: f32 },
    /// Go to an entry's page (the document's selection).
    Show { entry: String, page: usize },
    /// Back to the landing page.
    Home,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookRendered {
    /// `data:image/png;base64,…` — `scale` physical px per GUI px; `None`
    /// when the book doesn't parse.
    pub image: Option<String>,
    pub scale: u32,
    /// The entry and page shown (`None`: the landing or a list page).
    pub shown: Option<(String, usize)>,
    /// What a click did (`"next_page"`, `"entry:2"`, …).
    pub action: Option<String>,
    /// The page moves by itself (an entity, a structure): draw it again.
    pub animated: bool,
    /// The languages the mod's texts are in (besides the neutral one).
    pub locales: Vec<String>,
    pub error: Option<String>,
}

struct Session {
    renderer: BookRenderer,
    /// The resolved book JSON it was last given.
    json: String,
}

static SESSIONS: LazyLock<Mutex<HashMap<String, Session>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The mod a content file belongs to: the folder with its `yog.toml`.
fn mod_root(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|p| p.join("yog.toml").is_file()).map(Path::to_path_buf)
}

/// The texts a player reading in `locale` sees: `Set.name` → text (for
/// `@Set.name`) and translation key → text (item and block names), from
/// the mod's `resources/*.yres` — `Set.<locale>.yres` over the neutral
/// `Set.yres` — and the game's language files under them.
struct Texts {
    refs: HashMap<String, String>,
    lang: HashMap<String, String>,
}

fn texts(mod_root: Option<&Path>, res: &Resources, locale: Option<&str>) -> Texts {
    let mut t = Texts { refs: HashMap::new(), lang: HashMap::new() };
    // The game's (and the mod's assets') language files, the mod's own on top.
    let lang_file = locale.unwrap_or(yog_ids::game::LOCALE);
    let mod_id = mod_root.and_then(crate::design::project::mod_id).unwrap_or_default();
    for ns in [yog_ids::ns::MINECRAFT, mod_id.as_str()] {
        for file in [yog_ids::game::LOCALE, lang_file] {
            if let Some(Value::Object(map)) = res.read(ns, &yog_ids::lang_file(file)).and_then(|b| serde_json::from_slice(&b).ok()) {
                for (k, v) in map {
                    if let Value::String(v) = v {
                        t.lang.insert(k, v);
                    }
                }
            }
        }
    }
    // The book's own words, as the loader's jars ship them.
    for file in [yog_ids::game::LOCALE, lang_file] {
        if let Some((_, json)) = yog_book::LANG.iter().find(|(l, _)| *l == file)
            && let Ok(map) = serde_json::from_str::<HashMap<String, String>>(json)
        {
            t.lang.extend(map);
        }
    }
    let Some(root) = mod_root else { return t };
    let mut files: Vec<(String, Option<String>, PathBuf)> = Vec::new();
    for f in std::fs::read_dir(root.join("resources")).into_iter().flatten().flatten() {
        let p = f.path();
        let Some((set, loc)) = p.file_name().and_then(|n| n.to_str()).and_then(yog_formats::yres::split_file_name) else { continue };
        let (set, loc) = (set.to_string(), loc.map(str::to_string));
        if loc.is_none() || loc.as_deref() == locale {
            files.push((set, loc, p));
        }
    }
    // Neutral first, the locale's over it.
    files.sort_by_key(|(_, loc, _)| loc.is_some());
    for (set, _, p) in files {
        let Some(Value::Object(map)) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()) else {
            continue;
        };
        for (k, v) in map {
            let (text, key) = match &v {
                Value::String(s) => (Some(s.clone()), None),
                Value::Object(o) if o.get("type").is_none_or(|t| t == "text") => (
                    o.get("value").and_then(Value::as_str).map(str::to_string),
                    o.get("key").and_then(Value::as_str).map(str::to_string),
                ),
                _ => (None, None),
            };
            let Some(text) = text else { continue };
            // As `yog build` names its translation key.
            let key = key.unwrap_or_else(|| match set.as_str() {
                "Items" => format!("item.{mod_id}.{k}"),
                "Blocks" => format!("block.{mod_id}.{k}"),
                "Entities" => format!("entity.{mod_id}.{k}"),
                _ => format!("{mod_id}.{k}"),
            });
            t.lang.insert(key, text.clone());
            t.refs.insert(format!("{set}.{k}"), text);
        }
    }
    t
}

/// The languages the mod's `resources/*.yres` are translated into.
fn locales(mod_root: Option<&Path>) -> Vec<String> {
    let mut out: Vec<String> = mod_root
        .map(|r| {
            std::fs::read_dir(r.join("resources"))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|f| {
                    let name = f.file_name().to_string_lossy().into_owned();
                    yog_formats::yres::split_file_name(&name)?.1.map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

/// The game's name of an item or block: `item.<ns>.<path>`, else `block.…`.
fn item_name(lang: &HashMap<String, String>, id: &str) -> Option<String> {
    [yog_ids::kind::ITEM, yog_ids::kind::BLOCK].iter().find_map(|k| lang.get(&yog_ids::lang_key(k, id)).cloned())
}

/// What the book's renderer looks names up in (it takes a plain `fn`).
static LANG: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn translate(key: &str) -> Option<String> {
    LANG.lock().ok()?.get(key).cloned()
}

/// The mod's recipes, as the game gets them: `content/**/*.yrecipe` in
/// Yog's recipe format — (id, JSON).
fn recipes(mod_root: &Path) -> Vec<(String, yog_data::Value)> {
    let mod_id = crate::design::project::mod_id(mod_root).unwrap_or_default();
    let mut out = Vec::new();
    let mut stack = vec![mod_root.join("content")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !Format::Recipe.is(&p) {
                continue;
            }
            let Some(mut v) = std::fs::read_to_string(&p).ok().and_then(|t| crate::design::data::read_object(&t)).map(Value::Object) else {
                continue;
            };
            crate::design::data::runtime_ids(&mut v);
            let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let id = match v.get("id").and_then(Value::as_str) {
                Some(id) if id.contains(':') => id.to_string(),
                Some(id) => format!("{mod_id}:{id}"),
                None => format!("{mod_id}:{stem}"),
            };
            out.push((id, crate::design::data::from_json(&v)));
        }
    }
    out
}

/// `minecraft:amethyst_dust` → `Amethyst Dust`.
fn humanize(id: &str) -> String {
    let path = id.rsplit([':', '/']).next().unwrap_or(id);
    path.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The book as a player reads it: references (`@item:…` in id fields)
/// as plain ids, texts as their words (`@Book.title`) and names
/// (`@item:…`). Whether they exist is `yog check`'s to say; the designer
/// draws what it can.
struct Reader<'a> {
    texts: &'a Texts,
    missing: Vec<String>,
}

impl yog_data::Resolve for Reader<'_> {
    fn text(&mut self, t: &yog_data::Text, _: &yog_data::Path) -> Result<String, String> {
        Ok(match t {
            yog_data::Text::Literal(s) => s.clone(),
            yog_data::Text::NameOf(r) => {
                let id = yog_formats::book::refs::bare(&r.id);
                let name = match r.kind.as_str() {
                    yog_ids::kind::ENTITY => self.texts.lang.get(&yog_ids::lang_key(yog_ids::kind::ENTITY, id)).cloned(),
                    _ => item_name(&self.texts.lang, id),
                };
                name.unwrap_or_else(|| humanize(id))
            }
            yog_data::Text::Lang(k) => self.texts.lang.get(k).cloned().unwrap_or_else(|| k.clone()),
            yog_data::Text::Res(r) => {
                let key = format!("{}.{}", r.set, r.key);
                self.texts.refs.get(&key).cloned().unwrap_or_else(|| {
                    self.missing.push(format!("@{key}"));
                    format!("@{key}")
                })
            }
        })
    }

    fn words(&mut self, r: &yog_data::Res) -> Option<String> {
        self.texts.refs.get(&format!("{}.{}", r.set, r.key)).cloned()
    }
}

/// `v` without what the problem at `path` is about, so the rest can be
/// drawn: the page it's in, else the field (or an empty one if it's missing).
fn repair(v: &mut yog_data::Value, path: &yog_data::Path, message: &str) -> bool {
    use yog_data::{Seg, Value};
    let segs = path.segments();
    // A missing field: an empty one, in the object the problem is about.
    if let Some(field) = message.strip_prefix("missing `").and_then(|r| r.split('`').next()) {
        let mut at = &mut *v;
        for seg in segs {
            at = match (seg, at) {
                (Seg::Key(k), Value::Map(m)) => match m.get_mut(k) {
                    Some(x) => x,
                    None => return false,
                },
                (Seg::Index(n), Value::List(l)) => match l.get_mut(*n) {
                    Some(x) => x,
                    None => return false,
                },
                _ => return false,
            };
        }
        if let Value::Map(m) = at
            && m.get(field).is_none()
        {
            m.insert(field.to_string(), Value::from(""));
            return true;
        }
    }
    if let Some(i) = segs.iter().position(|s| matches!(s, Seg::Key(k) if k == "pages"))
        && let Some(Seg::Index(page)) = segs.get(i + 1)
    {
        let mut at = &mut *v;
        for seg in &segs[..=i] {
            at = match (seg, at) {
                (Seg::Key(k), Value::Map(m)) => match m.get_mut(k) {
                    Some(x) => x,
                    None => return false,
                },
                (Seg::Index(n), Value::List(l)) => match l.get_mut(*n) {
                    Some(x) => x,
                    None => return false,
                },
                _ => return false,
            };
        }
        if let Value::List(pages) = at
            && *page < pages.len()
        {
            pages.remove(*page);
            return true;
        }
        return false;
    }
    let mut at = &mut *v;
    let (last, parents) = match segs.split_last() {
        Some(x) => x,
        None => return false,
    };
    for seg in parents {
        at = match (seg, at) {
            (Seg::Key(k), Value::Map(m)) => match m.get_mut(k) {
                Some(x) => x,
                None => return false,
            },
            (Seg::Index(n), Value::List(l)) => match l.get_mut(*n) {
                Some(x) => x,
                None => return false,
            },
            _ => return false,
        };
    }
    match (last, at) {
        (Seg::Key(k), Value::Map(m)) => m.remove(k).is_some(),
        (Seg::Index(n), Value::List(l)) if *n < l.len() => {
            l.remove(*n);
            true
        }
        _ => false,
    }
}

/// The `.ybook` text → the book a player would see, and the first problem
/// in it (what's wrong is left out of the drawing).
pub fn parse(text: &str, mod_root: Option<&Path>, locale: Option<&str>) -> Result<(Book, Option<String>), String> {
    let doc = yog_data::text::parse(text).map_err(|e| format!("{}: {}", e.at, e.message))?;
    let res = Resources::for_mod(mod_root);
    let texts = texts(mod_root, &res, locale);
    let mut value = doc.value.clone();
    let mut first: Option<String> = None;
    let mut reader = Reader { texts: &texts, missing: Vec::new() };
    let mut book = None;
    for _ in 0..32 {
        match yog_data::from_value_with::<Book>(&value, &mut reader) {
            Ok(read) => {
                book = Some(read.value);
                break;
            }
            Err(problems) => {
                if first.is_none()
                    && let Some(p) = problems.first()
                {
                    let at = doc.locate(text, &p.path);
                    first = Some(if p.path.is_root() { format!("{at}: {}", p.message) } else { format!("{at}: {}: {}", p.path, p.message) });
                }
                // Missing fields of the book itself are filled in first, then the rest repaired.
                let mut changed = false;
                for p in &problems {
                    changed |= repair(&mut value, &p.path, &p.message);
                }
                if !changed {
                    break;
                }
            }
        }
    }
    let mut book = book.ok_or_else(|| first.clone().unwrap_or_else(|| "not a book".into()))?;
    // The reader shows texts, not keys.
    book.i18n = false;
    // The mod's own recipe layouts and kinds of page, as the build adds them.
    book.layouts = mod_root.map(|r| crate::design::mod_ui::recipe_layouts(r).into_iter().map(|(_, l)| l).collect()).unwrap_or_default();
    book.page_types = mod_root.map(crate::design::mod_ui::page_types).unwrap_or_default();
    let first = first.or_else(|| reader.missing.first().map(|m| format!("{m}: no such text in resources/*.yres")));
    if let Ok(mut lang) = LANG.lock() {
        *lang = texts.lang;
    }
    Ok((book, first))
}

pub fn render(args: &BookArgs) -> BookRendered {
    let root = mod_root(Path::new(&args.path));
    let locales = locales(root.as_deref());
    let locale = args.locale.as_deref().filter(|l| !l.is_empty());
    let fail = |error: String| BookRendered {
        image: None,
        scale: args.scale,
        shown: None,
        action: None,
        animated: false,
        locales: locales.clone(),
        error: Some(error),
    };
    let (book, problem) = match parse(&args.text, root.as_deref(), locale) {
        Ok(b) => b,
        Err(e) => return fail(e),
    };
    render_book(args, book, problem, root.as_deref(), locales)
}

/// Draws `book` (already read) for `args`: its session's reader, the action.
/// The mod whose book is being drawn: where its tags come from.
static BOOK_MOD: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// An item tag's items for the book being drawn (`yog_book::set_tag_items`).
fn book_tag_items(tag: &str) -> Vec<String> {
    let root = BOOK_MOD.lock().ok().and_then(|r| r.clone());
    root.map(|r| super::tags::item_tag(&r.join(yog_ids::file::MANIFEST), tag)).unwrap_or_default()
}

fn render_book(args: &BookArgs, book: yog_formats::book::Book, problem: Option<String>, root: Option<&Path>, locales: Vec<String>) -> BookRendered {
    yog_book::set_tag_items(book_tag_items);
    if let Ok(mut m) = BOOK_MOD.lock() {
        *m = root.map(Path::to_path_buf);
    }
    let (sw, sh) = (args.screen_w.clamp(1, 4096), args.screen_h.clamp(1, 4096));
    let fail = |error: String| BookRendered {
        image: None,
        scale: args.scale,
        shown: None,
        action: None,
        animated: false,
        locales: locales.clone(),
        error: Some(error),
    };
    let json = serde_json::to_string(&book).unwrap_or_default();

    let mut sessions = SESSIONS.lock().unwrap_or_else(|e| e.into_inner());
    let session = sessions.entry(args.path.clone()).or_insert_with(|| {
        let mut renderer = BookRenderer::new(book.clone());
        renderer.set_translator(translate);
        for (id, recipe) in root.map(recipes).unwrap_or_default() {
            renderer.add_recipe(id, &recipe);
        }
        Session { renderer, json: json.clone() }
    });
    if session.json != json {
        session.renderer.set_book(book);
        session.json = json;
    }
    let r = &mut session.renderer;

    // Lay out first: a click hits what was last drawn at this size.
    gfx::record(sw as i32, sh as i32, |ctx| r.render(ctx, sw as f32, sh as f32));
    let action = match &args.action {
        Some(BookAction::Click { x, y }) => r.click(*x, *y),
        Some(BookAction::Show { entry, page }) => {
            r.show(entry, *page);
            None
        }
        Some(BookAction::Home) => {
            r.handle_event("home");
            None
        }
        None => None,
    };

    images::set_document(root);
    let ops = gfx::record(sw as i32, sh as i32, |ctx| r.render(ctx, sw as f32, sh as f32));
    let res = Resources::for_mod(root);
    let scale = raster::fit_scale(sw, sh, args.scale);
    let canvas = raster::rasterize(&ops, sw, sh, scale, &res);
    match canvas.png() {
        Ok(png) => BookRendered {
            image: Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))),
            scale,
            shown: r.shown(),
            action,
            animated: r.animated(),
            locales,
            error: problem,
        },
        Err(e) => fail(e),
    }
}

/// What a page of a mod's kind shows when the designer has no page to read:
/// a property's default, else a sample of its kind.
fn sample(p: &yog_formats::book::page_type::PageProperty) -> serde_json::Value {
    use yog_formats::book::page_type::PropKind;
    if let Some(d) = &p.default {
        return yog_data::json::to_json(d);
    }
    match p.kind {
        PropKind::Text => serde_json::Value::String(format!("‹{}›", p.name)),
        PropKind::BookText => serde_json::Value::String(format!("‹{}› — the page's text, as a book wraps it.", p.name)),
        PropKind::Number => serde_json::json!(1),
        PropKind::Bool => serde_json::json!(true),
        PropKind::Item => serde_json::json!(yog_ids::game::BOOK),
        PropKind::Image => serde_json::json!(yog_ids::join(yog_ids::image_scheme::ITEM, yog_ids::game::BOOK)),
        PropKind::Data => serde_json::Value::Null,
    }
}

/// A `.yui` book page (`<Component book_page="…">`) drawn as the book shows
/// it: one entry of one page of the kind, its fields defaults or samples,
/// the kind as it is in the editor (not as saved).
pub fn page_preview(args: &BookArgs) -> BookRendered {
    let root = mod_root(Path::new(&args.path));
    let locales = locales(root.as_deref());
    let fail = |error: String| BookRendered { image: None, scale: args.scale, shown: None, action: None, animated: false, locales: locales.clone(), error: Some(error) };
    let ns = root.as_deref().map(crate::design::mod_ui::ns).unwrap_or_else(|| yog_ids::ns::MINECRAFT.into());
    let kind = match yog_formats::yui::parse(&args.text).and_then(|el| yog_formats::book::page_type::compile(&el, &ns)) {
        Ok(k) => k,
        Err(e) => return fail(e),
    };
    let mut page = serde_json::Map::new();
    page.insert("type".into(), serde_json::Value::String(kind.id.clone()));
    for p in &kind.properties {
        let v = sample(p);
        if !v.is_null() {
            page.insert(p.name.clone(), v);
        }
    }
    let entry = format!("{ns}:page_preview");
    let text = serde_json::json!({
        "id": format!("{ns}:page_preview"),
        "name": kind.id,
        "categories": [{ "id": entry, "name": kind.id }],
        "entries": [{ "id": entry, "name": kind.id, "category": format!("@category:{entry}"), "pages": [page] }],
    })
    .to_string();
    let locale = args.locale.as_deref().filter(|l| !l.is_empty());
    let (mut book, problem) = match parse(&text, root.as_deref(), locale) {
        Ok(b) => b,
        Err(e) => return fail(e),
    };
    book.page_types.retain(|t| t.id != kind.id);
    book.page_types.push(kind);
    let args = BookArgs { action: Some(BookAction::Show { entry, page: 0 }), ..args.clone() };
    render_book(&args, book, problem, root.as_deref(), locales)
}

/// Forgets a closed `.ybook`'s reader.
pub fn close(path: &str) {
    SESSIONS.lock().unwrap_or_else(|e| e.into_inner()).remove(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_references_to_their_texts() {
        let mut texts = Texts { refs: HashMap::new(), lang: HashMap::new() };
        texts.refs.insert("Book.title".to_string(), "Guide".to_string());
        texts.lang.insert("item.mymod.sapphire".to_string(), "Сапфир".to_string());
        let v = yog_data::text::parse_value(
            r#"{
            name: "@Book.title", subtitle: "@item:mymod:ruby_block", landing_text: "@@x",
            categories: [{ id: "m:c", name: "@item:mymod:sapphire", icon: "@item:mymod:sapphire" }],
            entries: [{ id: "m:e", name: "@Book.nope", category: "@category:m:c", pages: [] }],
            id: "m:b",
        }"#,
        )
        .unwrap();
        let mut r = Reader { texts: &texts, missing: Vec::new() };
        let b = yog_data::from_value_with::<Book>(&v, &mut r).unwrap_or_else(|p| panic!("{p:?}")).value;
        assert_eq!(b.name, "Guide");
        assert_eq!(b.subtitle.as_deref(), Some("Ruby Block"), "no translation: made from the id");
        assert_eq!(b.categories[0].name, "Сапфир", "a name in a text");
        assert_eq!(b.categories[0].icon.as_deref(), Some("mymod:sapphire"), "an id in an id field");
        assert_eq!(b.entries[0].category, "m:c");
        assert_eq!(b.landing_text, "@x");
        assert_eq!(r.missing, ["@Book.nope"]);
    }

    #[test]
    fn draws_what_it_can_of_a_broken_book() {
        let text = r#"{ id: "t:guide", name: "Guide",
            categories: [{ id: "t:c", name: "C" }],
            entries: [{ id: "t:e", category: "@category:t:c", pages: [
                { type: "spotlight", item: "" },
                { type: "text", text: "kept" },
            ] }] }"#;
        let (book, problem) = parse(text, None, None).unwrap();
        let problem = problem.unwrap();
        assert!(problem.starts_with("3:23: entries[0]: missing `name`"), "{problem}");
        assert_eq!(book.entries[0].pages.len(), 1, "the broken page is left out");
    }

    #[test]
    fn draws_and_clicks_through_a_book() {
        let text = r#"{ "id": "t:guide", "name": "Guide",
            "categories": [{ "id": "t:c", "name": "Basics" }],
            "entries": [{ "id": "t:e", "name": "First", "category": "@category:t:c",
                "pages": [{ "type": "text", "text": "one" }, { "type": "text", "text": "two" }] }] }"#;
        let args = |action| BookArgs { path: "/tmp/test.ybook".into(), text: text.into(), screen_w: 400, screen_h: 240, scale: 1, action, locale: None };
        let r = render(&args(None));
        assert!(r.error.is_none() && r.image.is_some(), "{:?}", r.error);
        assert_eq!(r.shown, None);
        let r = render(&args(Some(BookAction::Show { entry: "t:e".into(), page: 1 })));
        assert_eq!(r.shown, Some(("t:e".into(), 1)));
        let r = render(&args(Some(BookAction::Home)));
        assert_eq!(r.shown, None);
        close("/tmp/test.ybook");
    }

    #[test]
    fn a_book_page_component_is_drawn_as_the_book_shows_it() {
        let dir = std::env::temp_dir().join(format!("yog-idle-page-preview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src/ui")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        let yui = r#"<Component name="Note" book_page="note">
            <Property name="title" type="text"/>
            <Property name="body" type="book_text" default="Hello"/>
            <BookHeader text="{x:Bind title}"/>
            <BookText text="{x:Bind body}"/>
            <Drawn drawer="mymod:thing" h="32" data="{x:Bind title}"/>
        </Component>"#;
        let args = BookArgs {
            path: dir.join("src/ui/note.yui").to_string_lossy().into_owned(),
            text: yui.into(),
            screen_w: 480,
            screen_h: 270,
            scale: 1,
            action: None,
            locale: None,
        };
        let r = page_preview(&args);
        assert!(r.error.is_none(), "{:?}", r.error);
        assert!(r.image.is_some());
        assert_eq!(r.shown, Some(("mymod:page_preview".to_string(), 0)));
        let _ = std::fs::remove_dir_all(&dir);
    }

}
