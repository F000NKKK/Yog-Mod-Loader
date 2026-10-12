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
//! `yog build` (and `yog ui gen`) turns `src/**/foo.yui` into a sibling
//! `foo.designer.rs` — VS's `Form1.Designer.cs`: generated, git-ignored,
//! meant to be `include!`d from the screen's hand-written code-behind module.
//! See `docs/yui.md` in Yog-IDLE for the full format.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

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
    fn attr(&self, key: &str) -> Option<&str> {
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

const CONTAINERS: &[&str] = &["Screen", "Canvas", "Column", "Row"];
const LEAVES: &[&str] = &[
    "Label", "Button", "ItemSlot", "InvSlot", "Image", "Spacer",
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
    "font_scale",
    "shadow",
    "no_wrap",
    "focus_style",
    "focus_color",
    "enabled",
    "on_click",
];

fn kind_attrs(kind: &str) -> &'static [&'static str] {
    match kind {
        "Screen" => &["width", "height", "anchor", "modal", "pause"],
        "Label" | "Button" => &["text"],
        "ItemSlot" => &["item"],
        "InvSlot" => &["item", "count"],
        "Image" => &["texture", "tex_w", "tex_h"],
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

fn color_attr(el: &Element, key: &str, value: &str) -> Result<u32, String> {
    color(value).ok_or_else(|| {
        format!(
            "line {}: <{}> {key}=\"{value}\": expected #RRGGBB or #AARRGGBB",
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
        return Ok(Some(format!(
            "Units::Percent({})",
            f(number(el, key, p)?)
        )));
    }
    Ok(Some(format!("Units::Px({})", f(number(el, key, v)?))))
}

/// CSS-style shorthand: 1 value = all sides, 2 = vertical horizontal, 4 = top right bottom left.
fn padding(el: &Element, value: &str) -> Result<[f32; 4], String> {
    let parts: Vec<f32> = value
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .map(|s| number(el, "padding", s))
        .collect::<Result<_, _>>()?;
    match parts.as_slice() {
        [a] => Ok([*a; 4]),
        [v, h] => Ok([*v, *h, *v, *h]),
        [t, r, b, l] => Ok([*t, *r, *b, *l]),
        _ => Err(format!(
            "line {}: <{}> padding=\"{value}\": expected 1, 2 or 4 numbers",
            el.line, el.name
        )),
    }
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
}

impl Gen {
    /// The builder expression for `el`. `in_canvas`: the parent positions
    /// every child absolutely (`Screen`/`Canvas`) — a missing x/y means 0.
    fn widget(&mut self, el: &Element, in_canvas: bool, depth: usize) -> Result<String, String> {
        let kind = el.name.as_str();
        let is_container = CONTAINERS.contains(&kind);
        if !is_container && !LEAVES.contains(&kind) {
            return Err(format!(
                "line {}: unknown element <{kind}> (expected one of {}, {})",
                el.line,
                CONTAINERS[1..].join(", "),
                LEAVES.join(", ")
            ));
        }
        if kind == "Screen" && depth > 0 {
            return Err(format!("line {}: <Screen> must be the root element", el.line));
        }
        if !is_container && !el.children.is_empty() {
            return Err(format!("line {}: <{kind}> can't have children", el.line));
        }
        for (key, _) in &el.attrs {
            if !COMMON_ATTRS.contains(&key.as_str()) && !kind_attrs(kind).contains(&key.as_str()) {
                return Err(format!(
                    "line {}: <{kind}> has no attribute `{key}`",
                    el.line
                ));
            }
        }
        let text = || el.attr("text").unwrap_or_default().to_string();
        let mut expr = match kind {
            "Screen" | "Canvas" | "Column" => "widget::panel(FlexDir::Column)".to_string(),
            "Row" => "widget::panel(FlexDir::Row)".to_string(),
            "Label" => format!("widget::label({:?})", text()),
            "Button" => format!("widget::button({:?})", text()),
            "ItemSlot" => format!("widget::item_slot({:?})", el.attr("item").unwrap_or_default()),
            "InvSlot" => {
                let count = match el.attr("count") {
                    Some(c) => c.trim().parse::<u32>().map_err(|_| {
                        format!("line {}: <InvSlot> count=\"{c}\": expected a whole number", el.line)
                    })?,
                    None => 1,
                };
                format!(
                    "widget::inv_slot({:?}, {count})",
                    el.attr("item").unwrap_or_default()
                )
            }
            "Image" => {
                let tw = el.attr("tex_w").map(|v| number(el, "tex_w", v)).transpose()?.unwrap_or(16.0);
                let th = el.attr("tex_h").map(|v| number(el, "tex_h", v)).transpose()?.unwrap_or(16.0);
                format!(
                    "widget::mc_image({:?}, {}, {})",
                    el.attr("texture").unwrap_or_default(),
                    f(tw),
                    f(th)
                )
            }
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
            call(format!("at({}, {})", f(x.unwrap_or(0.0)), f(y.unwrap_or(0.0))));
        }
        for (key, value) in &el.attrs {
            let v = value.as_str();
            match key.as_str() {
                "id" => {
                    if !self.ids.contains(value) {
                        self.ids.push(value.clone());
                    }
                    call(format!("id({v:?})"));
                }
                "on_click" => {
                    if !self.events.contains(value) {
                        self.events.push(value.clone());
                    }
                    call(format!("on_click({v:?})"));
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
                    let d = pick(el, key, v, &["none", "fill", "left", "right", "top", "bottom"])?;
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
                "padding" => {
                    let [t, r, b, l] = padding(el, v)?;
                    call(format!("padding({}, {}, {}, {})", f(t), f(r), f(b), f(l)));
                }
                "bg" | "color" | "focus_color" => {
                    call(format!("{key}(0x{:08X})", color_attr(el, key, v)?));
                }
                "shadow" | "enabled" => call(format!("{key}({})", boolean(el, key, v)?)),
                "no_wrap" if boolean(el, key, v)? => call("no_wrap()".to_string()),
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

/// Generates the `.designer.rs` source for a `.yui` document. `source_name`
/// (the `.yui` file's name) only goes into the header comment.
pub fn generate(src: &str, source_name: &str) -> Result<String, String> {
    let root = parse_xml(src)?;
    if root.name != "Screen" {
        return Err(format!(
            "line {}: the root element must be <Screen>, not <{}>",
            root.line, root.name
        ));
    }
    let id = root
        .attr("id")
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("line {}: <Screen> needs an id (the UI id, e.g. \"mymod:main\")", root.line))?
        .to_string();
    let width = root.attr("width").map(|v| number(&root, "width", v)).transpose()?.unwrap_or(176.0);
    let height = root.attr("height").map(|v| number(&root, "height", v)).transpose()?.unwrap_or(166.0);
    let anchor = pick(
        &root,
        "anchor",
        root.attr("anchor").unwrap_or("center"),
        &[
            "center", "top_left", "top", "top_right", "left", "right", "bottom_left", "bottom",
            "bottom_right", "fill",
        ],
    )?;
    let modal = root.attr("modal").map(|v| boolean(&root, "modal", v)).transpose()?.unwrap_or(true);
    let pause = root.attr("pause").map(|v| boolean(&root, "pause", v)).transpose()?.unwrap_or(false);

    let mut gen = Gen {
        events: Vec::new(),
        ids: Vec::new(),
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
        r#"// Generated by yog from {source_name} — do not edit; edit the .yui file
// (Yog-IDLE's screen designer) instead. Meant to be `include!`d from the
// screen's code-behind module:
//
//     include!("{stem}.designer.rs");

/// The UI id this screen registers under (`register_ui`, `on_ui_render`, `open_ui`).
#[allow(dead_code)]
pub const ID: &str = {id:?};
/// Design-time size in GUI pixels.
#[allow(dead_code)]
pub const WIDTH: f32 = {width};
#[allow(dead_code)]
pub const HEIGHT: f32 = {height};

/// `on_click` events raised by this screen's widgets.
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

/// Opens the screen (modal: {modal}, pauses the game: {pause}).
#[allow(dead_code)]
pub fn open() {{
    ::yog_api::open_ui(ID, {modal}, {pause});
}}
"#,
        stem = source_name.strip_suffix(".yui").unwrap_or(source_name),
        width = f(width),
        height = f(height),
        events = consts(&gen.events),
        ids = consts(&gen.ids),
    );
    Ok(out)
}

/// Where `foo.yui`'s generated code goes: `foo.designer.rs` next to it.
pub fn designer_path(yui: &Path) -> PathBuf {
    yui.with_extension("designer.rs")
}

/// Generates `<file>.designer.rs` for one `.yui` file — only rewritten when
/// the output changed, so cargo doesn't rebuild for nothing.
pub fn generate_file(yui: &Path) -> Result<PathBuf, String> {
    let src = std::fs::read_to_string(yui).map_err(|e| format!("{}: {e}", yui.display()))?;
    let name = yui
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let code = generate(&src, &name).map_err(|e| format!("{}: {e}", yui.display()))?;
    let out = designer_path(yui);
    if std::fs::read_to_string(&out).ok().as_deref() != Some(code.as_str()) {
        std::fs::write(&out, code).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(out)
}

/// Every `.yui` file under `dir`, recursively.
pub fn find_yui_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(find_yui_files(&path));
        } else if path.extension().is_some_and(|e| e == "yui") {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// `yog build`'s step: regenerates every `src/**/*.yui`'s designer file.
pub fn generate_all(root: &Path) -> Result<usize, String> {
    let files = find_yui_files(&root.join("src"));
    for file in &files {
        generate_file(file)?;
    }
    Ok(files.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<!-- a comment -->
<Screen id="mymod:main" width="176" height="166" bg="#C0101010" padding="8">
  <Label id="title" x="8" y="6" text="Main &amp; Menu" color="#FFFFFF" no_wrap="true"/>
  <Button x="38" y="40" w="100" h="20" text="Play" bg="#FF555555" on_click="mymod:play"/>
  <Row x="8" y="70" w="50%" gap="2" align="center">
    <InvSlot item="minecraft:diamond" count="3"/>
    <Spacer flex="1"/>
  </Row>
</Screen>
"##;

    #[test]
    fn parses_the_subset() {
        let root = parse_xml(SAMPLE).unwrap();
        assert_eq!(root.name, "Screen");
        assert_eq!(root.children.len(), 3);
        assert_eq!(root.children[0].attr("text"), Some("Main & Menu"));
        assert_eq!(root.children[2].children[0].attr("count"), Some("3"));
        assert_eq!(root.children[1].line, 5);
    }

    #[test]
    fn generates_builder_code() {
        let code = generate(SAMPLE, "main.yui").unwrap();
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
        assert!(generate(bad_color, "x.yui").unwrap_err().contains("#RRGGBB"));
    }

    #[test]
    fn const_names() {
        assert_eq!(const_name("mymod:btn-play"), "MYMOD_BTN_PLAY");
        assert_eq!(const_name("openMenu"), "OPEN_MENU");
        assert_eq!(const_name("1st"), "_1ST");
    }
}
