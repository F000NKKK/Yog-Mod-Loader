//! The `.ybook` designer's backend: the book drawn by yog-book itself —
//! the renderer the game runs — through the recording `YogGfxApi`, and
//! clicked through like a reader would (`docs/books.md`).
//!
//! Each open `.ybook` keeps its own reader (the page it's on); an edit
//! swaps the book under it without losing the place.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use yog_ids::Format;
use yog_sync::{LazyLock, Mutex};

use base64::Engine as _;
use serde_json::Value;
use yog_book::BookRenderer;
use yog_formats::book::Book;

use super::assets::Resources;
use super::{gfx, images, raster};

#[derive(Debug, Clone, yog_data::Schema)]
#[data(rename_all = "camelCase")]
pub struct BookArgs {
    pub path: String,
    /// The `.ybook` as it is in the editor.
    pub text: String,
    pub screen_w: u32,
    pub screen_h: u32,
    pub scale: u32,
    #[data(default)]
    pub action: Option<BookAction>,
    /// The language to read it in (`ru_ru`, …; none: the neutral texts).
    #[data(default)]
    pub locale: Option<String>,
}

/// What the reader does before the book is drawn.
#[derive(Debug, Clone, yog_data::Schema)]
#[data(tag = "kind", rename_all = "camelCase")]
pub enum BookAction {
    /// A click at GUI pixels `x, y`.
    Click { x: f32, y: f32 },
    /// Go to an entry's page (the document's selection).
    Show { entry: String, page: usize },
    /// Back to the landing page.
    Home,
}

#[derive(Debug, yog_data::Schema)]
#[data(rename_all = "camelCase")]
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
    /// The resolved book it was last given, as Yog Data.
    given: Vec<u8>,
}

static SESSIONS: LazyLock<Mutex<HashMap<String, Session>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The mod a content file belongs to: the folder with its `yog.toml`.
fn mod_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|p| p.join("yog.toml").is_file())
        .map(Path::to_path_buf)
}

/// The texts a player reading in `locale` sees: `Set.name` → text (for
/// `@Set.name`) and translation key → text (item and block names), from
/// the mod's `resources/*.yres` — `Set.<locale>.yres` over the neutral
/// `Set.yres` — and the game's language files under them.
pub(super) struct Texts {
    pub(super) refs: HashMap<String, String>,
    pub(super) lang: HashMap<String, String>,
}

pub(super) fn texts(mod_root: Option<&Path>, res: &Resources, locale: Option<&str>) -> Texts {
    let lang_file = locale.unwrap_or(yog_ids::game::LOCALE);
    let mod_id = mod_root
        .and_then(crate::design::project::mod_id)
        .unwrap_or_default();
    let mut t = Texts {
        refs: HashMap::new(),
        lang: game_lang(res, &mod_id, lang_file),
    };
    let Some(root) = mod_root else { return t };
    for (set, p) in yres_files(root, locale) {
        add_yres(&mut t, &set, &p, &mod_id);
    }
    t
}

/// The game's (and the mod's assets') language files, the mod's own on top,
/// and the book's own words as the loader's jars ship them.
fn game_lang(res: &Resources, mod_id: &str, lang_file: &str) -> HashMap<String, String> {
    let mut lang = HashMap::new();
    for ns in [yog_ids::ns::MINECRAFT, mod_id] {
        for file in [yog_ids::game::LOCALE, lang_file] {
            if let Some(Value::Object(map)) = res
                .read(ns, &yog_ids::lang_file(file))
                .and_then(|b| serde_json::from_slice(&b).ok())
            {
                for (k, v) in map {
                    if let Value::String(v) = v {
                        lang.insert(k, v);
                    }
                }
            }
        }
    }
    for file in [yog_ids::game::LOCALE, lang_file] {
        if let Some((_, json)) = yog_book::LANG.iter().find(|(l, _)| *l == file)
            && let Ok(map) = serde_json::from_str::<HashMap<String, String>>(json)
        {
            lang.extend(map);
        }
    }
    lang
}

