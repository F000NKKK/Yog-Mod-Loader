//! Themes: the look of `.yui` screens, apart from their markup.
//!
//! A screen's markup describes what's on it; without a theme it draws in
//! yog-ui's plain look (flat colors). A theme gives each kind of element
//! (`Button`, `ListView`, `Screen`, …) default attributes — the same
//! attributes `.yui` writes (`bg`, `color`, `hover:bg`, …) — that apply
//! wherever the element doesn't set them itself or through a `style`.
//!
//! A `.yui` picks one with `<Screen theme="minecraft:vanilla">`: a built-in
//! theme ([`Theme::builtin`]) or a mod's `.ytheme` file — JSON shaped like
//! [`VANILLA`]: `{ "Button": { "bg": "sprite:button", ... }, ... }`.

/// One element kind's defaults: `(attribute, value)`.
pub type Rule = (String, Vec<(String, String)>);

/// Element kind → default attributes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Theme {
    rules: Vec<Rule>,
}

/// Vanilla Minecraft's look: the grey window, stone buttons, inset slots
/// for lists, dark-grey labels.
pub const VANILLA: &[(&str, &[(&str, &str)])] = &[
    ("Screen", &[("bg", "sprite:panel")]),
    ("Label", &[("color", "#404040"), ("shadow", "false")]),
    (
        "Button",
        &[
            ("bg", "sprite:button"),
            ("color", "#FFFFFF"),
            ("shadow", "true"),
            ("hover:bg", "sprite:button_hover"),
            ("disabled:bg", "sprite:button_disabled"),
            ("disabled:color", "#A0A0A0"),
        ],
    ),
    (
        "ToggleButton",
        &[
            ("bg", "sprite:button"),
            ("color", "#FFFFFF"),
            ("shadow", "true"),
            ("hover:bg", "sprite:button_hover"),
            ("checked:bg", "sprite:button_disabled"),
            ("checked:color", "#FFFFA0"),
            ("disabled:bg", "sprite:button_disabled"),
            ("disabled:color", "#A0A0A0"),
        ],
    ),
    ("ListView", &[("bg", "sprite:slot")]),
    ("ScrollView", &[("bg", "sprite:slot")]),
];

impl Theme {
    /// A built-in theme by id: `minecraft:vanilla`.
    pub fn builtin(id: &str) -> Option<Theme> {
        match id {
            "minecraft:vanilla" => Some(Theme::from_static(VANILLA)),
            _ => None,
        }
    }

    /// Ids of the built-in themes.
    pub const BUILTIN: &'static [&'static str] = &["minecraft:vanilla"];

    pub fn from_rules(rules: Vec<Rule>) -> Theme {
        Theme { rules }
    }

    fn from_static(rules: &[(&str, &[(&str, &str)])]) -> Theme {
        Theme {
            rules: rules
                .iter()
                .map(|(k, attrs)| {
                    let attrs = attrs.iter().map(|(a, v)| (a.to_string(), v.to_string())).collect();
                    (k.to_string(), attrs)
                })
                .collect(),
        }
    }

    /// The defaults for element `kind` (its `.yui` name).
    pub fn attrs(&self, kind: &str) -> &[(String, String)] {
        self.rules
            .iter()
            .find(|(k, _)| k == kind)
            .map_or(&[], |(_, a)| a.as_slice())
    }

    /// Adds to `attrs` (an element's own, styles already merged) every
    /// default of `kind` it doesn't set.
    pub fn apply(&self, kind: &str, attrs: &mut Vec<(String, String)>) {
        for (k, v) in self.attrs(kind) {
            if !attrs.iter().any(|(ak, _)| ak == k) {
                attrs.push((k.clone(), v.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_attributes_win() {
        let t = Theme::builtin("minecraft:vanilla").unwrap();
        let mut attrs = vec![("color".to_string(), "#FF0000".to_string())];
        t.apply("Button", &mut attrs);
        assert_eq!(attrs[0].1, "#FF0000");
        assert!(attrs.iter().any(|(k, v)| k == "bg" && v == "sprite:button"));
        assert!(t.attrs("Spacer").is_empty());
        assert!(Theme::builtin("nope:nope").is_none());
    }
}
