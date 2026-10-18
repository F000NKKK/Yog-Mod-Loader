//! Custom components — a mod's own reusable widgets, used from `.yui`
//! markup like the built-in elements (the user controls of WPF).
//!
//! A component is a `.yui` file with a `<Component>` root: its properties
//! and events declared in markup, its body bound to them with
//! `{x:Bind ...}`, and an optional code-behind `.rs` for methods the
//! bindings call (see Yog-IDLE's `docs/yui.md`, "Components"):
//!
//! ```xml
//! <Component name="StatBar" description="A labelled progress bar." icon="item:experience_bottle"
//!            design_w="120" design_h="10">
//!     <Property name="label" type="text" default="HP"/>
//!     <Property name="value" type="number" default="0.5" doc="Filled fraction, 0..1" category="Data"/>
//!     <Row w="100%" gap="4">
//!         <Label text="{x:Bind label}"/>
//!         <Spacer w="{x:Bind bar_w()}" h="6" bg="#FF55FF55"/>
//!     </Row>
//! </Component>
//! ```
//!
//! `yog build` generates the [`Component`] implementation from it (a struct
//! with one typed field per property, `NAME`, the designer metadata, `PROPS`,
//! `build`) and
//! registers it (`yog_api::ui_component!`) — this module is that code's
//! runtime: the registry, [`Props`] (the instance's attribute values, read
//! back typed), [`bind`] (binding conversions) and [`designer`], Yog-IDLE's
//! door into a mod built for the host machine.
//!
//! A screen uses a component like any element:
//! `<StatBar x="8" y="8" w="120" label="Mana" value="0.3"/>` becomes
//! `component::build("StatBar", &[("label", "Mana"), ("value", "0.3")])`
//! followed by the builder calls for the common attributes (`x`, `y`, `w`,
//! `h`, `bg`, `on_click`, ...).

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use yog_gfx::Color;

use crate::FlexDir;
use crate::widget::{self, Widget};

/// A property's type: how `.yui` writes it and how the designer edits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    /// Free text (`@key` is a translation key, as for built-in `text`).
    Text,
    /// A number (`1.5`).
    Number,
    /// A whole number (`3`).
    Int,
    /// `true` / `false`.
    Bool,
    /// `#RRGGBB`, `#AARRGGBB` or `0xAARRGGBB`.
    Color,
    /// One of the listed words.
    Choice(&'static [&'static str]),
    /// An action name (like `on_click`): the component hands it to a
    /// widget's `on_click`, and the screen's code-behind gets the event.
    Event,
    /// A Yog image source (`item:apple`, `mymod:gui/logo`, ...).
    Image,
}

impl PropKind {
    /// The designer's name for the kind.
    pub fn as_str(self) -> &'static str {
        match self {
            PropKind::Text => "text",
            PropKind::Number => "number",
            PropKind::Int => "int",
            PropKind::Bool => "bool",
            PropKind::Color => "color",
            PropKind::Choice(_) => "choice",
            PropKind::Event => "event",
            PropKind::Image => "image",
        }
    }
}

/// One property of a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prop {
    pub name: &'static str,
    pub kind: PropKind,
    /// The value when the element doesn't set it, as `.yui` would write it.
    pub default: &'static str,
    /// Shown in the designer's property grid.
    pub doc: &'static str,
    /// The property grid's category for it (empty: the component's own).
    pub category: &'static str,
}

impl Prop {
    pub const fn new(name: &'static str, kind: PropKind, default: &'static str) -> Self {
        Self {
            name,
            kind,
            default,
            doc: "",
            category: "",
        }
    }
    pub const fn text(name: &'static str, default: &'static str) -> Self {
        Self::new(name, PropKind::Text, default)
    }
    pub const fn number(name: &'static str, default: &'static str) -> Self {
        Self::new(name, PropKind::Number, default)
    }
    pub const fn int(name: &'static str, default: &'static str) -> Self {
        Self::new(name, PropKind::Int, default)
    }
    pub const fn bool(name: &'static str, default: bool) -> Self {
        Self::new(name, PropKind::Bool, if default { "true" } else { "false" })
    }
    pub const fn color(name: &'static str, default: &'static str) -> Self {
        Self::new(name, PropKind::Color, default)
    }
    pub const fn choice(
        name: &'static str,
        options: &'static [&'static str],
        default: &'static str,
    ) -> Self {
        Self::new(name, PropKind::Choice(options), default)
    }
    pub const fn event(name: &'static str) -> Self {
        Self::new(name, PropKind::Event, "")
    }
    pub const fn image(name: &'static str, default: &'static str) -> Self {
        Self::new(name, PropKind::Image, default)
    }
    /// Adds the property grid's description.
    pub const fn doc(self, doc: &'static str) -> Self {
        Self { doc, ..self }
    }
    /// Puts it in a property grid category of its own.
    pub const fn category(self, category: &'static str) -> Self {
        Self { category, ..self }
    }
}

