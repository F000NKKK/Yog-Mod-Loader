//! Component `.yui` files: a `<Component>` root declaring properties and
//! events, with one body element bound to them through `{x:Bind ...}` (the
//! same bindings as view-model screens, [`crate::ui::vm::compile_bind`]).
//! Generates a struct with one typed field per property and its
//! `yog_api::ui::component::Component` implementation, registered with
//! `ui_component!`. See Yog-IDLE's `docs/yui.md`, "Components".
//!
//! ```xml
//! <Component name=yog_ids::yui::STAT_BAR description="A labelled bar." icon="item:experience_bottle"
//!            design_w="120" design_h="10">
//!     <Property name="label" type="text" default="HP" doc="Shown on the left" category="Text"/>
//!     <Property name="value" type="number" default="0.5"/>
//!     <Event name="on_select"/>
//!     <Row w="100%" gap="4">
//!         <Label text="{x:Bind label}"/>
//!         <Spacer w="{x:Bind bar_w()}" h="6" bg="#FF55FF55"/>
//!     </Row>
//! </Component>
//! ```
//!
//! Bindings are evaluated once per instance, when the screen's tree is
//! built. Methods they call (`bar_w()`) live in the optional code-behind
//! `stat_bar.rs` (`yog_api::ui_include!("stat_bar"); impl StatBar { ... }`);
//! components without one are compiled in by `yog_api::ui_components!()`.
//!
//! The root's designer metadata — `display_name`, `description`, `icon`,
//! `category`, `design_w`/`design_h`, `toolbox` — goes into the
//! implementation's constants as written (`@Set.key` references included),
//! for Yog-IDLE's designer host.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use yog_ids::prop_kind as pk;
use yog_ids::yui_attr as at;
use yog_ids::yui_component as ca;

use crate::ui::yui::{self, Element, RAW_CALL, RAW_VALUE, UI_USE, err};
use yog_data::Diagnostic;

/// Index of the components without a code-behind, included by `ui_components!()`.
pub const INDEX_FILE: &str = ".yog-build/generated/components.g.rs";

/// A property type: its word, its field's Rust type, and whether
/// `Props` reads it by a method of its name (else as a word).
struct KindSpec {
    word: &'static str,
    rust: &'static str,
    typed: bool,
}

/// The property types.
const KINDS: [KindSpec; 7] = [
    KindSpec {
        word: pk::TEXT,
        rust: "String",
        typed: true,
    },
    KindSpec {
        word: pk::NUMBER,
        rust: "f32",
        typed: true,
    },
    KindSpec {
        word: pk::INT,
        rust: "i64",
        typed: true,
    },
    KindSpec {
        word: pk::BOOL,
        rust: "bool",
        typed: true,
    },
    KindSpec {
        word: pk::COLOR,
        rust: "u32",
        typed: true,
    },
    KindSpec {
        word: pk::CHOICE,
        rust: "String",
        typed: false,
    },
    KindSpec {
        word: pk::IMAGE,
        rust: "String",
        typed: false,
    },
];

/// A color property's value when it gives none: opaque white.
const WHITE: &str = "#FFFFFFFF";
/// What separates a choice's options.
const OPTION_SEP: char = ',';

#[derive(Debug, Clone)]
struct Prop {
    name: String,
    /// `text`, `number`, ..., or `event`.
    kind: String,
    default: String,
    doc: String,
    category: String,
    options: Vec<String>,
}

/// The `<Component>` root's attributes (besides `d:*`, design-time only).
const ROOT_ATTRS: &[&str] = &[
    ca::NAME,
    ca::DISPLAY_NAME,
    ca::DESCRIPTION,
    ca::ICON,
    ca::CATEGORY,
    ca::DESIGN_W,
    ca::DESIGN_H,
    ca::TOOLBOX,
];
/// A `<Property>`'s attributes, and an `<Event>`'s.
const PROP_ATTRS: &[&str] = &[
    ca::NAME,
    ca::TYPE,
    ca::DEFAULT,
    ca::OPTIONS,
    ca::DOC,
    ca::CATEGORY,
];
const EVENT_ATTRS: &[&str] = &[ca::NAME, ca::DOC, ca::CATEGORY];

