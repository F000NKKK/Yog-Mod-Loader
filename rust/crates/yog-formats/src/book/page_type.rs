//! A mod's own kinds of page (`hexcasting:pattern`): a `.yui` component
//! with `book_page="…"` declares one — its properties are the page's
//! fields, its body what the page shows — and `yog build` hands it to the
//! book as a [`PageType`]. A page of the kind ([`CustomPage`]) is laid
//! out by the book like the built-in ones; what only code can draw is a
//! `drawn` element ([`super::BookPage::Drawn`]'s primitive).
//!
//! ```xml
//! <Component name="PatternPage" book_page="hexcasting:pattern">
//!     <Property name="name" type="book_text"/>
//!     <Property name="patterns" type="data" default="[]"/>
//!     <Property name="text" type="book_text" default=""/>
//!     <BookHeader text="{x:Bind name}"/>
//!     <Drawn drawer="hexcasting:pattern" h="64" data="{x:Bind patterns}" visible="{x:Bind patterns}"/>
//!     <BookText text="{x:Bind text}"/>
//! </Component>
//! ```
//!
//! An attribute that is one binding takes the field's value; bindings
//! inside text are replaced by the fields' words. `visible` shows an
//! element when what it says isn't empty (an empty list, `false` and
//! `""` are empty).

use yog_data::{Map, Reader, Schema, Shape, Value, ValueRef};

/// The kinds a page type's property can be.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    /// Words as written.
    Text,
    /// Book text: translated, with markup and links.
    BookText,
    Number,
    Bool,
    /// An item (`@item:…`), drawn as its icon.
    Item,
    /// An image source.
    Image,
    /// Anything, handed on as it is (to a `drawn` element).
    Data,
}

/// A field of the pages of a type.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct PageProperty {
    pub name: String,
    pub kind: PropKind,
    /// What a page that leaves it out has; none: the field is required.
    pub default: Option<Value>,
}

/// One thing a page of the type shows, top to bottom.
#[derive(Schema, Debug, Clone, PartialEq)]
#[data(tag = "element")]
pub enum PageElement {
    /// The page's title, centred.
    Header {
        text: String,
        visible: Option<String>,
    },
    /// Book text, wrapped to the page.
    Text {
        text: String,
        visible: Option<String>,
    },
    /// An item's icon, centred.
    Item {
        item: String,
        visible: Option<String>,
    },
    /// An image the page's width.
    Image {
        image: String,
        height: f32,
        visible: Option<String>,
    },
    /// A picture the mod's code draws (`Registry::on_book_page`).
    Drawn {
        drawer: String,
        height: f32,
        data: String,
        visible: Option<String>,
    },
    /// Room.
    Spacer { height: f32 },
}

/// A mod's kind of page: its fields and what it shows.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct PageType {
    /// `<ns>:<type>`, what pages of it say as their `type`.
    pub id: String,
    pub properties: Vec<PageProperty>,
    pub body: Vec<PageElement>,
}

/// A page of a mod's own type: its `type` and fields as written; the
/// book lays it out by its [`PageType`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CustomPage {
    pub kind: String,
    pub fields: Map,
}

impl Schema for CustomPage {
    fn shape() -> Shape {
        Shape::Map(Value::shape)
    }

    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let map = cx.map(v)?;
        let kind = map.get("type").and_then(|t| t.as_str()).unwrap_or_default();
        if !kind.contains(':') {
            cx.at(yog_data::Seg::Key("type".into()), |cx| {
                cx.problem(format!(
                    "unknown page type `{kind}` — text, spotlight, recipe, image, entity, multiblock, relations, drawn, empty, or a mod's own (`mymod:kind`, a `.yui` component with `book_page`)"
                ))
            });
            return None;
        }
        let mut fields = Map::with_capacity(map.len());
        for (k, x) in map.iter().filter(|(k, _)| *k != "type") {
            fields.insert(k, x.to_value());
        }
        Some(CustomPage { kind: kind.to_string(), fields })
    }

    fn write(&self) -> Value {
        let mut m = Map::with_capacity(self.fields.len() + 1);
        m.insert("type", Value::from(self.kind.as_str()));
        for (k, v) in self.fields.iter() {
            m.insert(k, v.clone());
        }
        Value::Map(m)
    }
}

/// A field's words: strings as they are, numbers and booleans written out.
pub fn words(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Ref(r) | Value::Tag(r) => r.id.clone(),
        Value::Lang(k) => k.clone(),
        v => v.as_str().unwrap_or_default().to_string(),
    }
}

/// Whether a field counts as there: not null, `false`, `""` or empty.
pub fn present(v: &Value) -> bool {
    match v {
        Value::Null | Value::Bool(false) => false,
        Value::List(l) => !l.is_empty(),
        Value::Map(m) => !m.is_empty(),
        v => !words(v).is_empty(),
    }
}

