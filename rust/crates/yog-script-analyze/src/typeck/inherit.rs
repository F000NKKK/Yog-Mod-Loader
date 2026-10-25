//! Inheritance's rules, as C#'s: one base class, written first, never a
//! sealed one; structs and interfaces inherit interfaces only; an
//! `override` replaces a `virtual`, `abstract` or `override` member of a
//! base; a class that isn't abstract implements every abstract member and
//! interface member it inherits.

mod index;

use yog_ids::script_problem;

use crate::ast::Decl;
use index::{Index, Info};
use yog_analysis::Report;

/// Every rule checked over the source's own types (`decls`), what they
/// inherit from the library's (`library`) too.
pub(super) fn check(library: &[Decl], decls: &[Decl], report: &mut Report<'_>) {
    let mut index = Index::collect(library);
    index.mark_library();
    index.add(decls);
    for info in index.types.values().filter(|i| !i.library) {
        bases(&index, info, report);
        overrides(&index, info, report);
        if info.class && !info.is_abstract {
            implemented(&index, info, report);
        }
    }
}

/// `: Base, IA, IB`: at most one class, first, not sealed; none for a
/// struct or an interface.
fn bases(index: &Index, info: &Info, report: &mut Report<'_>) {
    let mut classes = 0;
    for (i, b) in info.bases.iter().enumerate() {
        let Some(base) = index.types.get(b.text.as_str()) else {
            continue;
        };
        if base.interface {
            continue;
        }
        classes += 1;
        let problem = if !info.class {
            Some(format!(
                "`{}` inherits interfaces only: `{}` is a class",
                info.name, b.text
            ))
        } else if classes > 1 {
            Some(format!(
                "`{}` inherits one class; `{}` is a second",
                info.name, b.text
            ))
        } else if i > 0 {
            Some(format!(
                "the base class `{}` comes first, before the interfaces",
                b.text
            ))
        } else if base.sealed {
            Some(format!("`{}` is sealed: nothing inherits it", b.text))
        } else {
            None
        };
        if let Some(message) = problem {
            report.error(script_problem::INHERIT, b.span, message);
        }
    }
}

/// Each `override` replaces something a base lets it.
fn overrides(index: &Index, info: &Info, report: &mut Report<'_>) {
    for m in info.members.iter().filter(|m| m.overrides) {
        let replaced = index
            .ancestors(info.name)
            .iter()
            .flat_map(|a| &a.members)
            .any(|b| b.name == m.name && (b.is_abstract || b.is_virtual || b.overrides));
        if !replaced {
            report.error(
                script_problem::INHERIT,
                m.span,
                format!(
                    "`{}` overrides nothing: no base has a `virtual` or `abstract` `{}`",
                    m.name, m.name
                ),
            );
        }
    }
}

/// A class that's made implements what its bases leave to it.
fn implemented(index: &Index, info: &Info, report: &mut Report<'_>) {
    let lineage: Vec<&Info> = std::iter::once(info)
        .chain(index.ancestors(info.name))
        .collect();
    let has = |name: &str| {
        lineage
            .iter()
            .filter(|i| i.class)
            .flat_map(|i| i.members.iter().map(move |m| (i, m)))
            // The runtime's class implements what it declares, bodies or not.
            .any(|(i, m)| m.name == name && (m.body || i.intrinsic))
    };
    for owner in &lineage {
        for m in owner
            .members
            .iter()
            .filter(|m| m.is_abstract || owner.interface && !m.body)
        {
            if !has(&m.name) {
                report.error(
                    script_problem::INHERIT,
                    info.span,
                    format!(
                        "`{}` doesn't implement `{}` of `{}`",
                        info.name, m.name, owner.name
                    ),
                );
            }
        }
    }
}

impl super::Checker<'_, '_, '_> {
    /// `new T(…)`: `T` isn't abstract (nor an interface).
    pub(super) fn made(&mut self, t: &super::Type, at: crate::source::token::Span) {
        let super::Type::Own(name, args) = t else {
            return;
        };
        if self
            .own
            .get(name, args.len())
            .is_some_and(|o| o.is_abstract || o.interface)
        {
            self.report.error(
                script_problem::INHERIT,
                at,
                format!("`{name}` is abstract: make a class that inherits it"),
            );
        }
    }
}
