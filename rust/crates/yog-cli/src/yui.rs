//! `.yui` screen files — the declarative form of a `yog-ui` widget tree that
//! Yog-IDLE's screen designer edits — and their code generation.
//!
//! A `.yui` file is a small XAML-like XML document: one element per widget,
//! attributes named after the `yog-ui` builder methods they become:
//!
//! ```xml
//! <Screen id="mymod:main_menu" width="176" height="166" anchor="center" bg="#C0101010" padding="8">
//!   <Label id="title" x="8" y="6" text="Main Menu" color="#FFFFFF" no_wrap="true"/>
//!   <Button x="38" y="40" w="100" h="20" text="Play" bg="#FF555555" on_click="play"/>
//!   <Column x="8" y="70" w="160" gap="2">
//!     <Label text="Flowing child"/>
//!   </Column>
//! </Screen>
//! ```
//!
//! `yog build` (and `yog check`, `yog ui gen`) turns `src/**/foo.yui` into
//! `.yog-build/generated/**/foo.g.rs` — WPF's `MainWindow.g.cs`: generated
//! into the build directory on every build, never committed, pulled into the
//! screen's hand-written code-behind module with `yog_api::ui_include!("foo")`.
//! See `docs/yui.md` in Yog-IDLE for the full format.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use yog_gfx::image::ImageSource;

// ── XML subset ────────────────────────────────────────────────────────────────

/// One XML element: name, attributes in source order, child elements.
#[derive(Debug, Clone)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// 1-based line of the start tag, for error messages.
    pub line: usize,
}

impl Element {
    pub(crate) fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn line(&self) -> usize {
        self.src[..self.pos].matches('\n').count() + 1
    }

    fn err<T>(&self, msg: impl std::fmt::Display) -> Result<T, String> {
        Err(format!("line {}: {msg}", self.line()))
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn skip_ws(&mut self) {
        let trimmed = self.rest().trim_start();
        self.pos = self.src.len() - trimmed.len();
    }

    /// Skips whitespace, comments, `<?...?>` and `<!DOCTYPE ...>`.
    fn skip_misc(&mut self) -> Result<(), String> {
        loop {
            self.skip_ws();
            let rest = self.rest();
            let end = if rest.starts_with("<!--") {
                rest.find("-->").map(|i| i + 3)
            } else if rest.starts_with("<?") {
                rest.find("?>").map(|i| i + 2)
            } else if rest.starts_with("<!") {
                rest.find('>').map(|i| i + 1)
            } else {
                return Ok(());
            };
            match end {
                Some(n) => self.pos += n,
                None => return self.err("unterminated comment or declaration"),
            }
        }
    }

    fn name(&mut self) -> Result<String, String> {
        let rest = self.rest();
        let len = rest
            .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')))
            .unwrap_or(rest.len());
        if len == 0 {
            return self.err("expected a name");
        }
        self.pos += len;
        Ok(rest[..len].to_string())
    }

    fn element(&mut self) -> Result<Element, String> {
        let line = self.line();
        if !self.rest().starts_with('<') {
            return self.err("expected an element");
        }
        self.pos += 1;
        let name = self.name()?;
        let mut attrs: Vec<(String, String)> = Vec::new();
        loop {
            self.skip_ws();
            let rest = self.rest();
            if rest.starts_with("/>") {
                self.pos += 2;
                return Ok(Element {
                    name,
                    attrs,
                    children: Vec::new(),
                    line,
                });
            }
            if rest.starts_with('>') {
                self.pos += 1;
                break;
            }
            if rest.is_empty() {
                return self.err(format!("unterminated <{name}>"));
            }
            let key = self.name()?;
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return self.err(format!("expected `=` after attribute `{key}`"));
            }
            self.pos += 1;
            self.skip_ws();
            let quote = match self.rest().chars().next() {
                Some(q @ ('"' | '\'')) => q,
                _ => return self.err(format!("attribute `{key}`: expected a quoted value")),
            };
            self.pos += 1;
            let Some(end) = self.rest().find(quote) else {
                return self.err(format!("attribute `{key}`: unterminated value"));
            };
            let raw = &self.rest()[..end];
            let value = unescape(raw).map_err(|e| format!("line {}: {e}", self.line()))?;
            self.pos += end + 1;
            if attrs.iter().any(|(k, _)| *k == key) {
                return self.err(format!("<{name}>: duplicate attribute `{key}`"));
            }
            attrs.push((key, value));
        }
        let mut children = Vec::new();
        loop {
            self.skip_misc()?;
            let rest = self.rest();
            if rest.starts_with("</") {
                self.pos += 2;
                let close = self.name()?;
                if close != name {
                    return self.err(format!("</{close}> closes <{name}>"));
                }
                self.skip_ws();
                if !self.rest().starts_with('>') {
                    return self.err("expected `>`");
                }
                self.pos += 1;
                return Ok(Element {
                    name,
                    attrs,
                    children,
                    line,
                });
            }
            if rest.is_empty() {
                return self.err(format!("<{name}> is never closed"));
            }
            if !rest.starts_with('<') {
                return self.err(format!(
                    "<{name}>: text content isn't allowed — use a `text` attribute"
                ));
            }
            children.push(self.element()?);
        }
    }
}

fn unescape(raw: &str) -> Result<String, String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let Some(semi) = rest[i..].find(';') else {
            return Err("`&` without `;` — write `&amp;`".into());
        };
        let entity = &rest[i + 1..i + semi];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse().ok()
                } else {
                    None
                };
                match code.and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    None => return Err(format!("unknown entity `&{entity};`")),
                }
            }
        }
        rest = &rest[i + semi + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Parses an XML document (the subset `.yui` uses) into its root element.
pub fn parse_xml(src: &str) -> Result<Element, String> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut p = Parser { src, pos: 0 };
    p.skip_misc()?;
    let root = p.element()?;
    p.skip_misc()?;
    if !p.rest().is_empty() {
        return p.err("content after the root element");
    }
    Ok(root)
}

// ── Screen model ──────────────────────────────────────────────────────────────

/// Whether `<kind>` is a mod's own component (not a built-in element).
pub(crate) fn is_custom_component(kind: &str) -> bool {
    !CONTAINERS.contains(&kind) && !LEAVES.contains(&kind) && is_component_name(kind)
}

/// Whether `key` is an attribute every element takes (not a component's
/// own property).
pub(crate) fn is_common_attr(key: &str) -> bool {
    COMMON_ATTRS.contains(&key) || key == "visible"
}