/// A property's name: snake_case.
fn is_ident(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The component's type name: `name=`, else the file's PascalCase stem.
fn type_name(root: &Element, include_name: &str) -> Result<String, Diagnostic> {
    let name = match root.attr(ca::NAME).map(str::trim).filter(|s| !s.is_empty()) {
        Some(n) => n.to_string(),
        None => yui::pascal(
            include_name
                .rsplit(yog_ids::sep::PATH)
                .next()
                .unwrap_or(include_name),
        ),
    };
    let ok = name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric());
    if !ok {
        return err(
            root,
            format_args!("name=\"{name}\": a component's name is PascalCase (StatBar)"),
        );
    }
    Ok(name)
}

/// A property's value when the `.yui` gives none, by its type.
pub(crate) fn default_of(kind: &str, options: &[String]) -> String {
    match kind {
        pk::NUMBER | pk::INT => "0".into(),
        pk::BOOL => yog_ids::yui_value::FALSE.into(),
        pk::COLOR => WHITE.into(),
        pk::CHOICE => options.first().cloned().unwrap_or_default(),
        _ => String::new(),
    }
}

/// A choice's options as written: `a, b, c`.
pub(crate) fn options_of(written: Option<&str>) -> Vec<String> {
    written
        .unwrap_or_default()
        .split(OPTION_SEP)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The declarations, in order, and the one body element after them.
fn declarations(root: &Element) -> Result<(Vec<Prop>, Option<&Element>), Diagnostic> {
    let mut props: Vec<Prop> = Vec::new();
    let mut body = None;
    for child in &root.children {
        if matches!(
            child.name.as_str(),
            yog_ids::yui::PROPERTY | yog_ids::yui::EVENT
        ) {
            let p = declaration(child)?;
            if props.iter().any(|q| q.name == p.name) {
                return err(child, format_args!("`{}` is declared twice", p.name));
            }
            props.push(p);
        } else if body.is_some() {
            return err(
                child,
                "a component has one body element (wrap several in a Column, Row or Canvas)",
            );
        } else {
            body = Some(child);
        }
    }
    Ok((props, body))
}

/// One `<Property>` or `<Event>`.
fn declaration(child: &Element) -> Result<Prop, Diagnostic> {
    let event = child.name == yog_ids::yui::EVENT;
    let allowed = if event { EVENT_ATTRS } else { PROP_ATTRS };
    if let Some((key, _)) = child
        .attrs
        .iter()
        .find(|(k, _)| !allowed.contains(&k.as_str()))
    {
        return err(
            child,
            format_args!("has no attribute `{key}` ({})", allowed.join(", ")),
        );
    }
    let name = child
        .attr(ca::NAME)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if !is_ident(&name) {
        return err(
            child,
            format_args!("name=\"{name}\": expected a snake_case name"),
        );
    }
    if yui::is_common_attr(&name) {
        return err(
            child,
            format_args!("`{name}` is an attribute every element has — pick another name"),
        );
    }
    let kind = if event {
        pk::EVENT.to_string()
    } else {
        declared_kind(child)?
    };
    let options = options_of(child.attr(ca::OPTIONS));
    if kind == pk::CHOICE && options.is_empty() {
        return err(child, "a choice needs options=\"a, b, c\"");
    }
    let default = checked_default(child, &kind, &options)?;
    let text = |key: &str| child.attr(key).unwrap_or_default().to_string();
    Ok(Prop {
        name,
        kind,
        default,
        doc: text(ca::DOC),
        category: text(ca::CATEGORY),
        options,
    })
}

/// A `<Property>`'s type: one of [`KINDS`] (text when it gives none).
fn declared_kind(child: &Element) -> Result<String, Diagnostic> {
    let kind = child.attr(ca::TYPE).unwrap_or(pk::TEXT).trim().to_string();
    if !KINDS.iter().any(|k| k.word == kind) {
        let known: Vec<&str> = KINDS.iter().map(|k| k.word).collect();
        return err(
            child,
            format_args!("type=\"{kind}\": expected one of {}", known.join(", ")),
        );
    }
    Ok(kind)
}

/// A property's default — as written, or its type's — checked against its
/// type; a color's as its value.
fn checked_default(child: &Element, kind: &str, options: &[String]) -> Result<String, Diagnostic> {
    let default = child
        .attr(ca::DEFAULT)
        .map_or_else(|| default_of(kind, options), str::to_string);
    let v = default.trim();
    let wrong = match kind {
        pk::NUMBER => v.parse::<f32>().is_err().then_some("expected a number"),
        pk::INT => v
            .parse::<i64>()
            .is_err()
            .then_some("expected a whole number"),
        pk::BOOL => (!matches!(v, yog_ids::yui_value::TRUE | yog_ids::yui_value::FALSE))
            .then_some("expected true or false"),
        // A literal or a `.yres` color, stored as its value.
        pk::COLOR => {
            return yui::color_attr(child, ca::DEFAULT, &default).map(|c| format!("#{c:08X}"));
        }
        pk::CHOICE => (!options.contains(&default)).then_some("isn't one of the options"),
        _ => None,
    };
    match wrong {
        Some(why) if kind == pk::CHOICE => err(child, format_args!("default=\"{default}\" {why}")),
        Some(why) => err(child, format_args!("default=\"{default}\": {why}")),
        None => Ok(default),
    }
}

/// What `<Image>` and `<Surface>` show until a bound source is set — valid
/// for the static checks.
const IMAGE_STAND_IN: &str = "item:air";
const SURFACE_STAND_IN: &str = "yog:none";
/// The binding conversions of the API.
const CONV: &str = "::yog_api::ui::component::bind";

/// The body with every `{x:Bind ...}` attribute turned into a builder call
/// (or, on a nested component, a Rust-valued property).
fn bind_tree(el: &Element, counter: &mut usize) -> Result<Element, Diagnostic> {
    let kind = el.name.as_str();
    if kind == yog_ids::yui::COMPONENT || kind == yog_ids::yui::SCREEN {
        return err(el, "can't be inside a component");
    }
    let mut out = Element {
        name: el.name.clone(),
        attrs: Vec::new(),
        children: Vec::new(),
        line: el.line,
    };
    // Design-time attributes (the designer's sample values) are left out.
    for (key, value) in el
        .attrs
        .iter()
        .filter(|(k, _)| !k.starts_with(yog_ids::yui_markup::DESIGN))
    {
        let Some(rust) = crate::ui::vm::compile_bind(value) else {
            out.attrs.push((key.clone(), value.clone()));
            continue;
        };
        let e = rust.map_err(|e| {
            Diagnostic::from(format!("<{kind}> {key}=\"{value}\": {e}")).at_line(el.line)
        })?;
        if !yui::is_builtin(kind) && !yui::is_common_attr(key) {
            // A nested component's property: passed as text.
            out.attrs
                .push((key.clone(), format!("{RAW_VALUE}{CONV}::text(&({e}))")));
            continue;
        }
        if let Some(stand_in) = stand_in(kind, key) {
            out.attrs.push((at::SOURCE.into(), stand_in.into()));
        }
        let call = bound_call(kind, key, &e).map_or_else(
            || {
                err(
                    el,
                    format_args!("{key}=\"{value}\": `{key}` can't be bound"),
                )
            },
            Ok,
        )?;
        out.attrs.push((format!("{RAW_CALL}{counter}"), call));
        *counter += 1;
    }
    for child in &el.children {
        out.children.push(bind_tree(child, counter)?);
    }
    Ok(out)
}

/// The static source an element with a bound source shows until it's set.
fn stand_in(kind: &str, key: &str) -> Option<&'static str> {
    match (kind, key) {
        (yog_ids::yui::SURFACE, at::SOURCE) => Some(SURFACE_STAND_IN),
        (yog_ids::yui::IMAGE, at::SOURCE) => Some(IMAGE_STAND_IN),
        _ => None,
    }
}

