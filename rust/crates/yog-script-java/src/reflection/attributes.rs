//! The attributes written on a type and its members, in its description:
//! `.Attribute(() -> new Range(0, 100))` — made anew each time asked for,
//! as .NET's. One whose arguments name macros (`[Derive(Clone)]`) has no
//! value to make, so it isn't there.

use yog_script_analyze::ast::Attribute;
use yog_script_ids::jvm::reflection::ATTRIBUTE;

use crate::Gen;
use yog_script_ids::java::NEW;

impl Gen<'_> {
    /// `.Attribute(…)` of each of `attributes` that's made of values.
    pub(super) fn attributes_described(&mut self, attributes: &[Attribute]) -> String {
        let mut out = String::new();
        for a in attributes {
            let Some(name) = a.path.last().map(|n| n.text.as_str()) else {
                continue;
            };
            if !a.args.is_empty() && !self.index.takes_args(name) {
                continue;
            }
            let args: Vec<String> = a
                .args
                .iter()
                .map(|arg| self.constant_value(&arg.value, None))
                .collect();
            out.push_str(&format!(
                ".{ATTRIBUTE}(() -> {NEW} {}({}))",
                self.index.place(name, 0),
                args.join(", ")
            ));
        }
        out
    }

    /// Member line `line` (a statement) with `attributes` on what it describes.
    pub(super) fn with_attributes(&mut self, line: String, attributes: &[Attribute]) -> String {
        let attributes = self.attributes_described(attributes);
        match line.strip_suffix(';') {
            Some(call) if !attributes.is_empty() => format!("{call}{attributes};"),
            _ => line,
        }
    }
}