const CONTAINERS: &[&str] = &["Screen", "Canvas", "Column", "Row", "ScrollView", "Grid"];
const LEAVES: &[&str] = &[
    "Label", "Button", "TextBox", "ItemSlot", "InvSlot", "Image", "Spacer", "Surface",
];
const COMMON_ATTRS: &[&str] = &[
    "id",
    "x",
    "y",
    "w",
    "h",
    "min_w",
    "min_h",
    "flex",
    "dock",
    "gap",
    "padding",
    "bg",
    "color",
    "align",
    "halign",
    "valign",
    "margin",
    "font_scale",
    "font",
    "shadow",
    "no_wrap",
    "ellipsis",
    "focus_style",
    "focus_color",
    "enabled",
    "on_click",
];

fn kind_attrs(kind: &str) -> &'static [&'static str] {
    match kind {
        "Screen" => &["width", "height", "anchor", "modal", "pause"],
        "Label" | "Button" => &["text"],
        "TextBox" => &[
            "text",
            "placeholder",
            "max_length",
            "on_change",
            "on_submit",
        ],
        "ItemSlot" => &["item"],
        "InvSlot" => &["item", "count"],
        "Image" => &["source", "stretch", "uv", "tint"],
        "Surface" => &["source"],
        _ => &[],
    }
}

fn number(el: &Element, key: &str, value: &str) -> Result<f32, String> {
    value.trim().parse::<f32>().map_err(|_| {
        format!(
            "line {}: <{}> {key}=\"{value}\": expected a number",
            el.line, el.name
        )
    })
}

fn boolean(el: &Element, key: &str, value: &str) -> Result<bool, String> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!(
            "line {}: <{}> {key}=\"{value}\": expected true or false",
            el.line, el.name
        )),
    }
}

/// `#RRGGBB`, `#AARRGGBB` or `0xAARRGGBB` → `0xAARRGGBB`.
pub fn color(value: &str) -> Option<u32> {
    let v = value.trim();
    let hex = v
        .strip_prefix('#')
        .or_else(|| v.strip_prefix("0x"))
        .or_else(|| v.strip_prefix("0X"))?
        .replace('_', "");
    let n = u32::from_str_radix(&hex, 16).ok()?;
    match hex.len() {
        6 => Some(0xFF00_0000 | n),
        8 => Some(n),
        _ => None,
    }
}

/// A color attribute: a literal (see [`color`]) or `@<Set>.<name>`, a color
/// of `resources/<Set>.yres` (see [`set_resource_refs`]).
fn color_attr(el: &Element, key: &str, value: &str) -> Result<u32, String> {
    if let Some(reference) = value.trim().strip_prefix('@') {
        return RESOURCE_REFS
            .with(|r| r.borrow().colors.get(reference).copied())
            .ok_or_else(|| {
                format!(
                    "line {}: <{}> {key}=\"{value}\": no color `{reference}` in resources/*.yres",
                    el.line, el.name
                )
            });
    }
    color(value).ok_or_else(|| {
        format!(
            "line {}: <{}> {key}=\"{value}\": expected #RRGGBB, #AARRGGBB or @<Set>.<name>",
            el.line, el.name
        )
    })
}

fn f(v: f32) -> String {
    format!("{v:?}")
}

fn size(el: &Element, key: &str, value: &str) -> Result<Option<String>, String> {
    let v = value.trim();
    if v.is_empty() || v == "auto" {
        return Ok(None);
    }
    if let Some(p) = v.strip_suffix('%') {
        return Ok(Some(format!("Units::Percent({})", f(number(el, key, p)?))));
    }
    Ok(Some(format!("Units::Px({})", f(number(el, key, v)?))))
}

/// CSS-style shorthand: 1 value = all sides, 2 = vertical horizontal, 4 = top right bottom left.
fn padding(el: &Element, key: &str, value: &str) -> Result<[f32; 4], String> {
    let parts: Vec<f32> = value
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .map(|s| number(el, key, s))
        .collect::<Result<_, _>>()?;
    match parts.as_slice() {
        [a] => Ok([*a; 4]),
        [v, h] => Ok([*v, *h, *v, *h]),
        [t, r, b, l] => Ok([*t, *r, *b, *l]),
        _ => Err(format!(
            "line {}: <{}> {key}=\"{value}\": expected 1, 2 or 4 numbers",
            el.line, el.name
        )),
    }
}

/// `MinHeight` → `min_height`.
pub(crate) fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The `.yui` spelling of a WPF/WinUI attribute name, for error hints.
fn wpf_hint(key: &str) -> Option<&'static str> {
    Some(match key {
        "HorizontalAlignment" => "halign",
        "VerticalAlignment" => "valign",
        "Margin" => "margin",
        "Padding" => "padding",
        "Width" => "w",
        "Height" => "h",
        "MinWidth" => "min_w",
        "MinHeight" => "min_h",
        "Background" => "bg",
        "Foreground" => "color",
        "Text" | "Content" => "text",
        "RowDefinitions" => "rows",
        "ColumnDefinitions" => "columns",
        "RowSpacing" => "row_spacing",
        "ColumnSpacing" => "column_spacing",
        "Visibility" => "visible",
        "IsEnabled" => "enabled",
        _ => return None,
    })
}

fn pick<'v>(el: &Element, key: &str, value: &str, options: &[&'v str]) -> Result<&'v str, String> {
    options
        .iter()
        .find(|o| **o == value.trim())
        .copied()
        .ok_or_else(|| {
            format!(
                "line {}: <{}> {key}=\"{value}\": expected one of {}",
                el.line,
                el.name,
                options.join(", ")
            )
        })
}

