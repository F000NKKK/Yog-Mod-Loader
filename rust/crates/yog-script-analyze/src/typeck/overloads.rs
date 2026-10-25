//! Methods of one name side by side, as C#'s overloads: each told apart by
//! its parameters' types, two of the same ones an error (C#'s CS0111).

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use crate::ast::{Decl, DeclKind};

impl Checker<'_, '_, '_> {
    /// Each method of `body` whose name and parameters' types another
    /// before it has already.
    pub(super) fn check_overloads(&mut self, body: &[Decl]) {
        let mut seen: Vec<(&str, Vec<Type>)> = Vec::new();
        for d in body {
            let DeclKind::Method(m) = &d.kind else {
                continue;
            };
            // An explicit implementation is its interface's member.
            if m.explicit.is_some() {
                continue;
            }
            // Typed again for the comparison only: the method reports its own.
            let (params, _) = self.quietly(|c| c.signature_types(m));
            let name = m.name.text.as_str();
            if seen.iter().any(|(n, p)| *n == name && *p == params) {
                self.report.error(
                    script_problem::DUPLICATE,
                    m.name.span,
                    format!("`{name}` taking these parameters is declared already"),
                );
            }
            seen.push((name, params));
        }
    }
}
