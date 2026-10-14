//! References in a book: what a `.ybook` names — items, blocks, entities,
//! recipes, its own entries and categories — it names through a typed
//! reference, `@item:minecraft:stick`, never a bare game id. Tools check
//! every reference (`yog build`: does it exist on every Minecraft version
//! the mod targets?) and turn it into the id the game uses.
//!
//! | Field | Takes |
//! |---|---|
//! | `entries[].category` | `@category:` |
//! | `icon` (categories, entries) | `@item:`, `@block:`, or an image (`ns:textures/x.png`) |
//! | `spotlight.item` | `@item:`, `@block:` |
//! | `recipe.recipe` | `@recipe:` |
//! | `entity.entity` | `@entity:` |
//! | `multiblock.mapping` values | `@block:`, with a state: `@block:minecraft:furnace[facing=north]` |
//! | `relations.entries` | `@entry:` |
//!
//! Texts (names, titles, page texts, …) may be whole references to texts
//! (`@Book.title`) or names (`@item:mymod:ruby` shows the item's name), and
//! link entries with `{link=@entry:mymod:basics|text}`.

use serde_json::Value;

/// What a reference names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    Item,
    Block,
    Entity,
    Recipe,
    Entry,
    Category,
}

impl RefKind {
    pub const ALL: [RefKind; 6] =
        [RefKind::Item, RefKind::Block, RefKind::Entity, RefKind::Recipe, RefKind::Entry, RefKind::Category];

    /// Its prefix: `item` for `@item:`.
    pub fn word(self) -> &'static str {
        match self {
            RefKind::Item => "item",
            RefKind::Block => "block",
            RefKind::Entity => "entity",
            RefKind::Recipe => "recipe",
            RefKind::Entry => "entry",
            RefKind::Category => "category",
        }
    }
}

/// `@item:minecraft:stick` → (Item, `minecraft:stick`). A block's state
/// (`[facing=north]`) stays on the id.
pub fn parse(s: &str) -> Option<(RefKind, &str)> {
    let rest = s.trim().strip_prefix('@')?;
    let (word, id) = rest.split_once(':')?;
    let kind = RefKind::ALL.into_iter().find(|k| k.word() == word)?;
    (!id.is_empty()).then_some((kind, id))
}

/// The id without a block state: `minecraft:furnace[facing=north]` → `minecraft:furnace`.
pub fn bare(id: &str) -> &str {
    id.split('[').next().unwrap_or(id)
}