fn pascal(value: &str) -> String {
    value
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// A Rust constant name for an event/widget id: `mymod:btn-play` → `MYMOD_BTN_PLAY`.
/// The Rust expression for a text attribute: `"@mymod.key"` is translation
/// key `mymod.key`, looked up when the tree is built (`::yog_api::tr`, so it
/// follows the game's language); `"@@..."` is a literal text starting
/// with `@`; anything else is the literal text.
///
/// `"@<Set>.<name>"` naming a text of `resources/<Set>.yres` (see
/// [`set_resource_refs`]) is that text's translation key.
/// `<StatBar …/>`: a custom component's name (not a built-in element).
fn is_component_name(kind: &str) -> bool {
    kind.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A custom component built by name from its own (non-common) attributes;
/// `@key` values are translated like `text`.
fn component_expr(el: &Element) -> String {
    let props: Vec<String> = el
        .attrs
        .iter()
        .filter(|(k, _)| !COMMON_ATTRS.contains(&k.as_str()))
        .map(|(k, v)| {
            if let Some(rust) = v.strip_prefix(RAW_VALUE) {
                return format!("({k:?}, &*{rust})");
            }
            match v.strip_prefix('@') {
                Some(rest) if !rest.trim().is_empty() => format!("({k:?}, &*{})", text_expr(v)),
                _ => format!("({k:?}, {v:?})"),
            }
        })
        .collect();
    format!(
        "::yog_api::ui::component::build({:?}, &[{}])",
        el.name,
        props.join(", ")
    )
}

pub fn text_expr(value: &str) -> String {
    match value.strip_prefix('@') {
        Some(rest) if rest.starts_with('@') => format!("{rest:?}"),
        Some(key) if !key.trim().is_empty() => {
            let key = key.trim();
            let resolved =
                RESOURCE_REFS.with(|r| r.borrow().texts.get(key).map(|(k, _)| k.clone()));
            format!("::yog_api::tr({:?})", resolved.as_deref().unwrap_or(key))
        }
        _ => format!("{value:?}"),
    }
}

thread_local! {
    /// The mod's `.yres` resources, for `@<Set>.<name>` in attributes.
    static RESOURCE_REFS: std::cell::RefCell<crate::yres::Refs> = Default::default();
}

/// Makes the mod's compiled `.yres` resources visible to the generator:
/// `text="@Strings.title"` and `source="@Icons.logo"` resolve through them.
pub fn set_resource_refs(refs: &crate::yres::Refs) {
    RESOURCE_REFS.with(|r| *r.borrow_mut() = refs.clone());
}

pub fn const_name(value: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_lower {
                out.push('_');
            }
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            out.push(c.to_ascii_uppercase());
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
            prev_lower = false;
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

struct Gen {
    events: Vec<String>,
    ids: Vec<String>,
    /// The mod's id (from `yog.toml`), for `mod:` image sources.
    mod_id: Option<String>,
    /// `line N: ...` warnings.
    warnings: Vec<String>,
}

impl Gen {
    /// `widget::surface(...)` for a `<Surface>`: the id its drawing is
    /// registered under (`ns:path`; `mod:path` = the mod's namespace).
    fn surface(&mut self, el: &Element) -> Result<String, String> {
        let line = el.line;
        let source = el
            .attr("source")
            .ok_or_else(|| format!("line {line}: <Surface> needs a source (the id its code registers, e.g. source=\"mod:grid\")"))?
            .trim();
        let id = match source.strip_prefix("mod:") {
            Some(path) => match &self.mod_id {
                Some(m) => format!("{m}:{path}"),
                None => {
                    return Err(format!(
                        "line {line}: <Surface> source=\"{source}\": mod: needs the mod id (no yog.toml found)"
                    ))
                }
            },
            None => source.to_string(),
        };
        if !id.contains(':') || !crate::content::valid_id(&id) {
            return Err(format!(
                "line {line}: <Surface> source=\"{source}\": expected <namespace>:<name> or mod:<name> \
                 (lower case a-z, 0-9, `_`, `-`, `.`, `/`)"
            ));
        }
        Ok(format!("widget::surface({id:?})"))
    }

    /// `widget::image(...)` for an `<Image>`: its `source` checked and
    /// `mod:` resolved, plus `stretch`/`uv`/`tint`.
    fn image(&mut self, el: &Element) -> Result<String, String> {
        let line = el.line;
        let source = el
            .attr("source")
            .ok_or_else(|| {
                format!("line {line}: <Image> needs a source (e.g. source=\"item:apple\")")
            })?
            .trim();
        let resolved;
        let source = match source.strip_prefix('@') {
            Some(name) => {
                resolved = RESOURCE_REFS
                    .with(|r| r.borrow().images.get(name).cloned())
                    .ok_or_else(|| format!("line {line}: <Image> source=\"{source}\": no image `{name}` in resources/*.yres"))?;
                resolved.as_str()
            }
            None => source,
        };
        let parsed = ImageSource::parse(source).ok_or_else(|| {
            format!(
                "line {line}: <Image> source=\"{source}\": expected item:<id>, block:<id>, \
                 sprite:<name>, mod:<path> or <namespace>:<path>"
            )
        })?;
        let source = match parsed {
            ImageSource::ModPath(path) => match &self.mod_id {
                Some(id) => yog_gfx::image::texture_id(id, &path),
                None => {
                    return Err(format!(
                        "line {line}: <Image> source=\"{source}\": mod: needs the mod id (no yog.toml found)"
                    ))
                }
            },
            ImageSource::Sprite(name) if !name.contains(':') && yog_gfx::image::sprite(&name).is_none() => {
                let names: Vec<&str> = yog_gfx::image::SPRITES.iter().map(|s| s.name).collect();
                return Err(format!(
                    "line {line}: <Image> source=\"{source}\": unknown sprite (known: {})",
                    names.join(", ")
                ));
            }
            ImageSource::Texture(_) => {
                if let Some(alias) = yog_gfx::image::suggest_alias(source) {
                    return Err(format!(
                        "line {line}: <Image> source=\"{source}\" is a raw resource path, which \
                         changes between Minecraft versions — write source=\"{alias}\""
                    ));
                }
                source.to_string()
            }
            _ => source.to_string(),
        };
        let mut expr = format!("widget::image({source:?})");
        if let Some(v) = el.attr("stretch") {
            let s = match v.trim().to_ascii_lowercase().replace('_', "").as_str() {
                "none" => "None",
                "fill" => "Fill",
                "uniform" => "Uniform",
                "uniformtofill" => "UniformToFill",
                _ => {
                    return Err(format!(
                        "line {line}: <Image> stretch=\"{v}\": expected None, Fill, Uniform or UniformToFill"
                    ))
                }
            };
            let _ = write!(expr, ".stretch(::yog_api::ui::Stretch::{s})");
        }
        if let Some(v) = el.attr("uv") {
            let nums: Vec<f32> = v
                .split([',', ' '])
                .filter(|p| !p.trim().is_empty())
                .map(|p| number(el, "uv", p))
                .collect::<Result<_, _>>()?;
            let [u, vv, w, h] = nums[..] else {
                return Err(format!(
                    "line {line}: <Image> uv=\"{v}\": expected \"u v width height\""
                ));
            };
            let _ = write!(expr, ".region([{}, {}, {}, {}])", f(u), f(vv), f(w), f(h));
        }
        if let Some(v) = el.attr("tint") {
            let _ = write!(expr, ".tint(0x{:08X})", color_attr(el, "tint", v)?);
        }
        Ok(expr)
    }

    /// The builder expression for `el`. `in_canvas`: the parent positions
    /// every child absolutely (`Screen`/`Canvas`) — a missing x/y means 0.
    fn widget(&mut self, el: &Element, in_canvas: bool, depth: usize) -> Result<String, String> {
        let kind = el.name.as_str();
        let is_container = CONTAINERS.contains(&kind);
        // Any other PascalCase element is one of the mod's own components
        // (`yog_api::ui_component!`), looked up by name when the tree is built.
        let is_component = !is_container && !LEAVES.contains(&kind) && is_component_name(kind);
        if !is_container && !LEAVES.contains(&kind) && !is_component {
            return Err(format!(
                "line {}: unknown element <{kind}> (expected one of {}, {}, or a custom component's PascalCase name)",
                el.line,
                CONTAINERS[1..].join(", "),
                LEAVES.join(", ")
            ));
        }
        if kind == "Screen" && depth > 0 {
            return Err(format!(
                "line {}: <Screen> must be the root element",
                el.line
            ));
        }
        if !is_container && !el.children.is_empty() {
            return Err(format!("line {}: <{kind}> can't have children", el.line));
        }
        for (key, _) in &el.attrs {
            if !is_component
                && !key.starts_with(RAW_CALL)
                && !COMMON_ATTRS.contains(&key.as_str())
                && !kind_attrs(kind).contains(&key.as_str())
            {
                let hint = wpf_hint(key)
                    .map(|h| format!(" — .yui writes it `{h}`"))
                    .unwrap_or_default();
                return Err(format!(
                    "line {}: <{kind}> has no attribute `{key}`{hint}",
                    el.line
                ));
            }
        }
        let text = || text_expr(el.attr("text").unwrap_or_default());
        let mut expr = match kind {
            "Screen" | "Canvas" | "Column" => "widget::panel(FlexDir::Column)".to_string(),
            "Row" => "widget::panel(FlexDir::Row)".to_string(),
            "ScrollView" => "widget::scroll_view()".to_string(),
            "Grid" => "widget::grid()".to_string(),
            "Label" => format!("widget::label({})", text()),
            "Button" => format!("widget::button({})", text()),
            "TextBox" => format!("widget::text_box({})", text()),
            "ItemSlot" => format!(
                "widget::item_slot({:?})",
                el.attr("item").unwrap_or_default()
            ),
            "InvSlot" => {
                let count = match el.attr("count") {
                    Some(c) => c.trim().parse::<u32>().map_err(|_| {
                        format!(
                            "line {}: <InvSlot> count=\"{c}\": expected a whole number",
                            el.line
                        )
                    })?,
                    None => 1,
                };
                format!(
                    "widget::inv_slot({:?}, {count})",
                    el.attr("item").unwrap_or_default()
                )
            }
            "Image" => self.image(el)?,
            "Surface" => self.surface(el)?,
            _ if is_component => component_expr(el),
            _ => "widget::spacer()".to_string(),
        };
        let indent = "    ".repeat(depth + 2);
        let mut call = |s: String| {
            let _ = write!(expr, "\n{indent}.{s}");
        };
        if kind == "Screen" {
            if let Some(v) = el.attr("width") {
                call(format!("w(Units::Px({}))", f(number(el, "width", v)?)));
            }
            if let Some(v) = el.attr("height") {
                call(format!("h(Units::Px({}))", f(number(el, "height", v)?)));
            }
        }
        let x = el.attr("x").map(|v| number(el, "x", v)).transpose()?;
        let y = el.attr("y").map(|v| number(el, "y", v)).transpose()?;
        if in_canvas || x.is_some() || y.is_some() {
            call(format!(
                "at({}, {})",
                f(x.unwrap_or(0.0)),
                f(y.unwrap_or(0.0))
            ));
        }
        for (key, value) in &el.attrs {
            let v = value.as_str();
            if key.starts_with(RAW_CALL) {
                // Builder calls added by the view-model generator.
                call(value.clone());
                continue;
            }
            if is_component && !COMMON_ATTRS.contains(&key.as_str()) {
                // A component's own property (passed to `build` above); an
                // `on_*` one names an event like `on_click` does.
                if key.starts_with("on_") && !v.trim().is_empty() && !self.events.contains(value) {
                    self.events.push(value.clone());
                }
                continue;
            }
            match key.as_str() {
                "id" => {
                    if !self.ids.contains(value) {
                        self.ids.push(value.clone());
                    }
                    call(format!("id({v:?})"));
                }
                "on_click" | "on_change" | "on_submit" => {
                    if !self.events.contains(value) {
                        self.events.push(value.clone());
                    }
                    call(format!("{key}({v:?})"));
                }
                "placeholder" => call(format!("placeholder({})", text_expr(v))),
                "font" => call(format!("font({:?})", v.trim())),
                "max_length" => {
                    let n = v.trim().parse::<usize>().map_err(|_| {
                        format!(
                            "line {}: <{kind}> max_length=\"{v}\": expected a whole number",
                            el.line
                        )
                    })?;
                    call(format!("max_length({n})"));
                }
                "w" | "h" => {
                    if let Some(s) = size(el, key, v)? {
                        call(format!("{key}({s})"));
                    }
                }
                "min_w" | "min_h" | "flex" | "gap" | "font_scale" => {
                    call(format!("{key}({})", f(number(el, key, v)?)));
                }
                "dock" => {
                    let d = pick(
                        el,
                        key,
                        v,
                        &["none", "fill", "left", "right", "top", "bottom"],
                    )?;
                    call(format!("dock(Dock::{})", pascal(d)));
                }
                "align" => {
                    let a = pick(el, key, v, &["start", "center", "end"])?;
                    call(format!("align(Align::{})", pascal(a)));
                }
                "focus_style" => {
                    let s = pick(el, key, v, &["outline", "fill", "none"])?;
                    call(format!("focus_style(FocusStyle::{})", pascal(s)));
                }
                "padding" | "margin" => {
                    let [t, r, b, l] = padding(el, key, v)?;
                    call(format!("{key}({}, {}, {}, {})", f(t), f(r), f(b), f(l)));
                }
                "halign" => {
                    let a = pick(el, key, v, &["left", "center", "right", "stretch"])?;
                    call(format!("halign(::yog_api::ui::HAlign::{})", pascal(a)));
                }
                "valign" => {
                    let a = pick(el, key, v, &["top", "center", "bottom", "stretch"])?;
                    call(format!("valign(::yog_api::ui::VAlign::{})", pascal(a)));
                }
                "bg" | "color" | "focus_color" => {
                    call(format!("{key}(0x{:08X})", color_attr(el, key, v)?));
                }
                "shadow" | "enabled" => call(format!("{key}({})", boolean(el, key, v)?)),
                "no_wrap" if boolean(el, key, v)? => call("no_wrap()".to_string()),
                "ellipsis" if boolean(el, key, v)? => call("ellipsis()".to_string()),
                _ => {} // kind-specific, handled above (or Screen metadata)
            }
        }
        if !el.children.is_empty() {
            let canvas = matches!(kind, "Screen" | "Canvas");
            for child in &el.children {
                let c = self.widget(child, canvas, depth + 1)?;
                let _ = write!(expr, "\n{indent}.child({c})");
            }
        }
        Ok(expr)
    }
}

/// Prefix of a custom component property's value that is a Rust
/// expression (a `String`) rather than text — set by the component
/// generator for bound properties; never in a `.yui`.
pub(crate) const RAW_VALUE: &str = "\u{1}rs:";

/// Attribute-name prefix of a raw builder call (`__rs0="action(...)"`)
/// that the view-model generator attaches to elements; never in a `.yui`.
pub(crate) const RAW_CALL: &str = "__rs";

/// The builder expression of `el` and its subtree (ids found, warnings) —
/// the static half of a view-model screen.
pub(crate) fn static_widget(
    el: &Element,
    in_canvas: bool,
    depth: usize,
    mod_id: Option<&str>,
) -> Result<(String, Vec<String>, Vec<String>), String> {
    let mut gen = Gen {
        events: Vec::new(),
        ids: Vec::new(),
        mod_id: mod_id.map(str::to_string),
        warnings: Vec::new(),
    };
    let expr = gen.widget(el, in_canvas, depth)?;
    Ok((expr, gen.ids, gen.warnings))
}

/// Generates the `.g.rs` source for a `.yui` document. `name` is the screen's
/// path under `src/` without the extension (`mod_list`, `screens/options`):
/// what the code-behind passes to `ui_include!`.
#[cfg_attr(not(test), allow(dead_code))] // the tests' entry point
pub fn generate(src: &str, name: &str) -> Result<String, String> {
    generate_for(src, name, None).map(|(code, _)| code)
}

/// [`generate`] for the mod `mod_id` (resolves `mod:` image sources), also
/// returning `line N: ...` warnings.
pub fn generate_for(
    src: &str,
    name: &str,
    mod_id: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    let mut root = parse_xml(src)?;
    crate::yui_grid::normalize(&mut root)?;
    if root.name == "Component" {
        return crate::yui_component::generate(&root, name, mod_id);
    }
    if root.attr("x:ViewModel").is_some() {
        return crate::yui_vm::generate(&root, name, mod_id);
    }
    if root.name != "Screen" {
        return Err(format!(
            "line {}: the root element must be <Screen>, not <{}>",
            root.line, root.name
        ));
    }
    let id = root
        .attr("id")
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "line {}: <Screen> needs an id (the UI id, e.g. \"mymod:main\")",
                root.line
            )
        })?
        .to_string();
    let width = root
        .attr("width")
        .map(|v| number(&root, "width", v))
        .transpose()?
        .unwrap_or(176.0);
    let height = root
        .attr("height")
        .map(|v| number(&root, "height", v))
        .transpose()?
        .unwrap_or(166.0);
    let anchor = pick(
        &root,
        "anchor",
        root.attr("anchor").unwrap_or("center"),
        &[
            "center",
            "top_left",
            "top",
            "top_right",
            "left",
            "right",
            "bottom_left",
            "bottom",
            "bottom_right",
            "fill",
        ],
    )?;
    let modal = root
        .attr("modal")
        .map(|v| boolean(&root, "modal", v))
        .transpose()?
        .unwrap_or(true);
    let pause = root
        .attr("pause")
        .map(|v| boolean(&root, "pause", v))
        .transpose()?
        .unwrap_or(false);

    let mut gen = Gen {
        events: Vec::new(),
        ids: Vec::new(),
        mod_id: mod_id.map(str::to_string),
        warnings: Vec::new(),
    };
    // The screen's own id is the UI id, not a widget id.
    let mut content_root = root.clone();
    content_root.attrs.retain(|(k, _)| k != "id");
    let body = gen.widget(&content_root, false, 0)?;

    let (ax, ay) = match anchor {
        "top_left" => ("0.0", "0.0"),
        "top" => ("((screen_w - WIDTH) / 2.0).floor()", "0.0"),
        "top_right" => ("screen_w - WIDTH", "0.0"),
        "left" => ("0.0", "((screen_h - HEIGHT) / 2.0).floor()"),
        "right" => ("screen_w - WIDTH", "((screen_h - HEIGHT) / 2.0).floor()"),
        "bottom_left" => ("0.0", "screen_h - HEIGHT"),
        "bottom" => ("((screen_w - WIDTH) / 2.0).floor()", "screen_h - HEIGHT"),
        "bottom_right" => ("screen_w - WIDTH", "screen_h - HEIGHT"),
        _ => (
            "((screen_w - WIDTH) / 2.0).floor()",
            "((screen_h - HEIGHT) / 2.0).floor()",
        ),
    };
    let place = if anchor == "fill" {
        "content.w(Units::Percent(100.0)).h(Units::Percent(100.0)).at(0.0, 0.0)".to_string()
    } else {
        format!("content.at(({ax}).max(0.0), ({ay}).max(0.0))")
    };

    let consts = |names: &[String]| -> String {
        let mut out = String::new();
        let mut used: Vec<String> = Vec::new();
        for v in names {
            let mut name = const_name(v);
            while used.contains(&name) {
                name.push('_');
            }
            used.push(name.clone());
            let _ = writeln!(out, "    pub const {name}: &str = {v:?};");
        }
        out
    };

    let mut out = String::new();
    let _ = write!(
        out,
        r#"// <auto-generated>
//     Generated by yog from src/{name}.yui — do not edit: changes are lost
//     on the next `yog build` / `yog check` / `yog ui gen`. Edit the .yui
//     file (Yog-IDLE's screen designer) instead.
// </auto-generated>
//
// Pulled into the screen's code-behind module with:
//
//     yog_api::ui_include!("{name}");

/// The UI id this screen registers under (`register_ui`, `on_ui_render`, `open_ui`).
#[allow(dead_code)]
pub const ID: &str = {id:?};
/// Design-time size in GUI pixels.
#[allow(dead_code)]
pub const WIDTH: f32 = {width};
#[allow(dead_code)]
pub const HEIGHT: f32 = {height};

/// `on_click`/`on_change`/`on_submit` events raised by this screen's widgets.
#[allow(dead_code)]
pub mod events {{
{events}}}

/// Ids of this screen's named widgets (for `Widget::find_mut`, `UiRoot::find_rect`).
#[allow(dead_code)]
pub mod ids {{
{ids}}}

/// The screen's widget tree, positioned at (0, 0).
#[allow(dead_code)]
pub fn build() -> ::yog_api::ui::Widget {{
    #[allow(unused_imports)]
    use ::yog_api::ui::{{widget, Align, Dock, FlexDir, FocusStyle, Units}};
    {body}
}}

/// The screen laid out for a `screen_w` × `screen_h` GUI, anchored {anchor}
/// — `customize` may change the tree first (dynamic text, enabled state, ...).
#[allow(dead_code)]
pub fn root_with(
    screen_w: f32,
    screen_h: f32,
    customize: impl FnOnce(&mut ::yog_api::ui::Widget),
) -> ::yog_api::ui::UiRoot {{
    use ::yog_api::ui::{{widget, FlexDir, Units}};
    let mut content = build();
    customize(&mut content);
    let screen = widget::panel(FlexDir::Column)
        .w(Units::Px(screen_w))
        .h(Units::Px(screen_h))
        .child({place});
    let mut ui = ::yog_api::ui::UiRoot::new(ID, screen);
    ui.layout(screen_w, screen_h);
    ui
}}

/// [`root_with`] without changes.
#[allow(dead_code)]
pub fn root(screen_w: f32, screen_h: f32) -> ::yog_api::ui::UiRoot {{
    root_with(screen_w, screen_h, |_| {{}})
}}

static __YUI_LAST: ::std::sync::Mutex<Option<::yog_api::ui::UiRoot>> = ::std::sync::Mutex::new(None);

/// Draws the screen — call from `on_ui_render(ID, ...)`. Remembers the
/// layout for [`click`].
#[allow(dead_code)]
pub fn render(gfx: &::yog_api::GfxContext) {{
    render_with(gfx, |_| {{}})
}}

/// [`render`], changing the tree first (see [`root_with`]).
#[allow(dead_code)]
pub fn render_with(gfx: &::yog_api::GfxContext, customize: impl FnOnce(&mut ::yog_api::ui::Widget)) {{
    let (w, h) = gfx.screen_size();
    let ui = root_with(w as f32, h as f32, customize);
    ui.render(gfx);
    if let Ok(mut last) = __YUI_LAST.lock() {{
        *last = Some(ui);
    }}
}}

/// The `on_click` event of the widget under a `"click:X:Y"` UI event (what
/// `register_ui`'s handler receives) — `None` for other events or a miss.
#[allow(dead_code)]
pub fn click(event: &str) -> Option<String> {{
    let mut parts = event.strip_prefix("click:")?.split(':');
    let x: f32 = parts.next()?.parse().ok()?;
    let y: f32 = parts.next()?.parse().ok()?;
    let last = __YUI_LAST.lock().ok()?;
    last.as_ref()?.hit_test(x, y)?.on_click.clone()
}}

/// Runs `f` on the layout drawn by the last [`render`] — for reading
/// rects and scroll state (`UiRoot::find_node`) when handling UI events.
/// `None` before the first render.
#[allow(dead_code)]
pub fn with_last<R>(f: impl FnOnce(&::yog_api::ui::UiRoot) -> R) -> Option<R> {{
    let last = __YUI_LAST.lock().ok()?;
    last.as_ref().map(f)
}}

/// Feeds a UI event to the text box `id` (one of [`ids`]) as laid out by the
/// last [`render`] — call from `register_ui`'s handler; pass the same state
/// to `TextBoxState::apply` in [`render_with`]'s customize. `Ignored` before
/// the first render or for an unknown id.
#[allow(dead_code)]
pub fn text_box(
    id: &str,
    state: &mut ::yog_api::ui::TextBoxState,
    event: &str,
) -> ::yog_api::ui::TextBoxEvent {{
    with_last(|ui| ui.find_node(id).map(|node| state.handle(event, node)))
        .flatten()
        .unwrap_or(::yog_api::ui::TextBoxEvent::Ignored)
}}

/// Opens the screen (modal: {modal}, pauses the game: {pause}).
#[allow(dead_code)]
pub fn open() {{
    ::yog_api::open_ui(ID, {modal}, {pause});
}}

/// Closes the screen if it's open (what Esc does) — for a Done/Cancel
/// button. Takes effect after the current event handler returns.
#[allow(dead_code)]
pub fn close() {{
    ::yog_api::close_ui(ID);
}}
"#,
        width = f(width),
        height = f(height),
        events = consts(&gen.events),
        ids = consts(&gen.ids),
    );
    Ok((out, gen.warnings))
}