/// The builder call setting attribute `key` of a `kind` element to the
/// value of Rust expression `e`; none when it can't be bound.
fn bound_call(kind: &str, key: &str, e: &str) -> Option<String> {
    use yog_ids::yui as y;
    let conv = |f: &str| format!("{CONV}::{f}(&({e}))");
    Some(match key {
        at::TEXT if matches!(kind, y::LABEL | y::BUTTON | y::TEXT_BOX) => {
            format!("with_text({})", conv("text"))
        }
        at::PLACEHOLDER if kind == y::TEXT_BOX => format!("{key}({})", conv("text")),
        at::SOURCE if matches!(kind, y::SURFACE | y::IMAGE) => {
            format!("with_source({})", conv("text"))
        }
        at::ITEM if matches!(kind, y::ITEM_SLOT | y::INV_SLOT) => {
            format!("with_source({})", conv("text"))
        }
        at::COUNT if kind == y::INV_SLOT => format!("with_count({})", conv("count")),
        at::BG | at::COLOR | at::FOCUS_COLOR => format!("{key}({})", conv("color")),
        at::TINT if kind == y::IMAGE => format!("{key}({})", conv("color")),
        at::ENABLED | at::VISIBLE | at::SHADOW => format!("{key}({CONV}::flag({e}))"),
        at::W | at::H => format!("{key}({})", conv("units")),
        at::GAP | at::FLEX | at::MIN_W | at::MIN_H | at::FONT_SCALE => {
            format!("{key}({})", conv("number"))
        }
        at::ON_CLICK | at::ON_CHANGE | at::ON_SUBMIT => format!("{key}({})", conv("text")),
        _ => return None,
    })
}

