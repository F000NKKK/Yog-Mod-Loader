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

use crate::binding::{self, Piece};
use crate::content::word;

/// A page's field naming its type.
const TYPE_FIELD: &str = "type";

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
        let kind = map.get(TYPE_FIELD).and_then(|t| t.as_str()).unwrap_or_default();
        if !kind.contains(yog_ids::sep::NAMESPACE) {
            cx.at(yog_data::Seg::Key(TYPE_FIELD.into()), |cx| {
                cx.problem(format!(
                    "unknown page type `{kind}` — text, spotlight, recipe, image, entity, multiblock, relations, drawn, empty, or a mod's own (`mymod:kind`, a `.yui` component with `book_page`)"
                ))
            });
            return None;
        }
        let mut fields = Map::with_capacity(map.len());
        for (k, x) in map.iter().filter(|(k, _)| *k != TYPE_FIELD) {
            fields.insert(k, x.to_value());
        }
        Some(CustomPage {
            kind: kind.to_string(),
            fields,
        })
    }

    fn write(&self) -> Value {
        let mut m = Map::with_capacity(self.fields.len() + 1);
        m.insert(TYPE_FIELD, Value::from(self.kind.as_str()));
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
        page.fields.get(name).or_else(|| {
            self.properties
                .iter()
                .find(|p| p.name == name)?
                .default
                .as_ref()
        })
    }

    /// `attr` with its bindings (`{x:Bind name}`) replaced by `page`'s words.
    pub fn text(&self, page: &CustomPage, attr: &str) -> String {
        let mut out = String::new();
        for piece in binding::pieces(attr) {
            match piece {
                Piece::Text(t) => out.push_str(t),
                Piece::Bind(name) => {
                    if let Some(v) = self.field(page, name) {
                        out.push_str(&words(v));
                    }
                }
            }
        }
        out
    }

    /// `attr`'s value: a field's whole value when it's one binding, else its text.
    pub fn value(&self, page: &CustomPage, attr: &str) -> Value {
        if let Some(name) = binding::whole(attr) {
            return self.field(page, name).cloned().unwrap_or(Value::Null);
        }
        Value::Str(self.text(page, attr))
    }

    /// Whether an element with `visible` shows on `page`.
    pub fn visible(&self, page: &CustomPage, visible: &Option<String>) -> bool {
        visible
            .as_ref()
            .is_none_or(|v| present(&self.value(page, v)))
    }
}

// ── From `.yui` ────────────────────────────────────────────────────────────────

use crate::yui::Element;
use yog_ids::{yui, yui_attr};

/// The fields every page has: a property can't take their names.
const OWN_FIELDS: [&str; 2] = [TYPE_FIELD, ANCHOR_FIELD];
const ANCHOR_FIELD: &str = "anchor";
/// A `<Spacer>`'s height, in book pixels, when it gives none.
const SPACER_HEIGHT: f32 = 4.0;
/// The elements a page type shows.
const ELEMENTS: [&str; 6] = [
    yui::BOOK_HEADER,
    yui::BOOK_TEXT,
    yui::BOOK_ITEM,
    yui::IMAGE,
    yui::DRAWN,
    yui::SPACER,
];

/// Whether `root` declares a kind of book page.
pub fn is_book_page(root: &Element) -> bool {
    root.name == yui::COMPONENT && root.attr(yui_attr::BOOK_PAGE).is_some()
}

fn err<T>(el: &Element, msg: impl std::fmt::Display) -> Result<T, String> {
    Err(format!("line {}: <{}>: {msg}", el.line, el.name))
}

impl PropKind {
    const ALL: [PropKind; 7] = [
        PropKind::Text,
        PropKind::BookText,
        PropKind::Number,
        PropKind::Bool,
        PropKind::Item,
        PropKind::Image,
        PropKind::Data,
    ];

    /// The kind written `name`.
    fn named(name: &str) -> Option<PropKind> {
        Self::ALL.into_iter().find(|k| word(k) == name)
    }
}

fn number(el: &Element, key: &str, default: Option<f32>) -> Result<f32, String> {
    match el.attr(key) {
        Some(v) => v
            .trim()
            .parse::<f32>()
            .or_else(|_| err(el, format_args!("{key}=\"{v}\": a number of book pixels"))),
        None => default
            .map(Ok)
            .unwrap_or_else(|| err(el, format_args!("needs {key}=\"…\""))),
    }
}

/// Attribute `key`, which `el` needs (`what` says what it is).
fn needs(el: &Element, key: &str, what: &str) -> Result<String, String> {
    el.attr(key)
        .map(|v| Ok(v.to_string()))
        .unwrap_or_else(|| err(el, format_args!("needs {key}=\"…\"{what}")))
}

/// `root` (a `<Component book_page="…">`) as a page type; `ns`: the mod's
/// namespace, for a `book_page` written without one.
pub fn compile(root: &Element, ns: &str) -> Result<PageType, String> {
    let id = yog_ids::qualify_in(ns, root.attr(yui_attr::BOOK_PAGE).unwrap_or_default().trim());
    let mut properties: Vec<PageProperty> = Vec::new();
    let mut body = Vec::new();
    for el in elements(root) {
        if el.name == yui::PROPERTY {
            let p = property(el)?;
            if properties.iter().any(|q| q.name == p.name) {
                return err(el, format_args!("name=\"{}\": declared twice", p.name));
            }
            properties.push(p);
        } else {
            body.push((el, element(el)?));
        }
    }
    // Every binding names a property.
    for (el, _) in &body {
        for (key, value) in &el.attrs {
            for name in binding::names(value) {
                if !properties.iter().any(|p| p.name == name) {
                    return err(el, format_args!("{key}: `{name}` isn't a property of this page"));
                }
            }
        }
    }
    Ok(PageType {
        id,
        properties,
        body: body.into_iter().map(|(_, e)| e).collect(),
    })
}

