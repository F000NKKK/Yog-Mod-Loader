//! What a bound attribute sets: the type its value converts to, and the
//! code applying a new value to the widget.

use yog_ids::yui as y;
use yog_ids::yui_attr as at;

use super::{NEW_VALUE, WIDGET};

/// What a bound attribute converts to and how it's applied.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Target {
    /// A text box's text: the view's text box `n`'s.
    TextBox,
    Text,
    Placeholder,
    Tooltip,
    Item,
    Source,
    Count,
    Color(&'static str),
    Enabled,
    Visible,
    Checked,
    Size(&'static str),
    Number(&'static str),
    ScrollKey,
}

/// What attribute `attr` of a `kind` element sets when bound; none when it
/// can't be bound.
pub(super) fn target(kind: &str, attr: &str) -> Option<Target> {
    Some(match attr {
        at::TEXT if kind == y::TEXT_BOX => Target::TextBox,
        at::TEXT if matches!(kind, y::LABEL | y::BUTTON) => Target::Text,
        at::PLACEHOLDER if kind == y::TEXT_BOX => Target::Placeholder,
        at::TOOLTIP => Target::Tooltip,
        at::ITEM if matches!(kind, y::ITEM_SLOT | y::INV_SLOT) => Target::Item,
        at::SOURCE if matches!(kind, y::IMAGE | y::SURFACE) => Target::Source,
        at::COUNT if kind == y::INV_SLOT => Target::Count,
        at::BG => Target::Color(at::BG),
        at::COLOR => Target::Color(at::COLOR),
        at::FOCUS_COLOR => Target::Color(at::FOCUS_COLOR),
        at::ENABLED => Target::Enabled,
        at::VISIBLE => Target::Visible,
        at::CHECKED if kind == y::BUTTON => Target::Checked,
        at::W => Target::Size(at::W),
        at::H => Target::Size(at::H),
        at::GAP => Target::Number(at::GAP),
        at::FONT_SCALE => Target::Number(at::FONT_SCALE),
        at::FLEX => Target::Number(at::FLEX),
        at::MIN_W => Target::Number(at::MIN_W),
        at::MIN_H => Target::Number(at::MIN_H),
        at::SCROLL_KEY if kind == y::SCROLL_VIEW => Target::ScrollKey,
        _ => return None,
    })
}

/// What a value is written as in a conversion: `text(&(E))`.
pub(super) const VALUE_MARK: char = 'E';

impl Target {
    /// The type the bound value converts to, and the conversion (its
    /// value written [`VALUE_MARK`]).
    pub(super) fn conversion(self) -> (&'static str, &'static str) {
        match self {
            Target::TextBox
            | Target::Text
            | Target::Placeholder
            | Target::Tooltip
            | Target::Item
            | Target::Source => ("String", "::yog_kit::bind::text(&(E))"),
            Target::Count => ("u32", "::yog_kit::bind::count(E)"),
            Target::Color(_) => ("::yog_api::ui::Color", "::yog_kit::bind::color(E)"),
            Target::Enabled | Target::Visible | Target::Checked => {
                ("bool", "::yog_kit::bind::flag(E)")
            }
            Target::Size(_) => ("::yog_api::ui::Units", "::yog_kit::bind::units(E)"),
            Target::Number(_) => ("f32", "::yog_kit::bind::number(E)"),
            Target::ScrollKey => ("u64", "::yog_kit::bind::key(&(E))"),
        }
    }

    /// The code applying a new value ([`NEW_VALUE`]) to the widget
    /// ([`WIDGET`]) — to text box `text_box` or scroller `scroller` of the
    /// view, which a template has none of; `Err` names what's missing.
    pub(super) fn apply(
        self,
        text_box: Option<u16>,
        scroller: Option<u16>,
    ) -> Result<String, &'static str> {
        let (w, v) = (WIDGET, NEW_VALUE);
        Ok(match self {
            Target::TextBox => format!(
                "u.set_text_box({}, {v});",
                text_box.ok_or("text boxes in item templates aren't supported")?
            ),
            Target::Text => format!("{w}.set_text(::std::clone::Clone::clone({v})); u.relayout();"),
            Target::Placeholder => {
                format!("::yog_kit::bind::set_placeholder({w}, {v}); u.repaint();")
            }
            Target::Tooltip => format!("{w}.set_tooltip(::std::clone::Clone::clone({v}));"),
            Target::Item => format!("::yog_kit::bind::set_item({w}, {v}); u.repaint();"),
            Target::Source => format!("::yog_kit::bind::set_source({w}, {v}); u.relayout();"),
            Target::Count => format!("::yog_kit::bind::set_count({w}, *{v}); u.repaint();"),
            Target::Color(c) => format!("{w}.style.{c} = *{v}; u.repaint();"),
            Target::Enabled => format!("{w}.enabled = *{v}; u.repaint();"),
            Target::Visible => format!("{w}.visible = *{v}; u.relayout();"),
            Target::Checked => format!("::yog_kit::bind::set_checked({w}, *{v}); u.repaint();"),
            Target::Size(a) | Target::Number(a) => format!("{w}.style.{a} = *{v}; u.relayout();"),
            Target::ScrollKey => format!(
                "u.reset_scroll({});",
                scroller.ok_or("scroll_key needs a ScrollView outside item templates")?
            ),
        })
    }
}
