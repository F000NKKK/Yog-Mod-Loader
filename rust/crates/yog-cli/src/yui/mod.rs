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

use yog_data::Diagnostic;

mod anchor;
mod attr_code;
mod elements;
mod files;
mod screen;
mod values;
mod widget;

pub use files::{
    GENERATED_DIR, Generated, find_yui_files, generate_all, generate_file, generated_path,
    include_name, is_book_page_file, project_root,
};
pub use screen::{generate_for, generate_with};
pub use values::{HEX_MARK, color, const_name, pascal, set_resource_refs, text_expr};
pub use yog_formats::yui::{Element, parse as parse_xml};

pub(crate) use anchor::Anchor;
pub(crate) use elements::{is_builtin, is_common_attr, is_custom_component};
pub(crate) use screen::{screen_frame, screen_theme};
pub(crate) use values::{
    Names, boolean, color_attr, image_source, is_image_value, number, snake, str_consts, text_attr,
};
pub(crate) use widget::static_widget;

#[cfg(test)]
pub use screen::generate;

/// The import generated widget-building code starts with.
pub(crate) const UI_USE: &str =
    "use ::yog_api::ui::{widget, Align, Dock, FlexDir, FocusStyle, Units};";

/// `line N: <Kind> msg`.
pub(crate) fn err<T>(el: &Element, msg: impl std::fmt::Display) -> Result<T, Diagnostic> {
    Err(Diagnostic::from(format!("<{}> {msg}", el.name)).at_line(el.line))
}

/// Prefix of a custom component property's value that is a Rust
/// expression (a `String`) rather than text — set by the component
/// generator for bound properties; never in a `.yui`.
pub(crate) const RAW_VALUE: &str = "\u{1}rs:";

/// Attribute-name prefix of a raw builder call (`__rs0="action(...)"`)
/// that the view-model generator attaches to elements; never in a `.yui`.
pub(crate) const RAW_CALL: &str = "__rs";

#[cfg(test)]
mod tests;