/// A custom component — implemented by the code `yog build` generates
/// from a `<Component>` `.yui` (see the [module docs](self)).
pub trait Component {
    /// The `.yui` element name (`<StatBar …/>`), PascalCase.
    const NAME: &'static str;
    /// Its description: the designer's toolbox tooltip and property grid
    /// header. Texts here and below may be `@Set.key` resource references.
    const DOC: &'static str = "";
    /// The designer's toolbox label (empty: `NAME`).
    const DISPLAY_NAME: &'static str = "";
    /// The toolbox icon, a Yog image source (`item:…`, `sprite:…`, `mod:…`)
    /// or `@Icons.x`; empty: a generic glyph.
    const ICON: &'static str = "";
    /// The toolbox category (empty: "<mod> Components").
    const CATEGORY: &'static str = "";
    /// Its size on the designer's canvas and when dropped (0: unset).
    const DESIGN_SIZE: (f32, f32) = (0.0, 0.0);
    /// Whether the toolbox offers it (false: internal to the mod's own markup).
    const TOOLBOX: bool = true;
    const PROPS: &'static [Prop] = &[];
    /// The widget tree for these properties. Common attributes (position,
    /// size, `bg`, `on_click`, ...) are applied to the result by the caller.
    fn build(props: &Props) -> Widget;
}

/// A registered component (see [`register`]).
#[derive(Debug, Clone, Copy)]
pub struct ComponentSpec {
    pub name: &'static str,
    pub doc: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub design_size: (f32, f32),
    pub toolbox: bool,
    pub props: &'static [Prop],
    pub build: fn(&Props) -> Widget,
}

impl ComponentSpec {
    pub fn of<C: Component>() -> Self {
        Self {
            name: C::NAME,
            doc: C::DOC,
            display_name: C::DISPLAY_NAME,
            icon: C::ICON,
            category: C::CATEGORY,
            design_size: C::DESIGN_SIZE,
            toolbox: C::TOOLBOX,
            props: C::PROPS,
            build: C::build,
        }
    }
}

static REGISTRY: Mutex<Vec<ComponentSpec>> = Mutex::new(Vec::new());

/// Adds `C` to this library's components (a later registration of the same
/// name replaces the earlier one). `yog_api::ui_component!` calls this when
/// the mod's library loads.
pub fn register<C: Component>() {
    register_spec(ComponentSpec::of::<C>());
}

pub fn register_spec(spec: ComponentSpec) {
    let mut all = REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
    all.retain(|s| s.name != spec.name);
    all.push(spec);
}

/// Every registered component, in registration order.
pub fn components() -> Vec<ComponentSpec> {
    REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// The registered component `name`.
pub fn find(name: &str) -> Option<ComponentSpec> {
    REGISTRY
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|s| s.name == name)
        .copied()
}

/// The component `name` built from `.yui` attribute values (what generated
/// screen code calls). An unknown name gives a red error box rather than a
/// panic, so a screen with a typo still opens.
pub fn build(name: &str, attrs: &[(&str, &str)]) -> Widget {
    match find(name) {
        Some(spec) => (spec.build)(&Props::new(spec.props, attrs)),
        None => error_box(&format!("unknown component <{name}>")),
    }
}

/// What [`build`] shows for a component it can't build.
pub fn error_box(message: &str) -> Widget {
    widget::panel(FlexDir::Column)
        .bg(Color(0xC0_400000))
        .padding(2.0, 2.0, 2.0, 2.0)
        .child(widget::label(message).color(Color(0xFF_FF8080)))
        .min_w(16.0)
        .min_h(12.0)
}

/// A color value that is missing or malformed: opaque white.
const FALLBACK_COLOR: Color = Color::WHITE;