/// The mod's `.yres` sets read in `locale`: (set, file), the neutral ones first.
fn yres_files(root: &Path, locale: Option<&str>) -> Vec<(String, PathBuf)> {
    let mut files: Vec<(String, Option<String>, PathBuf)> = Vec::new();
    for f in std::fs::read_dir(root.join(crate::yres::RESOURCES_DIR))
        .into_iter()
        .flatten()
        .flatten()
    {
        let p = f.path();
        let Some((set, loc)) = p
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(yog_formats::yres::split_file_name)
        else {
            continue;
        };
        let (set, loc) = (set.to_string(), loc.map(str::to_string));
        if loc.is_none() || loc.as_deref() == locale {
            files.push((set, loc, p));
        }
    }
    // Neutral first, the locale's over it.
    files.sort_by_key(|(_, loc, _)| loc.is_some());
    files.into_iter().map(|(set, _, p)| (set, p)).collect()
}

/// The texts of set `set`'s `.yres` at `file`, into `t`.
fn add_yres(t: &mut Texts, set: &str, file: &Path, mod_id: &str) {
    use yog_data::Value as V;
    let Some(doc) = crate::files::read_source(file)
        .ok()
        .and_then(|text| crate::design::data::read_object(&text))
    else {
        return;
    };
    let text_of = |o: &V, f: &str| o.get(f).and_then(V::as_str).map(str::to_string);
    for (k, v) in doc.as_map().into_iter().flatten() {
        let (text, key) = match v {
            V::Str(s) => (Some(s.clone()), None),
            V::Map(o)
                if o.get("type")
                    .and_then(V::as_str)
                    .is_none_or(|t| t == "text") =>
            {
                (text_of(v, "value"), text_of(v, "key"))
            }
            _ => (None, None),
        };
        let Some(text) = text else { continue };
        // As `yog build` names its translation key.
        let key = key.unwrap_or_else(|| crate::yres::default_key(set, mod_id, k));
        t.lang.insert(key, text.clone());
        t.refs.insert(format!("{set}.{k}"), text);
    }
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
                    yog_formats::yres::split_file_name(&name)?
                        .1
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

/// The game's name of an item or block: `item.<ns>.<path>`, else `block.…`.
pub(super) fn item_name(lang: &HashMap<String, String>, id: &str) -> Option<String> {
    [yog_ids::kind::ITEM, yog_ids::kind::BLOCK]
        .iter()
        .find_map(|k| lang.get(&yog_ids::lang_key(k, id)).cloned())
}

/// What the book's renderer looks names up in (it takes a plain `fn`).
static LANG: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn translate(key: &str) -> Option<String> {
    LANG.lock().get(key).cloned()
}

/// The mod's recipes, as the game gets them: `content/**/*.yrecipe` in
/// Yog's recipe format — (id, JSON).
fn recipes(mod_root: &Path) -> Vec<(String, yog_data::Value)> {
    let mod_id = crate::design::project::mod_id(mod_root).unwrap_or_default();
    let mut out = Vec::new();
    let mut stack = vec![mod_root.join(yog_ids::project::CONTENT)];
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
            let Some(mut v) = crate::files::read_source(&p)
                .ok()
                .and_then(|t| crate::design::data::read_object(&t))
            else {
                continue;
            };
            crate::design::data::runtime_ids(&mut v);
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let id = match v
                .get(yog_ids::content_field::ID)
                .and_then(yog_data::Value::as_str)
            {
                Some(id) if id.contains(':') => id.to_string(),
                Some(id) => format!("{mod_id}:{id}"),
                None => format!("{mod_id}:{stem}"),
            };
            out.push((id, v));
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
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
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
                    yog_ids::kind::ENTITY => self
                        .texts
                        .lang
                        .get(&yog_ids::lang_key(yog_ids::kind::ENTITY, id))
                        .cloned(),
                    _ => item_name(&self.texts.lang, id),
                };
                name.unwrap_or_else(|| humanize(id))
            }
            yog_data::Text::Lang(k) => self.texts.lang.get(k).cloned().unwrap_or_else(|| k.clone()),
            yog_data::Text::Res(r) => {
                self.texts.refs.get(&r.name()).cloned().unwrap_or_else(|| {
                    self.missing.push(r.to_string());
                    r.to_string()
                })
            }
        })
    }

    fn words(&mut self, r: &yog_data::Res) -> Option<String> {
        self.texts.refs.get(&r.name()).cloned()
    }
}

