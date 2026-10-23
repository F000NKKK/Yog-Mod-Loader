//! Names in C#'s case (`ys-grammar.md` §1.4): a warning when one isn't.

use yog_ids::script_problem;

use super::Resolver;
use crate::ast::Name;

/// The case a kind of name is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Case {
    /// Types, members, cases, constants: `ChargeLevel`.
    Pascal,
    /// Parameters and locals: `chargeLevel`.
    Camel,
    /// Interfaces: `IChargeable`.
    Interface,
    /// Type parameters: `T`, `TItem`.
    TypeParam,
    /// Whatever it's written as (names the game or a format gives).
    Any,
}

/// A discard: `_` names nothing.
const DISCARD: &str = "_";
/// What an interface's and a type parameter's names start with.
const INTERFACE_MARK: char = 'I';
const TYPE_PARAM_MARK: char = 'T';
/// What C#'s cases don't have inside a name.
const SEPARATOR: char = '_';

impl Case {
    /// Whether `name` is written in this case.
    fn fits(self, name: &str) -> bool {
        let mut chars = name.chars();
        let (Some(first), second) = (chars.next(), chars.next()) else {
            return true;
        };
        if name.contains(SEPARATOR) && self != Case::Any {
            return false;
        }
        match self {
            Case::Pascal => first.is_uppercase(),
            Case::Camel => first.is_lowercase(),
            Case::Interface => first == INTERFACE_MARK && second.is_some_and(char::is_uppercase),
            Case::TypeParam => first == TYPE_PARAM_MARK && second.is_none_or(char::is_uppercase),
            Case::Any => true,
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Case::Pascal => "PascalCase (`ChargeLevel`)",
            Case::Camel => "camelCase (`chargeLevel`)",
            Case::Interface => "`I` and PascalCase (`IChargeable`)",
            Case::TypeParam => "`T` or `T` and PascalCase (`TItem`)",
            Case::Any => "any case",
        }
    }
}

impl Resolver<'_, '_> {
    /// A warning when `name` isn't in `case`.
    pub(super) fn check_case(&mut self, name: &Name, case: Case) {
        // A macro's renamed local is checked as written in the macro.
        let written = name
            .text
            .split(crate::macros::HYGIENE_MARK)
            .next()
            .unwrap_or_default();
        if written == DISCARD || case.fits(written) {
            return;
        }
        self.report.warning(
            script_problem::NAMING,
            name.span,
            format!("`{}` should be written in {}", name.text, case.describe()),
        );
    }
}
