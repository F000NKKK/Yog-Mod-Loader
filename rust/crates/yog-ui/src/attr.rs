//! The attributes every `.yui` element takes, read once: [`Attr::parse`]
//! turns `key="value"` into a typed attribute (or says what's wrong with
//! it), [`Attr::apply`] sets it on a widget. The game's runtime, the screen
//! designer and `yog build`'s code generator all read them here.

use yog_gfx::Color;
use yog_ids::yui_attr as a;
use yog_ids::yui_value as w;

use crate::layout::Align;
use crate::widget::{Dock, FocusStyle, HAlign, Units, VAlign, Widget};

/// One attribute, its value read.
#[derive(Debug, Clone, PartialEq)]
pub enum Attr {
    W(Units),
    H(Units),
    MinW(f32),
    MinH(f32),
    Flex(f32),
    Gap(f32),
    FontScale(f32),
    Dock(Dock),
    Align(Align),
    FocusStyle(FocusStyle),
    HAlign(HAlign),
    VAlign(VAlign),
    Padding([f32; 4]),
    Margin([f32; 4]),
    Bg(Color),
    /// An image as the background: `sprite:panel`, `mymod:gui/frame`.
    BgImage(String),
    Color(Color),
    FocusColor(Color),
    Shadow(bool),
    Enabled(bool),
    NoWrap(bool),
    Ellipsis(bool),
    MaxLength(usize),
}

/// What a color written in hex starts with: `#RRGGBB`, `0xAARRGGBB`.
pub const HEX_MARKS: [&str; 3] = ["#", "0x", "0X"];

/// `#RRGGBB`, `#AARRGGBB` or `0xAARRGGBB` → the color (a file's color is
/// written with its `#` or `0x`, never as a decimal).
pub fn written_color(value: &str) -> Option<Color> {
    let v = value.trim();
    HEX_MARKS.iter().any(|p| v.starts_with(p)).then(|| Color::parse(v)).flatten()
}

/// A size: pixels, `50%`, or `auto` (and empty).
pub fn size(value: &str) -> Option<Units> {
    let v = value.trim();
    if v.is_empty() || v == w::AUTO {
        return Some(Units::Auto);
    }
    match v.strip_suffix('%') {
        Some(p) => p.trim().parse().ok().map(Units::Percent),
        None => v.parse().ok().map(Units::Px),
    }
}

/// CSS-style shorthand: 1 value = all sides, 2 = vertical horizontal, 4 =
/// top right bottom left.
pub fn sides(value: &str) -> Option<[f32; 4]> {
    let parts: Vec<f32> = value.split([' ', ',']).filter(|s| !s.is_empty()).map(|s| s.parse().ok()).collect::<Option<_>>()?;
    match parts.as_slice() {
        [all] => Some([*all; 4]),
        [v, h] => Some([*v, *h, *v, *h]),
        [t, r, b, l] => Some([*t, *r, *b, *l]),
        _ => None,
    }
}

/// The word `value` is among `words`, as its value; the words when it isn't one.
fn pick<T: Copy>(value: &str, words: &[(&str, T)]) -> Result<T, String> {
    let v = value.trim();
    words.iter().find(|(k, _)| *k == v).map(|(_, t)| *t).ok_or_else(|| {
        let all: Vec<&str> = words.iter().map(|(k, _)| *k).collect();
        format!("one of {}", all.join(", "))
    })
}

const DOCKS: [(&str, Dock); 6] = [
    (w::NONE, Dock::None),
    (w::FILL, Dock::Fill),
    (w::LEFT, Dock::Left),
    (w::RIGHT, Dock::Right),
    (w::TOP, Dock::Top),
    (w::BOTTOM, Dock::Bottom),
];
const ALIGNS: [(&str, Align); 3] = [(w::START, Align::Start), (w::CENTER, Align::Center), (w::END, Align::End)];
const FOCUS_STYLES: [(&str, FocusStyle); 3] =
    [(w::OUTLINE, FocusStyle::Outline), (w::FILL, FocusStyle::Fill), (w::NONE, FocusStyle::None)];
const HALIGNS: [(&str, HAlign); 4] =
    [(w::LEFT, HAlign::Left), (w::CENTER, HAlign::Center), (w::RIGHT, HAlign::Right), (w::STRETCH, HAlign::Stretch)];
const VALIGNS: [(&str, VAlign); 4] =
    [(w::TOP, VAlign::Top), (w::CENTER, VAlign::Center), (w::BOTTOM, VAlign::Bottom), (w::STRETCH, VAlign::Stretch)];
