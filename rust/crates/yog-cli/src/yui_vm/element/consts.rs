//! The states a widget's style can differ in (`.yui` `Style` triggers).

pub(super) const HOVER: &str = "hover";
pub(super) const PRESSED: &str = "pressed";
pub(super) const FOCUSED: &str = "focused";
pub(super) const SELECTED: &str = yog_ids::yui_attr::SELECTED;
pub(super) const CHECKED: &str = yog_ids::yui_attr::CHECKED;
pub(super) const DISABLED: &str = "disabled";