/// Where generated screen code goes, under the project root.
pub const GENERATED_DIR: &str = ".yog-build/generated";

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

/// The project root (the folder with `yog.toml`) a `.yui` file belongs to.
pub fn project_root(yui: &Path) -> Option<PathBuf> {
    absolute(yui)
        .ancestors()
        .skip(1)
        .find(|d| d.join("yog.toml").is_file())
        .map(Path::to_path_buf)
}

/// `foo.yui`'s name for `ui_include!`: its path under `src/` without the
/// extension, `/`-separated (`mod_list`, `screens/options`).
pub fn include_name(root: &Path, yui: &Path) -> String {
    let abs = absolute(yui);
    let rel = abs
        .strip_prefix(root.join("src"))
        .or_else(|_| abs.strip_prefix(root))
        .unwrap_or(&abs)
        .with_extension("");
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Where `src/<name>.yui`'s generated code goes:
/// `.yog-build/generated/<name>.g.rs`.
pub fn generated_path(root: &Path, name: &str) -> PathBuf {
    root.join(GENERATED_DIR).join(format!("{name}.g.rs"))
}

/// Generates `.yog-build/generated/<name>.g.rs` for one `.yui` file of the
/// project at `root` — only rewritten when the output changed, so cargo
/// doesn't rebuild for nothing.
pub fn generate_file(root: &Path, yui: &Path) -> Result<PathBuf, String> {
    let src = std::fs::read_to_string(yui).map_err(|e| format!("{}: {e}", yui.display()))?;
    let name = include_name(root, yui);
    let mod_id = std::fs::read_to_string(root.join("yog.toml"))
        .ok()
        .and_then(|t| crate::manifest::parse_yog_toml(&t).ok())
        .map(|t| t.id);
    let (code, warnings) = generate_for(&src, &name, mod_id.as_deref())
        .map_err(|e| format!("{}: {e}", yui.display()))?;
    for w in warnings {
        eprintln!("    warning: {}: {w}", yui.display());
    }
    let out = generated_path(root, &name);
    if std::fs::read_to_string(&out).ok().as_deref() != Some(code.as_str()) {
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&out, code).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(out)
}

/// Every `*.<ext>` file under `dir`, recursively.
fn find_files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(find_files(&path, ext));
        } else if path.extension().is_some_and(|e| e == ext) {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// Every `.yui` file under `dir`, recursively.
pub fn find_yui_files(dir: &Path) -> Vec<PathBuf> {
    find_files(dir, "yui")
}

/// `yog build`'s step: regenerates every `src/**/*.yui`'s `.g.rs` and drops
/// generated files whose `.yui` is gone.
pub fn generate_all(root: &Path) -> Result<usize, String> {
    let files = find_yui_files(&root.join("src"));
    let mut wanted = Vec::new();
    for file in &files {
        wanted.push(generate_file(root, file)?);
    }
    // Components without a code-behind, for `ui_components!()`.
    wanted.push(crate::yui_component::write_index(root)?);
    // `.yres` accessors live there too (`crate::yres` cleans those up).
    let res_index = root.join(crate::yres::GENERATED_FILE);
    let res_dir = root.join(crate::yres::GENERATED_DIR);
    // So do `content/` blocks and items (`crate::content`).
    let content_file = root.join(crate::content::GENERATED_FILE);
    for stale in find_files(&root.join(GENERATED_DIR), "rs") {
        if stale.to_string_lossy().ends_with(".g.rs")
            && !wanted.contains(&stale)
            && stale != res_index
            && stale != content_file
            && !stale.starts_with(&res_dir)
        {
            let _ = std::fs::remove_file(&stale);
        }
    }
    Ok(files.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(body: &str) -> String {
        format!(r#"<Screen id="mymod:s">{body}</Screen>"#)
    }

    #[test]
    fn custom_components() {
        let (code, _) = generate_for(
            &screen(r##"<StatBar x="4" w="80" label="@@hp" value="0.5" on_select="pick" bg="#FF000000"/>"##),
            "s",
            None,
        )
        .unwrap();
        assert!(
            code.contains(r#"::yog_api::ui::component::build("StatBar", &[("label", &*"@hp"), ("value", "0.5"), ("on_select", "pick")])"#),
            "{code}"
        );
        assert!(
            code.contains(".bg(0xFF000000)") && code.contains(".w(Units::Px(80.0))"),
            "{code}"
        );
        assert!(!code.contains(".value("), "{code}");
        assert!(generate_for(&screen(r#"<statBar/>"#), "s", None).is_err());
        assert!(generate_for(&screen(r#"<StatBar><Label/></StatBar>"#), "s", None).is_err());
    }

    #[test]
    fn surfaces() {
        let (code, _) = generate_for(
            &screen(r#"<Surface w="100" h="50" source="mod:grid"/>"#),
            "s",
            Some("hexcasting"),
        )
        .unwrap();
        assert!(
            code.contains(r#"widget::surface("hexcasting:grid")"#),
            "{code}"
        );
        let (code, _) =
            generate_for(&screen(r#"<Surface source="other:pad"/>"#), "s", None).unwrap();
        assert!(code.contains(r#"widget::surface("other:pad")"#), "{code}");
        assert!(generate_for(&screen(r#"<Surface/>"#), "s", None).is_err());
        assert!(generate_for(&screen(r#"<Surface source="pad"/>"#), "s", None).is_err());
        assert!(generate_for(&screen(r#"<Surface source="mod:pad"/>"#), "s", None).is_err());
        assert!(generate_for(&screen(r#"<Surface source="a:b" text="x"/>"#), "s", None).is_err());
    }

    #[test]
    fn image_sources() {
        let (code, warnings) = generate_for(
            &screen(r##"<Image w="156" h="29" source="item:apple" stretch="UniformToFill" tint="#80FFFFFF"/>"##),
            "s",
            None,
        )
        .unwrap();
        assert!(code.contains(r#"widget::image("item:apple").stretch(::yog_api::ui::Stretch::UniformToFill).tint(0x80FFFFFF)"#), "{code}");
        assert!(
            code.contains(".w(Units::Px(156.0))") || code.contains("w(156"),
            "{code}"
        );
        assert!(warnings.is_empty(), "{warnings:?}");

        let (code, _) = generate_for(
            &screen(r#"<Image source="mod:gui/logo" uv="0 0 32 16"/>"#),
            "s",
            Some("mymod"),
        )
        .unwrap();
        assert!(
            code.contains(
                r#"widget::image("mymod:textures/gui/logo.png").region([0.0, 0.0, 32.0, 16.0])"#
            ),
            "{code}"
        );
        assert!(generate_for(&screen(r#"<Image source="mod:gui/logo"/>"#), "s", None).is_err());
        assert!(generate_for(&screen(r#"<Image source="sprite:nope"/>"#), "s", None).is_err());
        assert!(generate_for(&screen(r#"<Image/>"#), "s", None).is_err());
        assert!(generate_for(
            &screen(r#"<Image source="item:apple" stretch="big"/>"#),
            "s",
            None
        )
        .is_err());
    }

    #[test]
    fn raw_texture_paths_are_errors_naming_the_alias() {
        let err = generate_for(
            &screen(r#"<Image source="minecraft:textures/item/apple.png"/>"#),
            "s",
            None,
        )
        .unwrap_err();
        assert!(err.contains(r#"source="item:apple""#), "{err}");
        assert!(
            generate_for(&screen(r#"<Image texture="item:apple"/>"#), "s", None)
                .unwrap_err()
                .contains("no attribute `texture`")
        );
    }

    const SAMPLE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<!-- a comment -->
<Screen id="mymod:main" width="176" height="166" bg="#C0101010" padding="8">
  <Label id="title" x="8" y="6" text="Main &amp; Menu" color="#FFFFFF" no_wrap="true"/>
  <Button x="38" y="40" w="100" h="20" text="Play" bg="#FF555555" on_click="mymod:play"/>
  <Row x="8" y="70" w="50%" gap="2" align="center">
    <InvSlot item="minecraft:diamond" count="3"/>
    <Spacer flex="1"/>
  </Row>
  <ScrollView id="list" x="0" y="100" w="80" h="40">
    <Label text="A long line" ellipsis="true"/>
  </ScrollView>
</Screen>
"##;

    #[test]
    fn parses_the_subset() {
        let root = parse_xml(SAMPLE).unwrap();
        assert_eq!(root.name, "Screen");
        assert_eq!(root.children.len(), 4);
        assert_eq!(root.children[0].attr("text"), Some("Main & Menu"));
        assert_eq!(root.children[2].children[0].attr("count"), Some("3"));
        assert_eq!(root.children[1].line, 5);
    }

    #[test]
    fn generates_builder_code() {
        let code = generate(SAMPLE, "main").unwrap();
        assert!(code.contains(r#"pub const ID: &str = "mymod:main";"#));
        assert!(code.contains(r#"widget::label("Main & Menu")"#));
        assert!(code.contains(".at(8.0, 6.0)"));
        assert!(code.contains(".color(0xFFFFFFFF)"));
        assert!(code.contains(".bg(0xC0101010)"));
        assert!(code.contains(".no_wrap()"));
        assert!(code.contains(".w(Units::Percent(50.0))"));
        assert!(code.contains("widget::inv_slot(\"minecraft:diamond\", 3)"));
        assert!(code.contains(r#"pub const MYMOD_PLAY: &str = "mymod:play";"#));
        assert!(code.contains(r#"pub const TITLE: &str = "title";"#));
        assert!(code.contains(".padding(8.0, 8.0, 8.0, 8.0)"));
        assert!(code.contains("widget::scroll_view()"));
        assert!(code.contains(".ellipsis()"));
        assert!(code.contains("pub fn with_last<R>"));
        assert!(code.starts_with("// <auto-generated>"));
        assert!(code.contains(r#"yog_api::ui_include!("main");"#));
        assert!(code.contains("pub fn close()"));
        // Children of a Row aren't positioned unless they say so.
        assert!(!code.contains("widget::spacer()\n                .at("));
    }

    #[test]
    fn reports_errors_with_lines() {
        let bad = "<Screen id=\"a:b\">\n  <Label txt=\"x\"/>\n</Screen>";
        let err = generate(bad, "x.yui").unwrap_err();
        assert!(err.contains("line 2") && err.contains("txt"), "{err}");
        assert!(generate("<Screen id=\"a:b\"><Label/></Scren>", "x.yui").is_err());
        assert!(generate("<Column/>", "x.yui").is_err());
        assert!(generate("<Screen/>", "x.yui").unwrap_err().contains("id"));
        let bad_color = "<Screen id=\"a:b\" bg=\"red\"/>";
        assert!(generate(bad_color, "x.yui")
            .unwrap_err()
            .contains("#RRGGBB"));
    }

    /// What the IDE's Format Document writes (Visual Studio's XAML layout):
    /// one attribute per line, comments among the children, ` />`.
    #[test]
    fn parses_formatted_markup() {
        let src = r##"<?xml version="1.0" encoding="utf-8"?>
<Screen id="mymod:main"
        width="176"
        height="166"
        bg="#C0101010">
    <!-- the title -->
    <Label id="title"
           x="8"
           y="6"
           text="Main"
           no_wrap="true" />

    <Row x="8" y="70" w="160">
        <!--
          a multi-line comment
        -->
        <Spacer flex="1" />
    </Row>
</Screen>
"##;
        let root = parse_xml(src).unwrap();
        assert_eq!(root.attr("height"), Some("166"));
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].attr("no_wrap"), Some("true"));
        assert_eq!(root.children[0].line, 7);
        assert_eq!(root.children[1].children.len(), 1);
        let code = generate(src, "main.yui").unwrap();
        assert!(code.contains(".at(8.0, 6.0)"));
    }

    #[test]
    fn text_box_and_font() {
        let src = r#"<Screen id="a:b">
  <TextBox id="search" w="100%" placeholder="Search..." max_length="64" on_change="search" on_submit="go" font="minecraft:uniform" shadow="false"/>
</Screen>"#;
        let code = generate(src, "x").unwrap();
        assert!(code.contains(r#"widget::text_box("")"#), "{code}");
        assert!(code.contains(r#".placeholder("Search...")"#));
        assert!(code.contains(".max_length(64)"));
        assert!(code.contains(r#".on_change("search")"#));
        assert!(code.contains(r#".on_submit("go")"#));
        assert!(code.contains(r#".font("minecraft:uniform")"#));
        assert!(code.contains(".shadow(false)"));
        assert!(code.contains(r#"pub const GO: &str = "go";"#));
        assert!(code.contains("pub fn text_box("));
        let bad = r#"<Screen id="a:b"><TextBox max_length="x"/></Screen>"#;
        assert!(generate(bad, "x").unwrap_err().contains("whole number"));
    }

    #[test]
    fn generated_paths() {
        let dir = std::env::temp_dir().join(format!("yog-yui-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src/screens")).unwrap();
        std::fs::write(dir.join("yog.toml"), "").unwrap();
        let yui = dir.join("src/screens/opts.yui");
        std::fs::write(&yui, r#"<Screen id="a:b"/>"#).unwrap();
        assert_eq!(project_root(&yui).as_deref(), Some(dir.as_path()));
        assert_eq!(include_name(&dir, &yui), "screens/opts");
        assert_eq!(generate_all(&dir).unwrap(), 1);
        let generated = dir.join(".yog-build/generated/screens/opts.g.rs");
        assert!(generated.is_file());
        std::fs::remove_file(&yui).unwrap();
        generate_all(&dir).unwrap();
        assert!(!generated.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn const_names() {
        assert_eq!(const_name("mymod:btn-play"), "MYMOD_BTN_PLAY");
        assert_eq!(const_name("openMenu"), "OPEN_MENU");
        assert_eq!(const_name("1st"), "_1ST");
    }

    #[test]
    fn text_starting_with_at_is_a_translation_key() {
        assert_eq!(text_expr("@mymod.title"), "::yog_api::tr(\"mymod.title\")");
        assert_eq!(text_expr("@@home"), "\"@home\"");
        assert_eq!(text_expr("plain"), "\"plain\"");
        assert_eq!(text_expr("@"), "\"@\"");
        let mut refs = crate::yres::Refs::default();
        refs.texts.insert(
            "Strings.title".into(),
            ("mymod.title".into(), "Title".into()),
        );
        set_resource_refs(&refs);
        assert_eq!(
            text_expr("@Strings.title"),
            "::yog_api::tr(\"mymod.title\")"
        );
        set_resource_refs(&crate::yres::Refs::default());
        let code = generate(
            r#"<Screen id="m:s" width="10" height="10"><Label text="@m.title"/><Button text="@@x"/></Screen>"#,
            "s.yui",
        )
        .unwrap();
        assert!(
            code.contains("widget::label(::yog_api::tr(\"m.title\"))"),
            "{code}"
        );
        assert!(code.contains("widget::button(\"@x\")"), "{code}");
    }

    #[test]
    fn color_attributes_resolve_yres_colors() {
        let mut refs = crate::yres::Refs::default();
        refs.colors.insert("Colors.text".into(), 0xFF12_3456);
        set_resource_refs(&refs);
        let code = generate(
            r#"<Screen id="m:s" width="10" height="10"><Label text="x" color="@Colors.text"/></Screen>"#,
            "s.yui",
        );
        let missing = generate(
            r#"<Screen id="m:s" width="10" height="10"><Label text="x" color="@Colors.nope"/></Screen>"#,
            "s.yui",
        );
        set_resource_refs(&crate::yres::Refs::default());
        let code = code.unwrap();
        assert!(code.contains("0xFF123456"), "{code}");
        assert!(missing.unwrap_err().contains("no color `Colors.nope`"));
    }
}
