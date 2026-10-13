//! Component `.yui` files: a `<Component>` root declaring properties and
//! events, with one body element bound to them through `{x:Bind ...}` (the
//! same bindings as view-model screens, [`crate::yui_vm::compile_bind`]).
//! Generates a struct with one typed field per property and its
//! `yog_api::ui::component::Component` implementation, registered with
//! `ui_component!`. See Yog-IDLE's `docs/yui.md`, "Components".
//!
//! ```xml
//! <Component name="StatBar" description="A labelled bar." icon="item:experience_bottle"
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

use crate::yui::{self, err, Element, RAW_CALL, RAW_VALUE, UI_USE};

/// Index of the components without a code-behind, included by `ui_components!()`.
pub const INDEX_FILE: &str = ".yog-build/generated/components.g.rs";

/// The property types and their Rust field types.
const KINDS: &[(&str, &str)] = &[
    ("text", "String"),
    ("number", "f32"),
    ("int", "i64"),
    ("bool", "bool"),
    ("color", "u32"),
    ("choice", "String"),
    ("image", "String"),
];

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
    "name",
    "display_name",
    "description",
    "icon",
    "category",
    "design_w",
    "design_h",
    "toolbox",
];
/// A `<Property>`'s / `<Event>`'s attributes.
const PROP_ATTRS: &[&str] = &["name", "type", "default", "options", "doc", "category"];