impl PageType {
    /// Field `name` of `page`, else the property's default.
    pub fn field<'a>(&'a self, page: &'a CustomPage, name: &str) -> Option<&'a Value> {
        page.fields.get(name).or_else(|| self.properties.iter().find(|p| p.name == name)?.default.as_ref())
    }

    /// `attr` with its bindings (`{x:Bind name}`) replaced by `page`'s words.
    pub fn text(&self, page: &CustomPage, attr: &str) -> String {
        let mut out = String::new();
        let mut rest = attr;
        while let Some(i) = rest.find("{x:Bind ") {
            out.push_str(&rest[..i]);
            let after = &rest[i + 8..];
            let Some(end) = after.find('}') else {
                out.push_str(&rest[i..]);
                return out;
            };
            if let Some(v) = self.field(page, after[..end].trim()) {
                out.push_str(&words(v));
            }
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        out
    }

    /// `attr`'s value: a field's whole value when it's one binding, else its text.
    pub fn value(&self, page: &CustomPage, attr: &str) -> Value {
        let t = attr.trim();
        if let Some(name) = t.strip_prefix("{x:Bind ").and_then(|r| r.strip_suffix('}'))
            && !name.contains('{')
        {
            return self.field(page, name.trim()).cloned().unwrap_or(Value::Null);
        }
        Value::Str(self.text(page, attr))
    }

    /// Whether an element with `visible` shows on `page`.
    pub fn visible(&self, page: &CustomPage, visible: &Option<String>) -> bool {
        visible.as_ref().is_none_or(|v| present(&self.value(page, v)))
    }
}

// ── From `.yui` ────────────────────────────────────────────────────────────────

use crate::yui::Element;

/// Whether `root` declares a kind of book page.
pub fn is_book_page(root: &Element) -> bool {
    root.name == "Component" && root.attr("book_page").is_some()
}

fn err<T>(el: &Element, msg: impl std::fmt::Display) -> Result<T, String> {
    Err(format!("line {}: <{}>: {msg}", el.line, el.name))
}

const KINDS: &[(&str, PropKind)] = &[
    ("text", PropKind::Text),
    ("book_text", PropKind::BookText),
    ("number", PropKind::Number),
    ("bool", PropKind::Bool),
    ("item", PropKind::Item),
    ("image", PropKind::Image),
    ("data", PropKind::Data),
];

/// The names `{x:Bind …}` in `attr` refer to.
fn bindings(attr: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = attr;
    while let Some(i) = rest.find("{x:Bind ") {
        let after = &rest[i + 8..];
        let Some(end) = after.find('}') else { break };
        out.push(after[..end].trim());
        rest = &after[end + 1..];
    }
    out
}

fn number(el: &Element, key: &str, default: Option<f32>) -> Result<f32, String> {
    match el.attr(key) {
        Some(v) => v.trim().parse::<f32>().or_else(|_| err(el, format_args!("{key}=\"{v}\": a number of book pixels"))),
        None => default.map(Ok).unwrap_or_else(|| err(el, format_args!("needs {key}=\"…\""))),
    }
}

/// `root` (a `<Component book_page="…">`) as a page type; `ns`: the mod's
/// namespace, for a `book_page` written without one.
pub fn compile(root: &Element, ns: &str) -> Result<PageType, String> {
    let id = root.attr("book_page").unwrap_or_default().trim();
    let id = if id.contains(':') { id.to_string() } else { format!("{ns}:{id}") };
    let mut properties: Vec<PageProperty> = Vec::new();
    let mut body = Vec::new();
    // The elements may sit in one <Column>, as a component's body does.
    let mut children: Vec<&Element> = Vec::new();
    for c in &root.children {
        if c.name == "Column" {
            children.extend(c.children.iter());
        } else {
            children.push(c);
        }
    }
    for el in children {
        let attr = |k: &str| el.attr(k).map(str::to_string);
        let visible = attr("visible");
        let text = || attr("text").map(Ok).unwrap_or_else(|| err(el, "needs text=\"…\""));
        let element = match el.name.as_str() {
            "Property" => {
                let name = attr("name").unwrap_or_default();
                if properties.iter().any(|p| p.name == name) || name.is_empty() {
                    return err(el, format_args!("name=\"{name}\": empty or declared twice"));
                }
                if name == "type" || name == "anchor" {
                    return err(el, format_args!("`{name}` is every page's own field — pick another name"));
                }
                let ty = attr("type").unwrap_or_else(|| "text".into());
                let Some((_, kind)) = KINDS.iter().find(|(k, _)| *k == ty) else {
                    let all: Vec<&str> = KINDS.iter().map(|(k, _)| *k).collect();
                    return err(el, format_args!("type=\"{ty}\": one of {}", all.join(", ")));
                };
                let default = match (attr("default"), kind) {
                    (None, _) => None,
                    (Some(d), PropKind::Data | PropKind::Number | PropKind::Bool) => Some(
                        yog_data::text::parse_value(&d).map_err(|e| format!("line {}: <Property> default=\"{d}\": {}", el.line, e.message))?,
                    ),
                    (Some(d), _) => Some(yog_data::Value::Str(d)),
                };
                properties.push(PageProperty { name, kind: *kind, default });
                continue;
            }
            "BookHeader" => PageElement::Header { text: text()?, visible },
            "BookText" => PageElement::Text { text: text()?, visible },
            "BookItem" => PageElement::Item { item: attr("item").map(Ok).unwrap_or_else(|| err(el, "needs item=\"…\""))?, visible },
            "Image" => PageElement::Image {
                image: attr("src").map(Ok).unwrap_or_else(|| err(el, "needs src=\"…\""))?,
                height: number(el, "h", None)?,
                visible,
            },
            "Drawn" => PageElement::Drawn {
                drawer: attr("drawer").map(Ok).unwrap_or_else(|| err(el, "needs drawer=\"mymod:…\" (Registry::on_book_page)"))?,
                height: number(el, "h", None)?,
                data: attr("data").unwrap_or_default(),
                visible,
            },
            "Spacer" => PageElement::Spacer { height: number(el, "h", Some(4.0))? },
            other => {
                return err(el, format_args!("not a book page's element (`{other}`) — BookHeader, BookText, BookItem, Image, Drawn, Spacer"));
            }
        };
        body.push((el, element));
    }
    // Every binding names a property.
    for (el, _) in &body {
        for (key, value) in &el.attrs {
            for name in bindings(value) {
                if !properties.iter().any(|p| p.name == name) {
                    return err(el, format_args!("{key}: `{name}` isn't a property of this page"));
                }
            }
        }
    }
    Ok(PageType { id, properties, body: body.into_iter().map(|(_, e)| e).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(json: &str) -> CustomPage {
        let v = yog_data::json::from_json_text(json).unwrap();
        yog_data::read_value::<CustomPage>(&v).unwrap()
    }

    #[test]
    fn bindings_take_fields_and_defaults() {
        let t = PageType {
            id: "m:p".into(),
            properties: vec![
                PageProperty { name: "input".into(), kind: PropKind::Text, default: Some(Value::from("")) },
                PageProperty { name: "patterns".into(), kind: PropKind::Data, default: Some(Value::List(vec![])) },
            ],
            body: vec![],
        };
        let p = page(r#"{"type":"m:p","input":"entity","patterns":[{"startdir":"EAST"}]}"#);
        assert_eq!(p.kind, "m:p");
        assert_eq!(t.text(&p, "{u|{x:Bind input}} → {u|{x:Bind output}}"), "{u|entity} → {u|}");
        assert!(matches!(t.value(&p, "{x:Bind patterns}"), Value::List(l) if l.len() == 1));
        assert!(t.visible(&p, &Some("{x:Bind patterns}".into())));
        let bare = page(r#"{"type":"m:p"}"#);
        assert!(!t.visible(&bare, &Some("{x:Bind patterns}".into())), "the default: an empty list");
        assert!(!t.visible(&bare, &Some("{x:Bind input}{x:Bind output}".into())));
        let e = yog_data::read_value::<CustomPage>(&yog_data::json::from_json_text(r#"{"type":"nope"}"#).unwrap()).unwrap_err();
        assert!(e[0].message.contains("unknown page type `nope`"), "{e:?}");
    }
    #[test]
    fn compiles_a_page_type() {
        let el = crate::yui::parse(
            r#"<Component name="PatternPage" book_page="pattern">
                <Property name="name" type="book_text"/>
                <Property name="patterns" type="data" default="[]"/>
                <Column>
                    <BookHeader text="{x:Bind name}"/>
                    <Drawn drawer="hexcasting:pattern" h="64" data="{x:Bind patterns}" visible="{x:Bind patterns}"/>
                </Column>
            </Component>"#,
        )
        .unwrap();
        assert!(is_book_page(&el));
        let t = compile(&el, "hexcasting").unwrap();
        assert_eq!(t.id, "hexcasting:pattern");
        assert_eq!(t.properties[1].default, Some(yog_data::Value::List(vec![])));
        assert!(matches!(&t.body[1], PageElement::Drawn { height, .. } if *height == 64.0));

        let bad = crate::yui::parse(r#"<Component book_page="p"><BookText text="{x:Bind nope}"/></Component>"#).unwrap();
        assert!(compile(&bad, "m").unwrap_err().contains("`nope` isn't a property"));
        let bad = crate::yui::parse(r#"<Component book_page="p"><Row/></Component>"#).unwrap();
        assert!(compile(&bad, "m").unwrap_err().contains("not a book page's element"));
    }
}
