//! A common attribute (read by `yog_ui::attr`) as the builder call that sets it.

use yog_ui::attr::Attr;
use yog_ui::widget::Units;

fn f(v: f32) -> String {
    format!("{v:?}")
}

/// `Units::Px(10.0)`, `Units::Percent(50.0)`; none for auto.
fn units(u: Units) -> Option<String> {
    match u {
        Units::Auto => None,
        Units::Px(v) => Some(format!("Units::Px({})", f(v))),
        Units::Percent(v) => Some(format!("Units::Percent({})", f(v))),
    }
}

fn sides(name: &str, [t, r, b, l]: [f32; 4]) -> String {
    format!("{name}({}, {}, {}, {})", f(t), f(r), f(b), f(l))
}

fn color(name: &str, c: yog_gfx::Color) -> String {
    format!("{name}(::yog_api::ui::Color(0x{:08X}))", c.0)
}

/// The call setting `attr`; none when it sets nothing (`auto`, `no_wrap="false"`).
pub(crate) fn call(attr: &Attr) -> Option<String> {
    Some(match attr {
        Attr::W(u) => format!("w({})", units(*u)?),
        Attr::H(u) => format!("h({})", units(*u)?),
        Attr::MinW(n) => format!("min_w({})", f(*n)),
        Attr::MinH(n) => format!("min_h({})", f(*n)),
        Attr::Flex(n) => format!("flex({})", f(*n)),
        Attr::Gap(n) => format!("gap({})", f(*n)),
        Attr::FontScale(n) => format!("font_scale({})", f(*n)),
        Attr::Dock(d) => format!("dock(Dock::{d:?})"),
        Attr::Align(a) => format!("align(Align::{a:?})"),
        Attr::FocusStyle(s) => format!("focus_style(FocusStyle::{s:?})"),
        Attr::HAlign(a) => format!("halign(::yog_api::ui::HAlign::{a:?})"),
        Attr::VAlign(a) => format!("valign(::yog_api::ui::VAlign::{a:?})"),
        Attr::Padding(p) => sides("padding", *p),
        Attr::Margin(m) => sides("margin", *m),
        Attr::Bg(c) => color("bg", *c),
        Attr::BgImage(src) => format!("bg_image({src:?})"),
        Attr::Color(c) => color("color", *c),
        Attr::FocusColor(c) => color("focus_color", *c),
        Attr::Shadow(on) => format!("shadow({on})"),
        Attr::Enabled(on) => format!("enabled({on})"),
        Attr::NoWrap(on) => on.then(|| "no_wrap()".to_string())?,
        Attr::Ellipsis(on) => on.then(|| "ellipsis()".to_string())?,
        Attr::MaxLength(n) => format!("max_length({n})"),
    })
}