fn is_ident(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The component's type name: `name=`, else the file's PascalCase stem.
fn type_name(root: &Element, include_name: &str) -> Result<String, String> {
    let name = match root.attr("name").map(str::trim).filter(|s| !s.is_empty()) {
        Some(n) => n.to_string(),
        None => yui::pascal(include_name.rsplit('/').next().unwrap_or(include_name)),
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

fn declarations(root: &Element) -> Result<(Vec<Prop>, Option<&Element>), String> {
    let mut props: Vec<Prop> = Vec::new();
    let mut body = None;
    for child in &root.children {
        match child.name.as_str() {
            "Property" | "Event" => {
                for (key, _) in &child.attrs {
                    let allowed = if child.name == "Event" {
                        &["name", "doc", "category"][..]
                    } else {
                        PROP_ATTRS
                    };
                    if !allowed.contains(&key.as_str()) {
                        return err(
                            child,
                            format_args!("has no attribute `{key}` ({})", allowed.join(", ")),
                        );
                    }
                }
                let name = child
                    .attr("name")
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
                        format_args!(
                            "`{name}` is an attribute every element has — pick another name"
                        ),
                    );
                }
                if props.iter().any(|p| p.name == name) {
                    return err(child, format_args!("`{name}` is declared twice"));
                }
                let kind = if child.name == "Event" {
                    "event".to_string()
                } else {
                    let kind = child.attr("type").unwrap_or("text").trim().to_string();
                    if !KINDS.iter().any(|(k, _)| *k == kind) {
                        let known: Vec<&str> = KINDS.iter().map(|(k, _)| *k).collect();
                        return err(
                            child,
                            format_args!("type=\"{kind}\": expected one of {}", known.join(", ")),
                        );
                    }
                    kind
                };
                let options: Vec<String> = child
                    .attr("options")
                    .unwrap_or_default()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if kind == "choice" && options.is_empty() {
                    return err(child, "a choice needs options=\"a, b, c\"");
                }
                let mut default =
                    child
                        .attr("default")
                        .map(str::to_string)
                        .unwrap_or_else(|| match kind.as_str() {
                            "number" | "int" => "0".into(),
                            "bool" => "false".into(),
                            "color" => "#FFFFFFFF".into(),
                            "choice" => options[0].clone(),
                            _ => String::new(),
                        });
                match kind.as_str() {
                    "number" if default.trim().parse::<f32>().is_err() => {
                        return err(
                            child,
                            format_args!("default=\"{default}\": expected a number"),
                        )
                    }
                    "int" if default.trim().parse::<i64>().is_err() => {
                        return err(
                            child,
                            format_args!("default=\"{default}\": expected a whole number"),
                        )
                    }
                    "bool" if !matches!(default.trim(), "true" | "false") => {
                        return err(
                            child,
                            format_args!("default=\"{default}\": expected true or false"),
                        )
                    }
                    "color" => {
                        // A literal or a `.yres` color, stored as its value.
                        let c = yui::color_attr(child, "default", &default)?;
                        default = format!("#{c:08X}");
                    }
                    "choice" if !options.contains(&default) => {
                        return err(
                            child,
                            format_args!("default=\"{default}\" isn't one of the options"),
                        )
                    }
                    _ => {}
                }
                props.push(Prop {
                    name,
                    kind,
                    default,
                    doc: child.attr("doc").unwrap_or_default().to_string(),
                    category: child.attr("category").unwrap_or_default().to_string(),
                    options,
                });
            }
            _ if body.is_some() => {
                return err(
                    child,
                    "a component has one body element (wrap several in a Column, Row or Canvas)",
                )
            }
            _ => body = Some(child),
        }
    }
    Ok((props, body))
}

/// The body with every `{x:Bind ...}` attribute turned into a builder call
/// (or, on a nested component, a Rust-valued property).
fn bind_tree(el: &Element, counter: &mut usize) -> Result<Element, String> {
    let kind = el.name.as_str();
    let builtin = yui::is_builtin(kind);
    if kind == "Component" || kind == "Screen" {
        return err(el, "can't be inside a component");
    }
    let mut out = Element {
        name: el.name.clone(),
        attrs: Vec::new(),
        children: Vec::new(),
        line: el.line,
    };
    for (key, value) in &el.attrs {
        if key.starts_with("d:") {
            continue; // design-time only (the designer's sample values)
        }
        let Some(rust) = crate::yui_vm::compile_bind(value) else {
            out.attrs.push((key.clone(), value.clone()));
            continue;
        };
        let e = rust.map_err(|e| format!("line {}: <{kind}> {key}=\"{value}\": {e}", el.line))?;
        let conv = "::yog_api::ui::component::bind";
        if !builtin && !yui::is_common_attr(key) {
            // A nested component's property: passed as text.
            out.attrs
                .push((key.clone(), format!("{RAW_VALUE}{conv}::text(&({e}))")));
            continue;
        }
        let call = match key.as_str() {
            "text" if matches!(kind, "Label" | "Button" | "TextBox") => {
                format!("with_text({conv}::text(&({e})))")
            }
            "placeholder" if kind == "TextBox" => format!("placeholder({conv}::text(&({e})))"),
            "source" if kind == "Surface" => {
                out.attrs.push(("source".into(), "yog:none".into()));
                format!("with_source({conv}::text(&({e})))")
            }
            "source" if kind == "Image" => {
                // A valid stand-in for the static checks; replaced at build.
                out.attrs.push(("source".into(), "item:air".into()));
                format!("with_source({conv}::text(&({e})))")
            }
            "item" if matches!(kind, "ItemSlot" | "InvSlot") => {
                format!("with_source({conv}::text(&({e})))")
            }
            "count" if kind == "InvSlot" => format!("with_count({conv}::count(&({e})))"),
            "bg" | "color" | "focus_color" => format!("{key}({conv}::color(&({e})))"),
            "tint" if kind == "Image" => format!("tint({conv}::color(&({e})))"),
            "enabled" | "visible" | "shadow" => format!("{key}({conv}::flag({e}))"),
            "w" | "h" => format!("{key}({conv}::units(&({e})))"),
            "gap" | "flex" | "min_w" | "min_h" | "font_scale" => {
                format!("{key}({conv}::number(&({e})))")
            }
            "on_click" | "on_change" | "on_submit" => format!("{key}({conv}::text(&({e})))"),
            _ => {
                return err(
                    el,
                    format_args!("{key}=\"{value}\": `{key}` can't be bound"),
                )
            }
        };
        out.attrs.push((format!("{RAW_CALL}{counter}"), call));
        *counter += 1;
    }
    for child in &el.children {
        out.children.push(bind_tree(child, counter)?);
    }
    Ok(out)
}

fn rust_str(s: &str) -> String {
    format!("{s:?}")
}

/// The `.g.rs` source of a `<Component>` `.yui` (`name`: its path under
/// `src/` without the extension, as for screens).
pub fn generate(
    root: &Element,
    name: &str,
    mod_id: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    let ty = type_name(root, name)?;
    for (key, _) in &root.attrs {
        if !ROOT_ATTRS.contains(&key.as_str()) && !key.starts_with("d:") {
            return err(
                root,
                format_args!("has no attribute `{key}` ({})", ROOT_ATTRS.join(", ")),
            );
        }
    }
    let size = |key: &str| -> Result<f32, String> {
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
    };
    let design = (size("design_w")?, size("design_h")?);
    let toolbox = match root.attr("toolbox").map(str::trim) {
        None | Some("true") => true,
        Some("false") => false,
        Some(v) => {
            return err(
                root,
                format_args!("toolbox=\"{v}\": expected true or false"),
            )
        }
    };
    let icon = root.attr("icon").unwrap_or_default().trim();
    if !icon.is_empty() && !icon.starts_with('@') && !icon.contains(':') {
        return err(root, format_args!("icon=\"{icon}\": expected an image source (item:apple, sprite:button, mod:gui/logo) or @Icons.name"));
    }
    let (props, body) = declarations(root)?;
    let Some(body) = body else {
        return err(
            root,
            "has no body — add one element (a Column, Row, Canvas, ...) after the declarations",
        );
    };
    let mut counter = 0;
    let bound = bind_tree(body, &mut counter)?;
    let (tree, _ids) = yui::static_widget(&bound, false, 1, mod_id)?;
    let doc = root.attr("description").unwrap_or_default();

    let mut fields = String::new();
    let mut init = String::new();
    let mut decls = String::new();
    for p in &props {
        let rust_ty = KINDS
            .iter()
            .find(|(k, _)| *k == p.kind)
            .map_or("String", |(_, t)| t);
        if !p.doc.is_empty() {
            let _ = writeln!(fields, "    /// {}", p.doc.replace('\n', " "));
        }
        let _ = writeln!(fields, "    pub {}: {rust_ty},", p.name);
        let read = match p.kind.as_str() {
            "text" => format!("p.text({})", rust_str(&p.name)),
            "number" => format!("p.number({})", rust_str(&p.name)),
            "int" => format!("p.int({})", rust_str(&p.name)),
            "bool" => format!("p.bool({})", rust_str(&p.name)),
            "color" => format!("p.color({})", rust_str(&p.name)),
            _ => format!("p.word({}).to_string()", rust_str(&p.name)),
        };
        let _ = writeln!(init, "            {}: {read},", p.name);
        let decl = match p.kind.as_str() {
            "bool" => format!("Prop::bool({}, {})", rust_str(&p.name), p.default.trim()),
            "event" => format!("Prop::event({})", rust_str(&p.name)),
            "choice" => format!(
                "Prop::choice({}, &[{}], {})",
                rust_str(&p.name),
                p.options
                    .iter()
                    .map(|o| rust_str(o))
                    .collect::<Vec<_>>()
                    .join(", "),
                rust_str(&p.default)
            ),
            k => format!("Prop::{k}({}, {})", rust_str(&p.name), rust_str(&p.default)),
        };
        let decl = if p.doc.is_empty() {
            decl
        } else {
            format!("{decl}.doc({})", rust_str(&p.doc))
        };
        let decl = if p.category.is_empty() {
            decl
        } else {
            format!("{decl}.category({})", rust_str(&p.category))
        };
        let _ = writeln!(decls, "        {decl},");
    }

    let code = format!(
        r#"// <auto-generated>
//     Generated by yog from src/{name}.yui — do not edit: changes are lost
//     on the next `yog build` / `yog check` / `yog ui gen`. Edit the .yui
//     file (Yog-IDLE's designer) instead.
// </auto-generated>

/// {doc_line}
#[derive(Debug, Clone, PartialEq)]
pub struct {ty} {{
{fields}}}

impl {ty} {{
    /// The property values of an instance (defaults for what it doesn't set).
    #[allow(unused_variables)]
    pub fn from_props(p: &::yog_api::ui::component::Props) -> Self {{
        Self {{
{init}        }}
    }}

    /// The component's widget tree for these values.
    #[allow(unused_variables, clippy::needless_borrow, clippy::needless_borrows_for_generic_args)]
    pub fn view(&self) -> ::yog_api::ui::Widget {{
        #[allow(unused_imports)]
        {UI_USE}
        let vm = self;
        {tree}
    }}
}}

impl ::yog_api::ui::component::Component for {ty} {{
    const NAME: &'static str = {name_lit};
    const DOC: &'static str = {doc_lit};
    const DISPLAY_NAME: &'static str = {display_lit};
    const ICON: &'static str = {icon_lit};
    const CATEGORY: &'static str = {category_lit};
    const DESIGN_SIZE: (f32, f32) = ({design_w:?}, {design_h:?});
    const TOOLBOX: bool = {toolbox};
    const PROPS: &'static [::yog_api::ui::component::Prop] = {{
        #[allow(unused_imports)]
        use ::yog_api::ui::component::Prop;
        &[
{decls}        ]
    }};
    fn build(props: &::yog_api::ui::component::Props) -> ::yog_api::ui::Widget {{
        Self::from_props(props).view()
    }}
}}

::yog_api::ui_component!({ty});
"#,
        doc_line = if doc.is_empty() {
            format!("The `{ty}` component.")
        } else {
            doc.replace('\n', " ")
        },
        name_lit = rust_str(&ty),
        doc_lit = rust_str(doc),
        display_lit = rust_str(root.attr("display_name").unwrap_or_default()),
        icon_lit = rust_str(icon),
        category_lit = rust_str(root.attr("category").unwrap_or_default()),
        design_w = design.0,
        design_h = design.1,
    );
    Ok((code, Vec::new()))
}

