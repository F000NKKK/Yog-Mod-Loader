//! A screen's `<Screen.Resources>`: values by key (`<Color x:Key="Accent"
//! value="#FFD700"/>`), and styles (`<Style x:Key based_on>`: their
//! attributes over their base's, `@Name` values put in) — read once for
//! the build and the designer alike.

use std::collections::HashMap;

use yog_ids::sep::TYPED;
use yog_ids::{yui, yui_attr};

/// One element of the resources: its kind, line and attributes.
pub struct Entry<'a> {
    pub kind: &'a str,
    pub line: usize,
    pub attrs: Vec<(&'a str, &'a str)>,
}

impl Entry<'_> {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
}

/// A screen's values and styles; the problems reading them (line, message).
#[derive(Default)]
pub struct Resources {
    pub values: HashMap<String, String>,
    pub styles: HashMap<String, Vec<(String, String)>>,
    pub problems: Vec<(usize, String)>,
}

/// The resource `v` names (`@Accent` → `Accent`); none for anything else
/// (`@@x`, `@Set.key`, words).
pub fn local_name(v: &str) -> Option<&str> {
    v.strip_prefix(TYPED).filter(|n| !n.is_empty() && !n.starts_with(TYPED) && !n.contains('.'))
}

/// A style's own attributes that aren't for its elements.
const STYLE_ONLY: [&str; 3] = [yui_attr::X_KEY, yui_attr::TARGET, yui_attr::BASED_ON];

impl Resources {
    /// The resources of `entries`: values first, then styles, their
    /// `based_on` chains in any order.
    pub fn read(entries: &[Entry]) -> Resources {
        let mut out = Resources::default();
        let mut pending: Vec<&Entry> = Vec::new();
        for r in entries {
            let Some(key) = r.attr(yui_attr::X_KEY) else {
                out.problems.push((r.line, format!("<{}> needs an x:Key", r.kind)));
                continue;
            };
            if r.kind == yui::STYLE {
                pending.push(r);
            } else if let Some(v) = r.attr(yui_attr::VALUE) {
                out.values.insert(key.to_string(), v.to_string());
            } else {
                out.problems.push((r.line, format!("<{}> needs a value (e.g. <Color x:Key=\"Accent\" value=\"#FFD700\"/>)", r.kind)));
            }
        }
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|s| !out.style(s));
            if pending.len() == before {
                out.problems.push((pending[0].line, "<Style> based_on names an unknown (or circular) style".into()));
                break;
            }
        }
        out
    }

    /// Style `s` read when its base is (true); its attributes over its base's.
    fn style(&mut self, s: &Entry) -> bool {
        let mut attrs = match s.attr(yui_attr::BASED_ON).map(|b| b.trim().trim_start_matches(TYPED)) {
            Some(b) => match self.styles.get(b) {
                Some(a) => a.clone(),
                None => return false,
            },
            None => Vec::new(),
        };
        for (k, v) in s.attrs.iter().filter(|(k, _)| !STYLE_ONLY.contains(k)) {
            let v = match local_name(v) {
                Some(n) => match self.values.get(n) {
                    Some(r) => r.clone(),
                    None => {
                        self.problems.push((s.line, format!("<Style> {k}=\"{v}\": no resource `{n}`")));
                        v.to_string()
                    }
                },
                None => v.to_string(),
            };
            attrs.retain(|(ek, _)| ek != k);
            attrs.push((k.to_string(), v));
        }
        let key = s.attr(yui_attr::X_KEY).unwrap_or_default().to_string();
        self.styles.insert(key, attrs);
        true
    }
}
