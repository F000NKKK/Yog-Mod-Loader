//! A mod's own kinds of book page: `.yui` components with `book_page`,
//! compiled into `yog_book::page_type::PageType`s for the books that use
//! them (see that module).
//!
//! ```xml
//! <Component name="PatternPage" book_page="hexcasting:pattern">
//!     <Property name="name" type="book_text"/>
//!     <Property name="patterns" type="data" default="[]"/>
//!     <BookHeader text="{x:Bind name}"/>
//!     <Drawn drawer="hexcasting:pattern" h="64" data="{x:Bind patterns}" visible="{x:Bind patterns}"/>
//! </Component>
//! ```

use std::path::Path;

use yog_book::page_type::{PageElement, PageProperty, PageType, PropKind};

use crate::yui::{self, Element};

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

/// The mod's page types: every `book_page` component under `src/`.
pub fn find(root: &Path, ns: &str, errors: &mut Vec<String>) -> Vec<PageType> {
    let mut out = Vec::new();
    for file in yui::find_yui_files(&root.join("src")) {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let Ok(el) = yui::parse_xml(&text) else { continue };
        if !is_book_page(&el) {
            continue;
        }
        let shown = file.strip_prefix(root).unwrap_or(&file).display().to_string();
        match compile(&el, ns) {
            Ok(t) => out.push(t),
            Err(e) => errors.push(format!("{shown}: {e}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_a_page_type() {
        let el = yui::parse_xml(
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

        let bad = yui::parse_xml(r#"<Component book_page="p"><BookText text="{x:Bind nope}"/></Component>"#).unwrap();
        assert!(compile(&bad, "m").unwrap_err().contains("`nope` isn't a property"));
        let bad = yui::parse_xml(r#"<Component book_page="p"><Row/></Component>"#).unwrap();
        assert!(compile(&bad, "m").unwrap_err().contains("not a book page's element"));
    }
}