/// `v` without what the problem at `path` is about, so the rest can be
/// drawn: the page it's in, else the field (or an empty one if it's missing).
fn repair(v: &mut yog_data::Value, path: &yog_data::Path, message: &str) -> bool {
    use yog_data::{Seg, Value};
    let segs = path.segments();
    // A missing field: an empty one, in the object the problem is about.
    if let Some(field) = message
        .strip_prefix("missing `")
        .and_then(|r| r.split('`').next())
        && let Some(Value::Map(m)) = v.at_mut(segs)
        && m.get(field).is_none()
    {
        m.insert(field.to_string(), Value::from(""));
        return true;
    }
    // A problem inside a page: that page goes.
    if let Some(i) = segs
        .iter()
        .position(|s| matches!(s, Seg::Key(k) if k == "pages"))
        && let Some(Seg::Index(page)) = segs.get(i + 1)
    {
        return match v.at_mut(&segs[..=i]) {
            Some(Value::List(pages)) if *page < pages.len() => {
                pages.remove(*page);
                true
            }
            _ => false,
        };
    }
    // Else the field itself goes.
    let Some((last, parents)) = segs.split_last() else {
        return false;
    };
    match (last, v.at_mut(parents)) {
        (Seg::Key(k), Some(Value::Map(m))) => m.remove(k).is_some(),
        (Seg::Index(n), Some(Value::List(l))) if *n < l.len() => {
            l.remove(*n);
            true
        }
        _ => false,
    }
}

