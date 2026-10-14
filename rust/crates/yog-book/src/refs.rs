//! References in a book: what a `.ybook` names — items, blocks, entities,
//! recipes, its own entries and categories — it names through a typed
//! reference, `@item:minecraft:stick`, never a bare game id. The schema
//! ([`crate::Book`]'s `#[derive(Schema)]`) says which field takes what;
//! tools check every reference through a [`Resolve`] (`yog build`: does it
//! exist on every Minecraft version the mod targets?) and get the id the
//! game uses.
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
//! (`@Book.title`), names (`@item:mymod:ruby` shows the item's name) or
//! any translation key (`@lang:gui.done`), and
//! link entries with `{link=@entry:mymod:basics|text}`.

use yog_data::schema::{Read, Resolve, Text, read_ref};
use yog_data::with::With;
use yog_data::{Errors, Ref, Schema, Shape, Value, typed};

use crate::Book;

/// The id without a block state: `minecraft:furnace[facing=north]` → `minecraft:furnace`.
pub fn bare(id: &str) -> &str {
    id.split('[').next().unwrap_or(id)
}

/// Every `{link=…|` in `text` (the target as written).
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

/// A link's target as the reader follows it: `@entry:mymod:basics` → `mymod:basics`.
pub fn link_target(l: &str) -> &str {
    l.strip_prefix("@entry:").unwrap_or(l)
}

/// A shown text: as [`yog_data::with::TextOf`], and every link in it
/// (`{link=@entry:…|…}`, or a web address) checked.
pub struct BookText;

impl With<String> for BookText {
    fn shape() -> Shape {
        Shape::Text
    }

    fn read(v: &Value, cx: &mut yog_data::Reader) -> Option<String> {
        let t = Text::read(v, cx)?;
        let words = match &t {
            Text::Literal(s) => Some(s.clone()),
            Text::Res(r) => cx.words(r),
            Text::NameOf(_) | Text::Lang(_) => None,
        };
        let mut ok = true;
        for l in links(words.as_deref().unwrap_or_default()) {
            if l.starts_with("http://") || l.starts_with("https://") {
                continue;
            }
            match typed::classify(l) {
                Ok(Value::Ref(r)) if r.kind == "entry" => {
                    cx.note_ref(r.clone());
                    if let Err(e) = resolve_link(cx, &r) {
                        cx.problem(format!("link: {e}"));
                        ok = false;
                    }
                }
                _ => {
                    cx.problem(format!("link `{l}`: write {{link=@entry:ns:entry|…}}"));
                    ok = false;
                }
            }
        }
        let resolved = cx.resolve_text(&t)?;
        ok.then_some(resolved)
    }

    fn write(f: &String) -> Value {
        Value::Text(f.clone())
    }
}

/// A link's entry, checked like a reference (the problem is the caller's to word).
fn resolve_link(cx: &mut yog_data::Reader, r: &Ref) -> Result<(), String> {
    let before = cx.problems.len();
    let ok = cx.resolve_ref(r).is_some();
    if ok {
        return Ok(());
    }
    let p = cx.problems.remove(before);
    Err(p.message)
}

/// An icon: `@item:`/`@block:` (resolved to the id) or an image source as it is.
pub struct Icon;

impl With<String> for Icon {
    fn shape() -> Shape {
        Shape::RefOrImage(&["item", "block"])
    }

    fn read(v: &Value, cx: &mut yog_data::Reader) -> Option<String> {
        match v {
            Value::Str(s) if !s.starts_with('@') => Some(s.clone()),
            Value::Image(s) => Some(s.clone()),
            _ => {
                let r = read_ref(v, cx, &["item", "block"])?;
                cx.resolve_ref(&r)
            }
        }
    }

    fn write(f: &String) -> Value {
        Value::Str(f.clone())
    }
}