/// A component's property values: what the element set, else the defaults.
#[derive(Debug, Clone, Default)]
pub struct Props {
    values: HashMap<String, String>,
}

impl Props {
    /// `props`' defaults overridden by `attrs`.
    pub fn new(props: &[Prop], attrs: &[(&str, &str)]) -> Self {
        let mut values: HashMap<String, String> = props
            .iter()
            .map(|p| (p.name.to_string(), p.default.to_string()))
            .collect();
        for (k, v) in attrs {
            values.insert((*k).to_string(), (*v).to_string());
        }
        Self { values }
    }

    /// The raw value (`None`: neither set nor declared).
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// A text value (`@@x` is the literal `@x`).
    pub fn text(&self, name: &str) -> String {
        let v = self.get(name).unwrap_or_default();
        match v.strip_prefix("@@") {
            Some(rest) => format!("@{rest}"),
            None => v.to_string(),
        }
    }

    /// A number (0 if missing or not a number).
    pub fn number(&self, name: &str) -> f32 {
        self.get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0.0)
    }

    /// A whole number (0 if missing or not one).
    pub fn int(&self, name: &str) -> i64 {
        self.get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    /// `true` only for `true`.
    pub fn bool(&self, name: &str) -> bool {
        self.get(name).is_some_and(|v| v.trim() == "true")
    }

    /// A color (opaque white if missing or malformed).
    pub fn color(&self, name: &str) -> Color {
        self.get(name)
            .and_then(parse_color)
            .unwrap_or(FALLBACK_COLOR)
    }

    /// A choice / event / image value, trimmed.
    pub fn word(&self, name: &str) -> &str {
        self.get(name).map(str::trim).unwrap_or_default()
    }
}

/// Conversions of `{x:Bind ...}` values to what a bound attribute takes
/// (generated component code calls these).
pub mod bind {
    use std::fmt::Display;

    use yog_gfx::Color;

    use crate::Units;

    /// `text`, `placeholder`, image sources, nested component properties.
    pub fn text<T: Display + ?Sized>(v: &T) -> String {
        v.to_string()
    }

    /// `bg`, `color`, `focus_color`, `tint`: a color, `0xAARRGGBB`, or a `.yui` color string.
    pub fn color<T: BindColor>(v: T) -> Color {
        v.to_color()
    }

    /// `enabled`, `visible`, `shadow`.
    pub fn flag(v: impl Into<bool>) -> bool {
        v.into()
    }

    /// `count` of an `InvSlot`.
    pub fn count<T: BindNumber>(v: T) -> u32 {
        v.to_f32().max(0.0) as u32
    }

    /// `gap`, `flex`, `min_w`, `min_h`, `font_scale`.
    pub fn number<T: BindNumber>(v: T) -> f32 {
        v.to_f32()
    }

    /// `w`, `h`: a number of pixels, [`Units`], or a `.yui` size string (`"50%"`, `"auto"`).
    pub fn units<T: BindUnits>(v: T) -> Units {
        v.to_units()
    }

    pub trait BindColor {
        fn to_color(self) -> Color;
    }
    impl BindColor for Color {
        fn to_color(self) -> Color {
            self
        }
    }
    impl BindColor for &Color {
        fn to_color(self) -> Color {
            *self
        }
    }
    impl BindColor for u32 {
        fn to_color(self) -> Color {
            Color(self)
        }
    }
    impl BindColor for &u32 {
        fn to_color(self) -> Color {
            Color(*self)
        }
    }
    impl BindColor for &str {
        fn to_color(self) -> Color {
            super::parse_color(self).unwrap_or(super::FALLBACK_COLOR)
        }
    }
    impl BindColor for &&str {
        fn to_color(self) -> Color {
            (*self).to_color()
        }
    }
    impl BindColor for &String {
        fn to_color(self) -> Color {
            self.as_str().to_color()
        }
    }
    impl BindColor for String {
        fn to_color(self) -> Color {
            self.as_str().to_color()
        }
    }

    pub trait BindNumber {
        fn to_f32(self) -> f32;
    }
    macro_rules! numbers {
        ($($t:ty),*) => {$(
            impl BindNumber for $t {
                fn to_f32(self) -> f32 {
                    self as f32
                }
            }
            impl BindNumber for &$t {
                fn to_f32(self) -> f32 {
                    *self as f32
                }
            }
        )*};
    }
    numbers!(f32, f64, i32, i64, u32, u64, usize);