/// The `.ybook` text → the book a player would see, and the first problem
/// in it (what's wrong is left out of the drawing).
pub fn parse(
    text: &str,
    mod_root: Option<&Path>,
    locale: Option<&str>,
) -> Result<(Book, Option<String>), String> {
    let doc = yog_data::text::parse(text).map_err(|e| format!("{}: {}", e.at, e.message))?;
    let res = Resources::for_mod(mod_root);
    let texts = texts(mod_root, &res, locale);
    let mut value = doc.value.clone();
    let mut first: Option<String> = None;
    let mut reader = Reader {
        texts: &texts,
        missing: Vec::new(),
    };
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
                    first = Some(if p.path.is_root() {
                        format!("{at}: {}", p.message)
                    } else {
                        format!("{at}: {}: {}", p.path, p.message)
                    });
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
    book.layouts = mod_root
        .map(|r| {
            crate::design::mod_ui::recipe_layouts(r)
                .into_iter()
                .map(|(_, l)| l)
                .collect()
        })
        .unwrap_or_default();
    book.page_types = mod_root
        .map(crate::design::mod_ui::page_types)
        .unwrap_or_default();
    let first = first.or_else(|| {
        reader
            .missing
            .first()
            .map(|m| format!("{m}: no such text in resources/*.yres"))
    });
    let mut lang = LANG.lock();
    *lang = texts.lang;
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
static BOOK_MOD: yog_sync::Mutex<Option<PathBuf>> = yog_sync::Mutex::new(None);

/// An item tag's items for the book being drawn (`yog_book::set_tag_items`).
fn book_tag_items(tag: &str) -> Vec<String> {
    let root = BOOK_MOD.lock().clone();
    root.map(|r| super::tags::item_tag(&r.join(yog_ids::file::MANIFEST), tag))
        .unwrap_or_default()
}

fn render_book(
    args: &BookArgs,
    book: yog_formats::book::Book,
    problem: Option<String>,
    root: Option<&Path>,
    locales: Vec<String>,
) -> BookRendered {
    yog_book::set_tag_items(book_tag_items);
    let mut m = BOOK_MOD.lock();
    *m = root.map(Path::to_path_buf);
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
    let given = yog_data::to_bytes(&book);

    let mut sessions = SESSIONS.lock();
    let session = sessions.entry(args.path.clone()).or_insert_with(|| {
        let mut renderer = BookRenderer::new(book.clone());
        renderer.set_translator(translate);
        for (id, recipe) in root.map(recipes).unwrap_or_default() {
            renderer.add_recipe(id, &recipe);
        }
        Session {
            renderer,
            given: given.clone(),
        }
    });
    if session.given != given {
        session.renderer.set_book(book);
        session.given = given;
    }
    let r = &mut session.renderer;

    // Lay out first: a click hits what was last drawn at this size.
    gfx::record(sw as i32, sh as i32, |ctx| {
        r.render(ctx, sw as f32, sh as f32)
    });
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
    let ops = gfx::record(sw as i32, sh as i32, |ctx| {
        r.render(ctx, sw as f32, sh as f32)
    });
    let res = Resources::for_mod(root);
    let scale = raster::fit_scale(sw, sh, args.scale);
    let canvas = raster::rasterize(&ops, sw, sh, scale, &res);
    match canvas.png() {
        Ok(png) => BookRendered {
            image: Some(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png)
            )),
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
fn sample(p: &yog_formats::book::page_type::PageProperty) -> yog_data::Value {
    use yog_data::Value;
    use yog_formats::book::page_type::PropKind;
    if let Some(d) = &p.default {
        return d.clone();
    }
    match p.kind {
        PropKind::Text => Value::Str(format!("‹{}›", p.name)),
        PropKind::BookText => Value::Str(format!(
            "‹{}› — the page's text, as a book wraps it.",
            p.name
        )),
        PropKind::Number => Value::Int(1),
        PropKind::Bool => Value::Bool(true),
        PropKind::Item => yog_ids::game::BOOK.into(),
        PropKind::Image => yog_ids::join(yog_ids::image_scheme::ITEM, yog_ids::game::BOOK).into(),
        PropKind::Data => Value::Null,
    }
}

/// A `.yui` book page (`<Component book_page="…">`) drawn as the book shows
/// it: one entry of one page of the kind, its fields defaults or samples,
/// the kind as it is in the editor (not as saved).
pub fn page_preview(args: &BookArgs) -> BookRendered {
    let root = mod_root(Path::new(&args.path));
    let locales = locales(root.as_deref());
    let fail = |error: String| BookRendered {
        image: None,
        scale: args.scale,
        shown: None,
        action: None,
        animated: false,
        locales: locales.clone(),
        error: Some(error),
    };
    let ns = root
        .as_deref()
        .map(crate::design::mod_ui::ns)
        .unwrap_or_else(|| yog_ids::ns::MINECRAFT.into());
    let kind = match yog_formats::yui::parse(&args.text)
        .and_then(|el| yog_formats::book::page_type::compile(&el, &ns))
    {
        Ok(k) => k,
        Err(e) => return fail(e),
    };
    use yog_data::{Map, Value};
    let mut page = Map::from_iter([("type", Value::from(kind.id.as_str()))]);
    for p in &kind.properties {
        let v = sample(p);
        if !v.is_null() {
            page.insert(p.name.as_str(), v);
        }
    }
    let entry = format!("{ns}:page_preview");
    let named = |extra: Vec<(&str, Value)>| -> Value {
        let mut m = Map::from_iter([
            ("id", Value::from(entry.as_str())),
            ("name", kind.id.as_str().into()),
        ]);
        for (k, v) in extra {
            m.insert(k, v);
        }
        m.into()
    };
    let book = named(vec![
        ("categories", Value::List(vec![named(vec![])])),
        (
            "entries",
            Value::List(vec![named(vec![
                (
                    "category",
                    Value::Ref(yog_data::Ref {
                        kind: yog_ids::kind::CATEGORY.into(),
                        id: entry.clone(),
                    }),
                ),
                ("pages", Value::List(vec![page.into()])),
            ])]),
        ),
    ]);
    let text = yog_data::text::to_string(&book);
    let locale = args.locale.as_deref().filter(|l| !l.is_empty());
    let (mut book, problem) = match parse(&text, root.as_deref(), locale) {
        Ok(b) => b,
        Err(e) => return fail(e),
    };
    book.page_types.retain(|t| t.id != kind.id);
    book.page_types.push(kind);
    let args = BookArgs {
        action: Some(BookAction::Show { entry, page: 0 }),
        ..args.clone()
    };
    render_book(&args, book, problem, root.as_deref(), locales)
}

/// Forgets a closed `.ybook`'s reader.
pub fn close(path: &str) {
    SESSIONS.lock().remove(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_references_to_their_texts() {
        let mut texts = Texts {
            refs: HashMap::new(),
            lang: HashMap::new(),
        };
        texts
            .refs
            .insert("Book.title".to_string(), "Guide".to_string());
        texts
            .lang
            .insert("item.mymod.sapphire".to_string(), "Сапфир".to_string());
        let v = yog_data::text::parse_value(
            r#"{
            name: "@Book.title", subtitle: "@item:mymod:ruby_block", landing_text: "@@x",
            categories: [{ id: "m:c", name: "@item:mymod:sapphire", icon: "@item:mymod:sapphire" }],
            entries: [{ id: "m:e", name: "@Book.nope", category: "@category:m:c", pages: [] }],
            id: "m:b",
        }"#,
        )
        .unwrap();
        let mut r = Reader {
            texts: &texts,
            missing: Vec::new(),
        };
        let b = yog_data::from_value_with::<Book>(&v, &mut r)
            .unwrap_or_else(|p| panic!("{p:?}"))
            .value;
        assert_eq!(b.name, "Guide");
        assert_eq!(
            b.subtitle.as_deref(),
            Some("Ruby Block"),
            "no translation: made from the id"
        );
        assert_eq!(b.categories[0].name, "Сапфир", "a name in a text");
        assert_eq!(
            b.categories[0].icon.as_deref(),
            Some("mymod:sapphire"),
            "an id in an id field"
        );
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
        assert!(
            problem.starts_with("3:23: entries[0]: missing `name`"),
            "{problem}"
        );
        assert_eq!(
            book.entries[0].pages.len(),
            1,
            "the broken page is left out"
        );
    }

    #[test]
    fn draws_and_clicks_through_a_book() {
        let text = r#"{ "id": "t:guide", "name": "Guide",
            "categories": [{ "id": "t:c", "name": "Basics" }],
            "entries": [{ "id": "t:e", "name": "First", "category": "@category:t:c",
                "pages": [{ "type": "text", "text": "one" }, { "type": "text", "text": "two" }] }] }"#;
        let args = |action| BookArgs {
            path: "/tmp/test.ybook".into(),
            text: text.into(),
            screen_w: 400,
            screen_h: 240,
            scale: 1,
            action,
            locale: None,
        };
        let r = render(&args(None));
        assert!(r.error.is_none() && r.image.is_some(), "{:?}", r.error);
        assert_eq!(r.shown, None);
        let r = render(&args(Some(BookAction::Show {
            entry: "t:e".into(),
            page: 1,
        })));
        assert_eq!(r.shown, Some(("t:e".into(), 1)));
        let r = render(&args(Some(BookAction::Home)));
        assert_eq!(r.shown, None);
        close("/tmp/test.ybook");
    }

    #[test]
    fn a_book_page_component_is_drawn_as_the_book_shows_it() {
        let dir =
            std::env::temp_dir().join(format!("yog-idle-page-preview-{}", std::process::id()));
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