impl Book {
    /// Reads a `.ybook` (`text`), references and texts going through
    /// `resolver`: the book as the game shows it, and what it references.
    pub fn read(text: &str, resolver: &mut dyn Resolve) -> Result<Read<Book>, Errors> {
        let doc = yog_data::text::parse(text).map_err(|e| {
            Errors(vec![yog_data::schema::Located {
                at: Some(e.at),
                path: yog_data::Path::root(),
                message: e.message,
            }])
        })?;
        yog_data::from_doc_with(text, &doc, resolver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BookPage;
    use yog_data::Path;

    struct Check(Vec<String>);

    impl Resolve for Check {
        fn reference(&mut self, r: &Ref, _: &Path) -> Result<String, String> {
            self.0.push(format!("{}:{}", r.kind, r.id));
            if r.id == "minecraft:nope" {
                Err("no item minecraft:nope".into())
            } else {
                Ok(r.id.clone())
            }
        }
        fn text(&mut self, t: &Text, _: &Path) -> Result<String, String> {
            Ok(match t {
                Text::Res(r) if r.set == "Book" => format!("mymod.{}", r.key),
                Text::Literal(s) => s.clone(),
                other => format!("{other:?}"),
            })
        }
    }

    const BOOK: &str = r#"{
  id: "mymod:guide", name: "@Book.title",
  categories: [{ id: "mymod:basics", name: "Basics", icon: "@item:minecraft:book" }],
  entries: [{
    id: "mymod:ruby", name: "Ruby", category: "@category:mymod:basics", icon: "minecraft:textures/item/book.png",
    pages: [
      { type: "spotlight", item: "@block:minecraft:stone", text: "See {link=@entry:mymod:ruby|this}" },
      { type: "multiblock", pattern: [["A"]], mapping: { A: "@block:minecraft:furnace[facing=north]" } },
    ],
  }],
}"#;

    #[test]
    fn references_resolve_to_plain_ids() {
        let mut c = Check(Vec::new());
        let book = Book::read(BOOK, &mut c)
            .unwrap_or_else(|e| panic!("{e}"))
            .value;
        assert_eq!(book.name, "mymod.title");
        assert_eq!(book.categories[0].icon.as_deref(), Some("minecraft:book"));
        assert_eq!(book.entries[0].category, "mymod:basics");
        assert_eq!(
            book.entries[0].icon.as_deref(),
            Some("minecraft:textures/item/book.png"),
            "an image stays"
        );
        assert!(
            matches!(&book.entries[0].pages[0], BookPage::Spotlight { item, .. } if item == "minecraft:stone")
        );
        assert!(
            matches!(&book.entries[0].pages[1], BookPage::Multiblock { mapping, .. } if mapping["A"] == "minecraft:furnace[facing=north]")
        );
        assert!(
            c.0.contains(&"entry:mymod:ruby".to_string()),
            "links are checked"
        );
        assert_eq!(book.nameplate_color, "FFDD98", "defaults as the runtime's");
    }

    #[test]
    fn bare_ids_wrong_kinds_and_bad_links_are_errors() {
        let bad = r#"{
  id: "mymod:guide", name: "Guide",
  entries: [{ id: "e", name: "E", category: "@category:c", pages: [
    { type: "recipe", recipe: "minecraft:stick" },
    { type: "entity", entity: "@item:minecraft:nope" },
    { type: "text", text: "{link=items/staff|old}" },
  ]}],
}"#;
        let e = Book::read(bad, &mut Check(Vec::new()))
            .unwrap_err()
            .to_string();
        assert_eq!(
            e.lines().collect::<Vec<_>>(),
            [
                "4:31: entries[0].pages[0].recipe: `minecraft:stick`: name it by a reference — `@recipe:minecraft:stick`",
                "5:31: entries[0].pages[1].entity: `@item:minecraft:nope` is a reference to item — this takes `@entity:`",
                "6:27: entries[0].pages[2].text: link `items/staff`: write {link=@entry:ns:entry|…}",
            ]
        );
    }
}