    pub trait BindUnits {
        fn to_units(self) -> Units;
    }
    impl<T: BindNumber> BindUnits for T {
        fn to_units(self) -> Units {
            Units::Px(self.to_f32())
        }
    }
    impl BindUnits for Units {
        fn to_units(self) -> Units {
            self
        }
    }
    impl BindUnits for &str {
        fn to_units(self) -> Units {
            let v = self.trim();
            if v.is_empty() || v == "auto" {
                Units::Auto
            } else if let Some(p) = v.strip_suffix('%') {
                Units::Percent(p.trim().parse().unwrap_or(0.0))
            } else {
                Units::Px(v.parse().unwrap_or(0.0))
            }
        }
    }
    impl BindUnits for &Units {
        fn to_units(self) -> Units {
            *self
        }
    }
    impl BindUnits for &&str {
        fn to_units(self) -> Units {
            (*self).to_units()
        }
    }
    impl BindUnits for &String {
        fn to_units(self) -> Units {
            self.as_str().to_units()
        }
    }
}

/// The attributes every `.yui` element takes (`yog build` applies them to
/// a component's widget with builder calls — they never reach its
/// [`Props`]).
pub const COMMON_ATTRS: &[&str] = &[
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
    "font",
    "shadow",
    "no_wrap",
    "ellipsis",
    "focus_style",
    "focus_color",
    "enabled",
    "on_click",
    "visible",
];

/// One common style attribute applied to `w` as generated code would
/// (layout attributes — position, size, dock, flex — are left to the
/// caller; malformed values are ignored).
pub fn apply_style(w: Widget, key: &str, value: &str) -> Widget {
    let v = value.trim();
    let num = || v.parse::<f32>().ok();
    // `set` applied when the value parsed, else `w` as it is.
    fn with<T>(w: Widget, value: Option<T>, set: fn(Widget, T) -> Widget) -> Widget {
        match value {
            Some(v) => set(w, v),
            None => w,
        }
    }
    match key {
        "bg" => with(w, parse_color(v), Widget::bg),
        "color" => with(w, parse_color(v), Widget::color),
        "focus_color" => with(w, parse_color(v), Widget::focus_color),
        "min_w" => with(w, num(), Widget::min_w),
        "min_h" => with(w, num(), Widget::min_h),
        "gap" => with(w, num(), Widget::gap),
        "font_scale" => with(w, num(), Widget::font_scale),
        "font" => w.font(v),
        "shadow" => w.shadow(v == "true"),
        "enabled" => w.enabled(v != "false"),
        "no_wrap" if v == "true" => w.no_wrap(),
        "ellipsis" if v == "true" => w.ellipsis(),
        "align" => w.align(match v {
            "center" => crate::Align::Center,
            "end" => crate::Align::End,
            _ => crate::Align::Start,
        }),
        "padding" => {
            let parts: Option<Vec<f32>> = v
                .split([' ', ','])
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().ok())
                .collect();
            match parts.as_deref() {
                Some([a]) => w.padding(*a, *a, *a, *a),
                Some([vv, hh]) => w.padding(*vv, *hh, *vv, *hh),
                Some([t, r, b, l]) => w.padding(*t, *r, *b, *l),
                _ => w,
            }
        }
        _ => w,
    }
}

/// `#RRGGBB`, `#AARRGGBB` or `0xAARRGGBB` (as `.yui` colors), or a decimal
/// `0xAARRGGBB` value (what a bound `u32` property arrives as).
pub fn parse_color(value: &str) -> Option<Color> {
    Color::parse(value)
}

/// The screen designer's protocol (see the [module docs](self)): one C-ABI
/// entry point, `yog_ui_designer`, taking a text request and answering
/// through a write callback.
///
/// Requests (lines; values escaped with `\\` and `\n`):
///
/// - `describe` → JSON `{"abi":1,"components":[{"name","doc","props":[{"name","kind","default","doc","options"}]}]}`
/// - `render` / `<name>` / `<w> <h> <avail_w> <avail_h>` / `key=value`… —
///   builds the component, sizes it `w × h` (≤ 0: its natural size, within
///   `avail_w × avail_h`), draws it at the origin through the `YogGfxApi`
///   and answers `{"w":…,"h":…}`.
///
/// A failure (unknown component, panic in `build`) answers `false` and an
/// error message.
pub mod designer {
    use std::ffi::c_void;
    use std::fmt::Write as _;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use yog_abi::{YogGfxApi, YogStr};
    use yog_gfx::GfxContext;