const BOOLS: [(&str, bool); 2] = [(w::TRUE, true), (w::FALSE, false)];

/// An image-like value: an image source with its scheme or namespace.
fn is_image(v: &str) -> bool {
    v.contains(yog_ids::sep::NAMESPACE)
}

impl Attr {
    /// `key="value"` as an attribute; none when `key` isn't one of these
    /// (an id, an event, a text, a kind's own); what's wrong otherwise.
    pub fn parse(key: &str, value: &str) -> Result<Option<Attr>, String> {
        let num = || value.trim().parse::<f32>().map_err(|_| "a number".to_string());
        let color = || written_color(value).ok_or_else(|| "#RRGGBB or #AARRGGBB".to_string());
        let flag = || pick(value, &BOOLS).map_err(|_| "true or false".to_string());
        Ok(Some(match key {
            a::W => Attr::W(size(value).ok_or("a size: pixels, 50% or auto")?),
            a::H => Attr::H(size(value).ok_or("a size: pixels, 50% or auto")?),
            a::MIN_W => Attr::MinW(num()?),
            a::MIN_H => Attr::MinH(num()?),
            a::FLEX => Attr::Flex(num()?),
            a::GAP => Attr::Gap(num()?),
            a::FONT_SCALE => Attr::FontScale(num()?),
            a::DOCK => Attr::Dock(pick(value, &DOCKS)?),
            a::ALIGN => Attr::Align(pick(value, &ALIGNS)?),
            a::FOCUS_STYLE => Attr::FocusStyle(pick(value, &FOCUS_STYLES)?),
            a::HALIGN => Attr::HAlign(pick(value, &HALIGNS)?),
            a::VALIGN => Attr::VAlign(pick(value, &VALIGNS)?),
            a::PADDING => Attr::Padding(sides(value).ok_or("1, 2 or 4 numbers")?),
            a::MARGIN => Attr::Margin(sides(value).ok_or("1, 2 or 4 numbers")?),
            a::BG if is_image(value) => Attr::BgImage(value.trim().to_string()),
            a::BG => Attr::Bg(color()?),
            a::COLOR => Attr::Color(color()?),
            a::FOCUS_COLOR => Attr::FocusColor(color()?),
            a::SHADOW => Attr::Shadow(flag()?),
            a::ENABLED => Attr::Enabled(flag()?),
            a::NO_WRAP => Attr::NoWrap(flag()?),
            a::ELLIPSIS => Attr::Ellipsis(flag()?),
            a::MAX_LENGTH => Attr::MaxLength(value.trim().parse().map_err(|_| "a whole number".to_string())?),
            _ => return Ok(None),
        }))
    }

    /// Whether it styles the widget — not where it goes or how big it is.
    pub fn is_style(&self) -> bool {
        !matches!(self, Attr::W(_) | Attr::H(_) | Attr::Flex(_) | Attr::Dock(_) | Attr::Margin(_) | Attr::HAlign(_) | Attr::VAlign(_))
    }

    /// The attribute set on `widget`.
    pub fn apply(self, widget: Widget) -> Widget {
        match self {
            Attr::W(Units::Auto) | Attr::H(Units::Auto) => widget,
            Attr::W(u) => widget.w(u),
            Attr::H(u) => widget.h(u),
            Attr::MinW(n) => widget.min_w(n),
            Attr::MinH(n) => widget.min_h(n),
            Attr::Flex(n) => widget.flex(n),
            Attr::Gap(n) => widget.gap(n),
            Attr::FontScale(n) => widget.font_scale(n),
            Attr::Dock(d) => widget.dock(d),
            Attr::Align(al) => widget.align(al),
            Attr::FocusStyle(s) => widget.focus_style(s),
            Attr::HAlign(h) => widget.halign(h),
            Attr::VAlign(v) => widget.valign(v),
            Attr::Padding([t, r, b, l]) => widget.padding(t, r, b, l),
            Attr::Margin([t, r, b, l]) => widget.margin(t, r, b, l),
            Attr::Bg(c) => widget.bg(c),
            Attr::BgImage(src) => widget.bg_image(src),
            Attr::Color(c) => widget.color(c),
            Attr::FocusColor(c) => widget.focus_color(c),
            Attr::Shadow(on) => widget.shadow(on),
            Attr::Enabled(on) => widget.enabled(on),
            Attr::NoWrap(true) => widget.no_wrap(),
            Attr::Ellipsis(true) => widget.ellipsis(),
            Attr::NoWrap(false) | Attr::Ellipsis(false) => widget,
            Attr::MaxLength(n) => widget.max_length(n),
        }
    }
}