/// The root's designer metadata, as the implementation's constants say it.
struct Meta<'a> {
    doc: &'a str,
    display_name: &'a str,
    icon: &'a str,
    category: &'a str,
    design: (f32, f32),
    toolbox: bool,
}

/// What an image source starts with or has, beside a `.yres` image (`@Icons.name`).
const RES_MARK: char = '@';

impl<'a> Meta<'a> {
    fn read(root: &'a Element) -> Result<Meta<'a>, Diagnostic> {
        if let Some((key, _)) = root.attrs.iter().find(|(k, _)| {
            !ROOT_ATTRS.contains(&k.as_str()) && !k.starts_with(yog_ids::yui_markup::DESIGN)
        }) {
            return err(
                root,
                format_args!("has no attribute `{key}` ({})", ROOT_ATTRS.join(", ")),
            );
        }
        let toolbox = match root.attr(ca::TOOLBOX).map(str::trim) {
            None | Some(yog_ids::yui_value::TRUE) => true,
            Some(yog_ids::yui_value::FALSE) => false,
            Some(v) => {
                return err(
                    root,
                    format_args!("toolbox=\"{v}\": expected true or false"),
                );
            }
        };
        let icon = root.attr(ca::ICON).unwrap_or_default().trim();
        if !icon.is_empty()
            && !icon.starts_with(RES_MARK)
            && !icon.contains(yog_ids::sep::NAMESPACE)
        {
            return err(
                root,
                format_args!(
                    "icon=\"{icon}\": expected an image source (item:apple, sprite:button, mod:gui/logo) or @Icons.name"
                ),
            );
        }
        let text = |key: &str| root.attr(key).unwrap_or_default();
        Ok(Meta {
            doc: text(ca::DESCRIPTION),
            display_name: text(ca::DISPLAY_NAME),
            icon,
            category: text(ca::CATEGORY),
            design: (
                design_size(root, ca::DESIGN_W)?,
                design_size(root, ca::DESIGN_H)?,
            ),
            toolbox,
        })
    }
}

/// A design size: GUI pixels, none for 0.
fn design_size(root: &Element, key: &str) -> Result<f32, Diagnostic> {
    match root.attr(key).map(str::trim) {
        None | Some("") => Ok(0.0),
        Some(v) => match v.parse::<f32>() {
            Ok(n) if n >= 0.0 => Ok(n),
            _ => err(
                root,
                format_args!("{key}=\"{v}\": expected a size in GUI pixels"),
            ),
        },
    }
}

/// A property's field, its line of `from_props`, and its declaration.
fn prop_code(p: &Prop) -> (String, String, String) {
    let lit = |s: &str| format!("{s:?}");
    let spec = KINDS.iter().find(|k| k.word == p.kind);
    let mut field = String::new();
    if !p.doc.is_empty() {
        let _ = writeln!(field, "    /// {}", p.doc.replace('\n', " "));
    }
    let _ = writeln!(
        field,
        "    pub {}: {},",
        p.name,
        spec.map_or("String", |k| k.rust)
    );
    let read = match spec {
        Some(k) if k.typed => format!("p.{}({})", k.word, lit(&p.name)),
        _ => format!("p.word({}).to_string()", lit(&p.name)),
    };
    let init = format!("            {}: {read},\n", p.name);
    let mut decl = match p.kind.as_str() {
        pk::BOOL => format!("Prop::bool({}, {})", lit(&p.name), p.default.trim()),
        pk::EVENT => format!("Prop::event({})", lit(&p.name)),
        pk::CHOICE => format!(
            "Prop::choice({}, &[{}], {})",
            lit(&p.name),
            p.options
                .iter()
                .map(|o| lit(o))
                .collect::<Vec<_>>()
                .join(", "),
            lit(&p.default)
        ),
        k => format!("Prop::{k}({}, {})", lit(&p.name), lit(&p.default)),
    };
    if !p.doc.is_empty() {
        let _ = write!(decl, ".doc({})", lit(&p.doc));
    }
    if !p.category.is_empty() {
        let _ = write!(decl, ".category({})", lit(&p.category));
    }
    (field, init, format!("        {decl},\n"))
}

/// The `.g.rs` source of a `<Component>` `.yui` (`name`: its path under
/// `src/` without the extension, as for screens).
pub(crate) fn generate(
    root: &Element,
    name: &str,
    mod_id: Option<&str>,
) -> Result<(String, Vec<Diagnostic>), Diagnostic> {
    let ty = type_name(root, name)?;
    let meta = Meta::read(root)?;
    let (props, body) = declarations(root)?;
    let Some(body) = body else {
        return err(
            root,
            "has no body — add one element (a Column, Row, Canvas, ...) after the declarations",
        );
    };
    let bound = bind_tree(body, &mut 0)?;
    let (tree, _ids) = yui::static_widget(&bound, false, 1, mod_id)?;
    Ok((component_code(name, &ty, &meta, &props, &tree), Vec::new()))
}

/// The component's struct, its view, and its `Component` implementation.
fn component_code(name: &str, ty: &str, meta: &Meta, props: &[Prop], tree: &str) -> String {
    let (mut fields, mut init, mut decls) = (String::new(), String::new(), String::new());
    for (f, i, d) in props.iter().map(prop_code) {
        fields.push_str(&f);
        init.push_str(&i);
        decls.push_str(&d);
    }
    let lit = |s: &str| format!("{s:?}");
    let doc_line = if meta.doc.is_empty() {
        format!("The `{ty}` component.")
    } else {
        meta.doc.replace('\n', " ")
    };
    let values = [
        ("NAME", name.to_string()),
        ("DOC_LINE", doc_line),
        ("TY", ty.to_string()),
        ("FIELDS", fields),
        ("INIT", init),
        ("UI_USE", UI_USE.to_string()),
        ("TREE", tree.to_string()),
        ("NAME_LIT", lit(ty)),
        ("DOC_LIT", lit(meta.doc)),
        ("DISPLAY_LIT", lit(meta.display_name)),
        ("ICON_LIT", lit(meta.icon)),
        ("CATEGORY_LIT", lit(meta.category)),
        ("DESIGN_W", format!("{:?}", meta.design.0)),
        ("DESIGN_H", format!("{:?}", meta.design.1)),
        ("TOOLBOX", meta.toolbox.to_string()),
        ("DECLS", decls),
    ];
    crate::project::rust_code::fill(TEMPLATE, &values)
}

/// The generated code of a component, its `$NAME$`s to fill.
const TEMPLATE: &str = include_str!("component.g.rs.in");
/// Whether `src` is a component `.yui` (its root is `<Component>`).
fn is_component_file(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| yui::parse_xml(&s).ok())
        .is_some_and(|r| r.name == yog_ids::yui::COMPONENT && !crate::ui::book_page::is_book_ui(&r))
}