    use super::{COMMON_ATTRS, PropKind, Props, apply_style, components, find};
    use crate::widget::{self, Widget};
    use crate::{FlexDir, UiRoot, Units};

    /// Version of this protocol; the designer host refuses others.
    pub const ABI: u32 = 1;

    /// `write(user_data, text)`: the host's sink for the answer.
    pub type WriteFn = unsafe extern "C" fn(user_data: *mut c_void, text: YogStr);

    /// `export_mod!`'s `yog_ui_designer`.
    ///
    /// # Safety
    /// `request` must be valid UTF-8 for the call; `gfx` a valid table (or
    /// null for `describe`); `write` sound to call with `user_data`.
    pub unsafe fn call(
        request: YogStr,
        gfx: *const YogGfxApi,
        user_data: *mut c_void,
        write: WriteFn,
    ) -> bool {
        // SAFETY: the caller's contract.
        let request = unsafe { request.as_str() };
        let (ok, answer) = match catch_unwind(AssertUnwindSafe(|| answer(request, gfx))) {
            Ok(Ok(a)) => (true, a),
            Ok(Err(e)) => (false, e),
            Err(panic) => (false, panic_message(&*panic)),
        };
        // SAFETY: the caller's contract.
        unsafe { write(user_data, YogStr::from_str(&answer)) };
        ok
    }

    fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
        let msg = panic
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        format!("the component panicked: {msg}")
    }

    fn answer(request: &str, gfx: *const YogGfxApi) -> Result<String, String> {
        let mut lines = request.lines();
        match lines.next().map(str::trim) {
            Some("describe") => Ok(describe()),
            Some("render") => {
                if gfx.is_null() {
                    return Err("render: no gfx table".into());
                }
                let name = lines.next().unwrap_or_default().trim().to_string();
                let size: Vec<f32> = lines
                    .next()
                    .unwrap_or_default()
                    .split_whitespace()
                    .filter_map(|n| n.parse().ok())
                    .collect();
                let [w, h, aw, ah] = size[..] else {
                    return Err("render: expected `w h avail_w avail_h`".into());
                };
                let attrs: Vec<(String, String)> = lines
                    .filter_map(|l| l.split_once('='))
                    .map(|(k, v)| (k.trim().to_string(), unescape(v)))
                    .collect();
                // SAFETY: non-null, valid for the call (the caller's contract).
                let ctx = unsafe { GfxContext::from_raw(gfx) };
                render(&ctx, &name, &attrs, [w, h, aw, ah])
            }
            other => Err(format!("unknown designer request {other:?}")),
        }
    }

    fn render(
        ctx: &GfxContext,
        name: &str,
        attrs: &[(String, String)],
        [w, h, aw, ah]: [f32; 4],
    ) -> Result<String, String> {
        let spec = find(name).ok_or_else(|| format!("no component named {name}"))?;
        type Attrs<'a> = Vec<(&'a str, &'a str)>;
        let (common, own): (Attrs, Attrs) = attrs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .partition(|(k, _)| COMMON_ATTRS.contains(k));
        let mut root: Widget = (spec.build)(&Props::new(spec.props, &own));
        for (k, v) in common {
            root = apply_style(root, k, v);
        }
        if w > 0.0 {
            root = root.w(Units::Px(w));
        }
        if h > 0.0 {
            root = root.h(Units::Px(h));
        }
        let (aw, ah) = (aw.max(1.0), ah.max(1.0));
        let screen = widget::panel(FlexDir::Column)
            .w(Units::Px(aw))
            .h(Units::Px(ah))
            .child(root.at(0.0, 0.0));
        // Measured like the game measures: through the host's text/image metrics.
        crate::text::use_host_metrics(ctx);
        crate::image::use_host_images(ctx);
        let mut ui = UiRoot::new("yog:designer", screen);
        ui.layout(aw, ah);
        let rect = ui
            .layout_root
            .children
            .first()
            .map(|n| n.rect)
            .unwrap_or_default();
        ui.render(ctx);
        Ok(format!("{{\"w\":{},\"h\":{}}}", rect.w, rect.h))
    }

    fn describe() -> String {
        let mut out = format!("{{\"abi\":{ABI},\"components\":[");
        for (i, c) in components().iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"name\":{},\"description\":{},\"display_name\":{},\"icon\":{},\"category\":{},\"design_w\":{},\"design_h\":{},\"toolbox\":{},\"props\":[",
                json(c.name),
                json(c.doc),
                json(c.display_name),
                json(c.icon),
                json(c.category),
                c.design_size.0,
                c.design_size.1,
                c.toolbox
            );
            for (j, p) in c.props.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                let options = match p.kind {
                    PropKind::Choice(o) => o.iter().map(|s| json(s)).collect::<Vec<_>>().join(","),
                    _ => String::new(),
                };
                let _ = write!(
                    out,
                    "{{\"name\":{},\"kind\":{},\"default\":{},\"doc\":{},\"category\":{},\"options\":[{options}]}}",
                    json(p.name),
                    json(p.kind.as_str()),
                    json(p.default),
                    json(p.doc),
                    json(p.category)
                );
            }
            out.push_str("]}");
        }
        out.push_str("]}");
        out
    }

    /// A JSON string literal.
    fn json(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    let _ = write!(out, "\\u{:04x}", c as u32);
                }
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    /// A request value as written by [`escape`].
    pub fn unescape(v: &str) -> String {
        let mut out = String::with_capacity(v.len());
        let mut chars = v.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        }
        out
    }

    /// A request value: one line, `\\` and `\n` escaped.
    pub fn escape(v: &str) -> String {
        v.replace('\\', "\\\\").replace('\n', "\\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Badge;
    impl Component for Badge {
        const NAME: &'static str = "TestBadge";
        const ICON: &'static str = "item:apple";
        const DESIGN_SIZE: (f32, f32) = (40.0, 12.0);
        const PROPS: &'static [Prop] = &[
            Prop::text("label", "new"),
            Prop::color("fill", "#FF3060C0").doc("Background"),
            Prop::choice("size", &["small", "large"], "small"),
        ];
        fn build(p: &Props) -> Widget {
            widget::label(p.text("label"))
                .bg(p.color("fill"))
                .padding(1.0, 2.0, 1.0, 2.0)
        }
    }

    #[test]
    fn props_fall_back_to_defaults() {
        let p = Props::new(Badge::PROPS, &[("label", "@@x"), ("n", " 2.5 ")]);
        assert_eq!(p.text("label"), "@x");
        assert_eq!(p.color("fill"), Color(0xFF3060C0));
        assert_eq!(p.word("size"), "small");
        assert_eq!(p.number("n"), 2.5);
        assert_eq!(p.int("missing"), 0);
        assert!(!p.bool("missing"));
    }

    #[test]
    fn builds_registered_components() {
        register::<Badge>();
        let w = build("TestBadge", &[("label", "hi")]);
        assert!(matches!(&w.kind, crate::widget::WidgetKind::Label(t) if t == "hi"));
        assert_eq!(w.style.bg, Color(0xFF3060C0));
        let missing = build("Nope", &[]);
        assert!(!missing.children.is_empty());
    }

    #[test]
    fn describes_and_escapes() {
        register::<Badge>();
        let mut out = String::new();
        unsafe extern "C" fn sink(ud: *mut std::ffi::c_void, s: yog_abi::YogStr) {
            // SAFETY: `ud` is the `String` below; `s` valid for the call.
            unsafe { (*(ud as *mut String)).push_str(s.as_str()) };
        }
        let req = "describe";
        let ok = unsafe {
            designer::call(
                yog_abi::YogStr::from_str(req),
                std::ptr::null(),
                &mut out as *mut String as *mut _,
                sink,
            )
        };
        assert!(ok, "{out}");
        assert!(out.contains("\"name\":\"TestBadge\""), "{out}");
        assert!(out.contains("\"options\":[\"small\",\"large\"]"), "{out}");
        assert!(out.contains("\"icon\":\"item:apple\",\"category\":\"\",\"design_w\":40,\"design_h\":12,\"toolbox\":true"), "{out}");
        assert_eq!(designer::unescape(&designer::escape("a\\b\nc")), "a\\b\nc");
    }
}
