//! List patterns (C#'s): `[1, let second, .., let last]` matches an
//! array's, a string's or a list's items — each item's pattern on its
//! item, `..` at most once, `.. let rest` its slice, of the matched kind.

use yog_ids::script_problem;
use yog_script_ids::system::LIST;

use super::Checker;
use super::model::Type;
use crate::ast::Pattern;
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// `[items]` (at `at`) on a value of type `on`.
    pub(super) fn list_pattern(&mut self, items: &[Pattern], on: &Type, at: Span) {
        let item = match list_item(on) {
            Some(t) => t,
            None if on.known() => {
                self.report.error(
                    script_problem::TYPE_MISMATCH,
                    at,
                    format!(
                        "a list pattern matches an array, a string or a `List<T>`, not a `{on}`"
                    ),
                );
                Type::Unknown
            }
            None => Type::Unknown,
        };
        let slices = items
            .iter()
            .filter(|p| matches!(p, Pattern::Slice(_)))
            .count();
        if slices > 1 {
            self.report.error(
                script_problem::EXPECTED,
                at,
                "a list pattern has `..` once at most",
            );
        }
        for p in items {
            match p {
                Pattern::Slice(Some(rest)) => self.bind_arm(rest, on.non_null()),
                Pattern::Slice(None) => {}
                one => self.bind_arm(one, &item),
            }
        }
    }
}

/// What a list pattern on `on` matches each item of: an array's item, a
/// string's `char`, a list's `T`.
pub fn list_item(on: &Type) -> Option<Type> {
    match on.non_null() {
        Type::Array(item, 1) => Some((**item).clone()),
        Type::String => Some(Type::Char),
        Type::Own(name, args) if name == LIST && args.len() == 1 => args.first().cloned(),
        _ => None,
    }
}