/// What one field of a book holds.
#[derive(Clone, Copy)]
enum Field {
    /// Shown text.
    Text,
    /// A reference of one of these kinds.
    Ref(&'static [RefKind]),
    /// `@item:`/`@block:`, or an image source as it is.
    Icon,
    /// Anything else (numbers, colors, ids it defines, image sources).
    Plain,
}

const ITEMS: &[RefKind] = &[RefKind::Item, RefKind::Block];

/// What the field `key` of a page of type `page` (none: not a page) holds.
fn field(page: Option<&str>, key: &str) -> Field {
    match (page, key) {
        (_, "name" | "title" | "text" | "description" | "landing_text" | "subtitle" | "input" | "output") => Field::Text,
        (None, "icon") => Field::Icon,
        (None, "category") => Field::Ref(&[RefKind::Category]),
        (Some("spotlight"), "item") => Field::Ref(ITEMS),
        (Some("recipe"), "recipe") => Field::Ref(&[RefKind::Recipe]),
        (Some("entity"), "entity") => Field::Ref(&[RefKind::Entity]),
        (Some("multiblock"), "mapping") => Field::Ref(&[RefKind::Block]),
        (Some("relations"), "entries") => Field::Ref(&[RefKind::Entry]),
        _ => Field::Plain,
    }
}

/// What a tool does with a book's references and texts.
pub trait Resolver {
    /// A reference of `kind` to `id` at `path` (`entries[2].pages[0].item`):
    /// the id the game uses, or why it's wrong.
    fn reference(&mut self, kind: RefKind, id: &str, path: &str) -> Result<String, String>;
    /// A text: what to put instead (a translation key for `@Book.title`, a
    /// name for `@item:…`), `None` to keep it. Its links are checked
    /// through [`Resolver::reference`] already.
    fn text(&mut self, text: &str, path: &str) -> Option<String>;
    /// The text a whole-text reference stands for (`@Book.title` → its
    /// words), to check the links in it; `None` if unknown.
    fn text_of(&mut self, _text: &str) -> Option<String> {
        None
    }
}

/// Every `{link=@entry:…|` in `text`: (the reference as written).
pub fn links(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("{link=") {
        rest = &rest[i + 6..];
        let end = rest.find(['|', ' ', '}']).unwrap_or(rest.len());
        out.push(&rest[..end]);
        rest = &rest[end..];
    }
    out
}

/// Resolves every reference and text of `book` (a `.ybook` as JSON) in
/// place; returns what's wrong, each with its path.
pub fn resolve(book: &mut Value, r: &mut dyn Resolver) -> Vec<String> {
    let mut errors = Vec::new();
    walk(book, None, "", r, &mut errors);
    errors
}

fn walk(v: &mut Value, page: Option<&str>, path: &str, r: &mut dyn Resolver, errors: &mut Vec<String>) {
    let Value::Object(map) = v else { return };
    let page_type = map.get("type").and_then(Value::as_str).map(str::to_string);
    let page = page.or(page_type.as_deref()).map(str::to_string);
    for (key, val) in map.iter_mut() {
        let here = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
        match key.as_str() {
            "categories" | "entries" if page.is_none() => {
                if let Value::Array(items) = val {
                    for (i, item) in items.iter_mut().enumerate() {
                        walk(item, None, &format!("{here}[{i}]"), r, errors);
                    }
                }
                continue;
            }
            "pages" => {
                if let Value::Array(items) = val {
                    for (i, item) in items.iter_mut().enumerate() {
                        let t = item.get("type").and_then(Value::as_str).unwrap_or_default().to_string();
                        walk(item, Some(&t), &format!("{here}[{i}]"), r, errors);
                    }
                }
                continue;
            }
            _ => {}
        }
        match field(page.as_deref(), key) {
            Field::Text => text(val, &here, r, errors),
            Field::Ref(kinds) => each_string(val, &here, &mut |s, at| reference(s, kinds, at, r, errors)),
            Field::Icon => {
                if let Value::String(s) = val
                    && s.starts_with('@')
                {
                    *s = reference(s, ITEMS, &here, r, errors);
                }
            }
            Field::Plain => {}
        }
    }
}

/// `f` on every string of `v`: itself, an array's, an object's values.
fn each_string(v: &mut Value, path: &str, f: &mut dyn FnMut(&str, &str) -> String) {
    match v {
        Value::String(s) => *s = f(s, path),
        Value::Array(items) => {
            for (i, item) in items.iter_mut().enumerate() {
                each_string(item, &format!("{path}[{i}]"), f);
            }
        }
        Value::Object(map) => {
            for (k, item) in map.iter_mut() {
                each_string(item, &format!("{path}.{k}"), f);
            }
        }
        _ => {}
    }
}

fn reference(s: &str, kinds: &[RefKind], path: &str, r: &mut dyn Resolver, errors: &mut Vec<String>) -> String {
    let wanted = || kinds.iter().map(|k| format!("@{}:", k.word())).collect::<Vec<_>>().join(" or ");
    match parse(s) {
        Some((kind, id)) if kinds.contains(&kind) => match r.reference(kind, id, path) {
            Ok(resolved) => resolved,
            Err(e) => {
                errors.push(format!("{path}: {e}"));
                s.to_string()
            }
        },
        Some((kind, _)) => {
            errors.push(format!("{path}: `{s}` is a reference to {} — this takes {}", kind.word(), wanted()));
            s.to_string()
        }
        None => {
            errors.push(format!("{path}: `{s}`: name it by a reference — `@{}:{}`", kinds[0].word(), s.trim_start_matches('@')));
            s.to_string()
        }
    }
}

fn text(v: &mut Value, path: &str, r: &mut dyn Resolver, errors: &mut Vec<String>) {
    let Value::String(s) = v else { return };
    let words = if s.starts_with('@') && !s.starts_with("@@") { r.text_of(s) } else { None };
    for l in links(words.as_deref().unwrap_or(s)) {
        match parse(l) {
            Some((RefKind::Entry, id)) => {
                if let Err(e) = r.reference(RefKind::Entry, id, path) {
                    errors.push(format!("{path}: link: {e}"));
                }
            }
            // A web address links outside the book.
            _ if l.starts_with("http://") || l.starts_with("https://") => {}
            _ => errors.push(format!("{path}: link `{l}`: write {{link=@entry:ns:entry|…}}")),
        }
    }
    if let Some(t) = r.text(s, path) {
        *s = t;
    }
}

/// A link's target as the reader follows it: `@entry:mymod:basics` → `mymod:basics`.
pub fn link_target(l: &str) -> &str {
    match parse(l) {
        Some((RefKind::Entry, id)) => id,
        _ => l,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Check(Vec<(RefKind, String)>);

    impl Resolver for Check {
        fn reference(&mut self, kind: RefKind, id: &str, _path: &str) -> Result<String, String> {
            self.0.push((kind, id.to_string()));
            if id == "minecraft:nope" { Err("no item minecraft:nope".into()) } else { Ok(id.to_string()) }
        }
        fn text(&mut self, text: &str, _path: &str) -> Option<String> {
            text.strip_prefix("@Book.").map(|k| format!("mymod.{k}"))
        }
    }

    #[test]
    fn references_resolve_and_bare_ids_are_errors() {
        let mut book = serde_json::json!({
            "id": "mymod:guide", "name": "@Book.title",
            "categories": [{ "id": "mymod:basics", "name": "Basics", "icon": "@item:minecraft:book" }],
            "entries": [{
                "id": "mymod:ruby", "name": "Ruby", "category": "@category:mymod:basics", "icon": "minecraft:textures/item/book.png",
                "pages": [
                    { "type": "spotlight", "item": "@block:minecraft:stone", "text": "See {link=@entry:mymod:ruby|this}" },
                    { "type": "recipe", "recipe": "minecraft:stick" },
                    { "type": "multiblock", "pattern": [["A"]], "mapping": { "A": "@block:minecraft:furnace[facing=north]" } },
                    { "type": "entity", "entity": "@item:minecraft:nope" },
                    { "type": "text", "text": "{link=items/staff|old}" }
                ]
            }]
        });
        let mut c = Check(Vec::new());
        let errors = resolve(&mut book, &mut c);
        assert_eq!(book["name"], "mymod.title");
        assert_eq!(book["categories"][0]["icon"], "minecraft:book");
        assert_eq!(book["entries"][0]["category"], "mymod:basics");
        assert_eq!(book["entries"][0]["icon"], "minecraft:textures/item/book.png", "an image stays");
        assert_eq!(book["entries"][0]["pages"][0]["item"], "minecraft:stone");
        assert_eq!(book["entries"][0]["pages"][2]["mapping"]["A"], "minecraft:furnace[facing=north]");
        assert!(c.0.contains(&(RefKind::Entry, "mymod:ruby".into())), "links are checked");
        assert_eq!(errors.len(), 3, "{errors:#?}");
        assert!(errors[0].contains("entries[0].pages[1].recipe") && errors[0].contains("`@recipe:minecraft:stick`"));
        assert!(errors[1].contains("is a reference to item"));
        assert!(errors[2].contains("link `items/staff`"));
    }
}