/// Whether `src` is a component `.yui` (its root is `<Component>`).
fn is_component_file(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| yui::parse_xml(&s).ok())
        .is_some_and(|r| r.name == "Component")
}

/// Writes [`INDEX_FILE`]: a module per component `.yui` under `src/` that
/// has no code-behind `.rs` next to it (those `ui_include!` themselves).
/// Warns when there are such components but `src/lib.rs` doesn't call
/// `yog_api::ui_components!()`.
pub fn write_index(root: &Path) -> Result<PathBuf, String> {
    let mut out = String::from(
        "// <auto-generated> by yog: component .yui files without a code-behind,\n\
         // compiled in by `yog_api::ui_components!()` — do not edit. </auto-generated>\n",
    );
    let mut count = 0;
    for file in yui::find_yui_files(&root.join("src")) {
        if file.with_extension("rs").is_file() || !is_component_file(&file) {
            continue;
        }
        let name = yui::include_name(root, &file);
        let module = name.replace(['/', '-'], "_");
        let _ = writeln!(
            out,
            "#[allow(unused_imports)]\npub mod {module} {{\n    use super::*;\n    include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{name}.g.rs\"));\n}}"
        );
        count += 1;
    }
    if count > 0 {
        let lib = std::fs::read_to_string(root.join("src/lib.rs")).unwrap_or_default();
        if !lib.contains("ui_components!") {
            eprintln!(
                "    warning: {count} component(s) without a code-behind aren't compiled in — add `yog_api::ui_components!();` to src/lib.rs"
            );
        }
    }
    let path = root.join(INDEX_FILE);
    crate::write_if_changed(&path, &out)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(src: &str) -> Result<String, String> {
        generate(&yui::parse_xml(src)?, "ui/stat_bar", None).map(|(c, _)| c)
    }

    #[test]
    fn generates_a_component() {
        let code = gen(r##"<Component description="A bar." icon="@Icons.bar" category="Bars" design_w="120" design_h="10" d:bg="#FF000000">
            <Property name="label" default="HP" doc="Left text" category="Text"/>
            <Property name="value" type="number" default="0.5"/>
            <Property name="fill" type="color" default="#FF55FF55"/>
            <Property name="size" type="choice" options="small, big"/>
            <Property name="wide" type="bool" default="true"/>
            <Event name="on_select"/>
            <Row w="100%" gap="4" on_click="{x:Bind on_select}">
                <Label text="{x:Bind label}" d:text="Sample" visible="{x:Bind wide}"/>
                <Spacer w="{x:Bind bar_w()}" h="6" bg="{x:Bind fill}"/>
                <Pill text="{x:Bind label}" tone="red"/>
            </Row>
        </Component>"##)
        .unwrap();
        for want in [
            "pub struct StatBar {",
            "    /// Left text\n    pub label: String,",
            "pub value: f32,",
            "pub fill: u32,",
            "label: p.text(\"label\"),",
            "size: p.word(\"size\").to_string(),",
            "Prop::text(\"label\", \"HP\").doc(\"Left text\").category(\"Text\")",
            "const DOC: &'static str = \"A bar.\";",
            "const ICON: &'static str = \"@Icons.bar\";",
            "const CATEGORY: &'static str = \"Bars\";",
            "const DESIGN_SIZE: (f32, f32) = (120.0, 10.0);",
            "const TOOLBOX: bool = true;",
            "Prop::choice(\"size\", &[\"small\", \"big\"], \"small\")",
            "Prop::bool(\"wide\", true)",
            "Prop::event(\"on_select\")",
            "const NAME: &'static str = \"StatBar\";",
            ".with_text(::yog_api::ui::component::bind::text(&(vm.label)))",
            ".visible(::yog_api::ui::component::bind::flag(vm.wide))",
            ".w(::yog_api::ui::component::bind::units(&(vm.bar_w())))",
            ".bg(::yog_api::ui::component::bind::color(&(vm.fill)))",
            ".on_click(::yog_api::ui::component::bind::text(&(vm.on_select)))",
            "component::build(\"Pill\", &[(\"text\", &*::yog_api::ui::component::bind::text(&(vm.label))), (\"tone\", \"red\")])",
            "::yog_api::ui_component!(StatBar);",
        ] {
            assert!(code.contains(want), "missing {want:?} in\n{code}");
        }
    }

    #[test]
    fn rejects_bad_components() {
        for (src, msg) in [
            (r#"<Component name="bad"><Row/></Component>"#, "PascalCase"),
            (
                r#"<Component><Property name="w"/><Row/></Component>"#,
                "every element has",
            ),
            (
                r#"<Component><Property name="halign"/><Row/></Component>"#,
                "every element has",
            ),
            (
                r#"<Component><Property name="margin"/><Row/></Component>"#,
                "every element has",
            ),
            (
                r#"<Component><Property name="a" type="float"/><Row/></Component>"#,
                "expected one of",
            ),
            (
                r#"<Component><Property name="a" type="number" default="x"/><Row/></Component>"#,
                "expected a number",
            ),
            (r#"<Component><Row/><Row/></Component>"#, "one body element"),
            (
                r#"<Component doc="x"><Row/></Component>"#,
                "no attribute `doc`",
            ),
            (
                r#"<Component design_w="wide"><Row/></Component>"#,
                "expected a size",
            ),
            (
                r#"<Component toolbox="no"><Row/></Component>"#,
                "expected true or false",
            ),
            (
                r#"<Component icon="apple"><Row/></Component>"#,
                "image source",
            ),
            (
                r#"<Component><Event name="on_x" type="text"/><Row/></Component>"#,
                "no attribute `type`",
            ),
            (
                r#"<Component><Property name="a"/></Component>"#,
                "has no body",
            ),
            (
                r#"<Component><Row padding="{x:Bind a}"/></Component>"#,
                "can't be bound",
            ),
            (
                r#"<Component><Row bg="{x:Bind cx.close()}"/></Component>"#,
                "cx",
            ),
        ] {
            let e = gen(src).unwrap_err();
            assert!(e.contains(msg), "{src}: {e}");
        }
    }

    #[test]
    fn index_lists_components_without_code_behind() {
        let dir = std::env::temp_dir().join(format!("yog-components-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src/ui")).unwrap();
        std::fs::write(dir.join("yog.toml"), "").unwrap();
        std::fs::write(
            dir.join("src/ui/pill.yui"),
            "<Component><Label/></Component>",
        )
        .unwrap();
        std::fs::write(dir.join("src/bar.yui"), "<Component><Label/></Component>").unwrap();
        std::fs::write(dir.join("src/bar.rs"), "").unwrap();
        std::fs::write(dir.join("src/main.yui"), r#"<Screen id="a:b"/>"#).unwrap();
        let index = std::fs::read_to_string(write_index(&dir).unwrap()).unwrap();
        assert!(
            index.contains("pub mod ui_pill {") && index.contains("/generated/ui/pill.g.rs"),
            "{index}"
        );
        assert!(!index.contains("bar") && !index.contains("main"), "{index}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
