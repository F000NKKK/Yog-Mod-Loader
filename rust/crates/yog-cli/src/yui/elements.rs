//! The elements a `.yui` is made of: built-in containers and leaves,
//! the attributes each takes, the mod's own components.

use super::values::{is_resource_text, resource_color, text_attr};
use super::{Element, RAW_VALUE};
use yog_data::Diagnostic;

/// Whether `<kind>` is a built-in element.
pub(crate) fn is_builtin(kind: &str) -> bool {
    CONTAINERS.contains(&kind) || LEAVES.contains(&kind)
}

/// Whether `<kind>` is a mod's own component (not a built-in element).
pub(crate) fn is_custom_component(kind: &str) -> bool {
    !is_builtin(kind) && is_component_name(kind)
}

/// Whether `key` is an attribute every element takes (not a component's
/// own property).
pub(crate) fn is_common_attr(key: &str) -> bool {
    COMMON_ATTRS.contains(&key) || key == yog_ids::yui_attr::VISIBLE
}

/// The built-in elements that hold others, and those that don't.
pub(super) const CONTAINERS: &[&str] = &[
    yog_ids::yui::SCREEN,
    yog_ids::yui::CANVAS,
    yog_ids::yui::COLUMN,
    yog_ids::yui::ROW,
    yog_ids::yui::SCROLL_VIEW,
    yog_ids::yui::GRID,
];
pub(super) const LEAVES: &[&str] = &[
    yog_ids::yui::LABEL,
    yog_ids::yui::BUTTON,
    yog_ids::yui::TEXT_BOX,
    yog_ids::yui::ITEM_SLOT,
    yog_ids::yui::INV_SLOT,
    yog_ids::yui::SLOT,
    yog_ids::yui::PLAYER_INVENTORY,
    yog_ids::yui::IMAGE,
    yog_ids::yui::SPACER,
    yog_ids::yui::SURFACE,
    yog_ids::yui::MACHINE,
];
/// The attributes every element takes.
pub(super) const COMMON_ATTRS: &[&str] = &[
    yog_ids::yui_attr::ID,
    yog_ids::yui_attr::X,
    yog_ids::yui_attr::Y,
    yog_ids::yui_attr::W,
    yog_ids::yui_attr::H,
    yog_ids::yui_attr::MIN_W,
    yog_ids::yui_attr::MIN_H,
    yog_ids::yui_attr::FLEX,
    yog_ids::yui_attr::DOCK,
    yog_ids::yui_attr::GAP,
    yog_ids::yui_attr::PADDING,
    yog_ids::yui_attr::BG,
    yog_ids::yui_attr::COLOR,
    yog_ids::yui_attr::ALIGN,
    yog_ids::yui_attr::HALIGN,
    yog_ids::yui_attr::VALIGN,
    yog_ids::yui_attr::MARGIN,
    yog_ids::yui_attr::FONT_SCALE,
    yog_ids::yui_attr::FONT,
    yog_ids::yui_attr::SHADOW,
    yog_ids::yui_attr::NO_WRAP,
    yog_ids::yui_attr::ELLIPSIS,
    yog_ids::yui_attr::FOCUS_STYLE,
    yog_ids::yui_attr::FOCUS_COLOR,
    yog_ids::yui_attr::ENABLED,
    yog_ids::yui_attr::ON_CLICK,
    yog_ids::yui_attr::TOOLTIP,
];

/// The attributes element `kind` takes besides the common ones.
pub(super) fn kind_attrs(kind: &str) -> &'static [&'static str] {
    match kind {
        yog_ids::yui::SCREEN => &[
            yog_ids::yui_attr::WIDTH,
            yog_ids::yui_attr::HEIGHT,
            yog_ids::yui_attr::ANCHOR,
            yog_ids::yui_attr::MODAL,
            yog_ids::yui_attr::PAUSE,
            yog_ids::yui_attr::LIVE,
            yog_ids::yui_attr::THEME,
        ],
        yog_ids::yui::LABEL | yog_ids::yui::BUTTON => &[yog_ids::yui_attr::TEXT],
        yog_ids::yui::TEXT_BOX => &[
            yog_ids::yui_attr::TEXT,
            yog_ids::yui_attr::PLACEHOLDER,
            yog_ids::yui_attr::MAX_LENGTH,
            yog_ids::yui_attr::ON_CHANGE,
            yog_ids::yui_attr::ON_SUBMIT,
        ],
        yog_ids::yui::ITEM_SLOT => &[yog_ids::yui_attr::ITEM],
        yog_ids::yui::INV_SLOT => &[yog_ids::yui_attr::ITEM, yog_ids::yui_attr::COUNT],
        yog_ids::yui::SLOT => &[yog_ids::yui_attr::INDEX, yog_ids::yui_attr::ACCEPTS],
        yog_ids::yui::IMAGE | yog_ids::yui::VIDEO => &[
            yog_ids::yui_attr::SOURCE,
            yog_ids::yui_attr::STRETCH,
            yog_ids::yui_attr::UV,
            yog_ids::yui_attr::TINT,
        ],
        yog_ids::yui::SURFACE => &[yog_ids::yui_attr::SOURCE],
        yog_ids::yui::MACHINE => &[
            yog_ids::yui_attr::KIND,
            yog_ids::yui_attr::RESULT,
            yog_ids::yui_attr::PLACES,
        ],
        _ => &[],
    }
}

/// `<StatBar …/>`: a custom component's name (not a built-in element).
pub(super) fn is_component_name(kind: &str) -> bool {
    kind.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A custom component built by name from its own (non-common) attributes;
/// `@key` values are translated like `text`.
pub(super) fn component_expr(el: &Element) -> Result<String, Diagnostic> {
    let props: Vec<String> = el
        .attrs
        .iter()
        .filter(|(k, _)| !COMMON_ATTRS.contains(&k.as_str()))
        .map(|(k, v)| {
            if let Some(rust) = v.strip_prefix(RAW_VALUE) {
                return Ok(format!("({k:?}, &*{rust})"));
            }
            match v.strip_prefix('@') {
                // A `.yres` color (for a color property): its value.
                Some(rest) if !is_resource_text(rest) && resource_color(rest).is_some() => {
                    let c = resource_color(rest).unwrap_or_default();
                    Ok(format!("({k:?}, \"#{c:08X}\")"))
                }
                Some(rest) if !rest.trim().is_empty() => {
                    Ok(format!("({k:?}, &*{})", text_attr(el, k, v)?))
                }
                _ => Ok(format!("({k:?}, {v:?})")),
            }
        })
        .collect::<Result<_, Diagnostic>>()?;
    Ok(format!(
        "::yog_api::ui::component::build({:?}, &[{}])",
        el.name,
        props.join(", ")
    ))
}

/// The `.yui` spelling of a WPF/WinUI attribute name, for error hints.
pub(super) fn wpf_hint(key: &str) -> Option<&'static str> {
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