/// What a mod's library says to compile in its components without a code-behind.
const COMPONENTS_MACRO: &str = "ui_components!";

/// Writes [`INDEX_FILE`]: a module per component `.yui` under `src/` that
/// has no code-behind `.rs` next to it (those `ui_include!` themselves).
/// With a warning when there are such components but `src/lib.rs` doesn't
/// call `yog_api::ui_components!()`.
pub fn write_index(root: &Path) -> Result<(PathBuf, Option<String>), Diagnostic> {
    let mut out = String::from(
        "// <auto-generated> by yog: component .yui files without a code-behind,\n\
         // compiled in by `yog_api::ui_components!()` — do not edit. </auto-generated>\n",
    );
    let src = root.join(yog_ids::project::SRC);
    let mut count = 0;
    for file in yui::find_yui_files(&src) {
        if file.with_extension("rs").is_file() || !is_component_file(&file) {
            continue;
        }
        let name = yui::include_name(root, &file);
        let module = yog_ids::rust_name(&name.replace(yog_ids::sep::PATH, yog_ids::sep::WORD));
        let _ = writeln!(
            out,
            // The module's `use super::*` is for a component that names the crate's items; most don't.
            "#[allow(unused_imports)]\npub mod {module} {{\n    use super::*;\n    include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{name}.g.rs\"));\n}}"
        );
        count += 1;
    }
    let lib = std::fs::read_to_string(src.join("lib.rs")).unwrap_or_default();
    let warning = (count > 0 && !lib.contains(COMPONENTS_MACRO))
        .then(|| format!("{count} component(s) without a code-behind aren't compiled in — add `yog_api::ui_components!();` to src/lib.rs"));
    let path = root.join(INDEX_FILE);
    crate::write_if_changed(&path, &out)?;
    Ok((path, warning))
}

#[cfg(test)]
mod tests;