/// The page's elements: `root`'s, or those of the one `<Column>` they sit
/// in, as a component's body does.
fn elements(root: &Element) -> Vec<&Element> {
    let mut out = Vec::new();
    for c in &root.children {
        if c.name == yui::COLUMN {
            out.extend(c.children.iter());
        } else {
            out.push(c);
        }
    }
    out
}

/// `<Property name type default>`: a field of the pages.
fn property(el: &Element) -> Result<PageProperty, String> {
    let name = el.attr(yui_attr::NAME).unwrap_or_default().to_string();
    if name.is_empty() {
        return err(el, "needs name=\"…\"");
    }
    if OWN_FIELDS.contains(&name.as_str()) {
        return err(
            el,
            format_args!("`{name}` is every page's own field — pick another name"),
        );
    }
    let ty = el.attr(yui_attr::TYPE).map_or_else(|| word(&PropKind::Text), str::to_string);
    let Some(kind) = PropKind::named(&ty) else {
        let all: Vec<String> = PropKind::ALL.iter().map(word).collect();
        return err(el, format_args!("type=\"{ty}\": one of {}", all.join(", ")));
    };
    let default = match (el.attr(yui_attr::DEFAULT), kind) {
        (None, _) => None,
        (Some(d), PropKind::Data | PropKind::Number | PropKind::Bool) => {
            Some(yog_data::text::parse_value(d).map_err(|e| {
                format!("line {}: <{}> default=\"{d}\": {}", el.line, el.name, e.message)
            })?)
        }
        (Some(d), _) => Some(Value::from(d)),
    };
    Ok(PageProperty {
        name,
        kind,
        default,
    })
}

/// One of the elements a page shows.
fn element(el: &Element) -> Result<PageElement, String> {
    let visible = el.attr(yui_attr::VISIBLE).map(str::to_string);
    Ok(match el.name.as_str() {
        yui::BOOK_HEADER => PageElement::Header {
            text: needs(el, yui_attr::TEXT, "")?,
            visible,
        },
        yui::BOOK_TEXT => PageElement::Text {
            text: needs(el, yui_attr::TEXT, "")?,
            visible,
        },
        yui::BOOK_ITEM => PageElement::Item {
            item: needs(el, yui_attr::ITEM, "")?,
            visible,
        },
        yui::IMAGE => PageElement::Image {
            image: needs(el, yui_attr::SRC, "")?,
            height: number(el, yui_attr::H, None)?,
            visible,
        },
        yui::DRAWN => PageElement::Drawn {
            drawer: needs(el, yui_attr::DRAWER, " (Registry::on_book_page)")?,
            height: number(el, yui_attr::H, None)?,
            data: el.attr(yui_attr::DATA).unwrap_or_default().to_string(),
            visible,
        },
        yui::SPACER => PageElement::Spacer {
            height: number(el, yui_attr::H, Some(SPACER_HEIGHT))?,
        },
        other => {
            return err(
                el,
                format_args!(
                    "not a book page's element (`{other}`) — {}",
                    ELEMENTS.join(", ")
                ),
            );
        }
    })
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
                PageProperty {
                    name: "input".into(),
                    kind: PropKind::Text,
                    default: Some(Value::from("")),
                },
                PageProperty {
                    name: "patterns".into(),
                    kind: PropKind::Data,
                    default: Some(Value::List(vec![])),
                },
            ],
            body: vec![],
        };
        let p = page(r#"{"type":"m:p","input":"entity","patterns":[{"startdir":"EAST"}]}"#);
        assert_eq!(p.kind, "m:p");
        assert_eq!(
            t.text(&p, "{u|{x:Bind input}} → {u|{x:Bind output}}"),
            "{u|entity} → {u|}"
        );
        assert!(matches!(t.value(&p, "{x:Bind patterns}"), Value::List(l) if l.len() == 1));
        assert!(t.visible(&p, &Some("{x:Bind patterns}".into())));
        let bare = page(r#"{"type":"m:p"}"#);
        assert!(
            !t.visible(&bare, &Some("{x:Bind patterns}".into())),
            "the default: an empty list"
        );
        assert!(!t.visible(&bare, &Some("{x:Bind input}{x:Bind output}".into())));
        let e = yog_data::read_value::<CustomPage>(
            &yog_data::json::from_json_text(r#"{"type":"nope"}"#).unwrap(),
        )
        .unwrap_err();
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

        let bad = crate::yui::parse(
            r#"<Component book_page="p"><BookText text="{x:Bind nope}"/></Component>"#,
        )
        .unwrap();
        assert!(
            compile(&bad, "m")
                .unwrap_err()
                .contains("`nope` isn't a property")
        );
        let bad = crate::yui::parse(r#"<Component book_page="p"><Row/></Component>"#).unwrap();
        assert!(
            compile(&bad, "m")
                .unwrap_err()
                .contains("not a book page's element")
        );
    }
}
