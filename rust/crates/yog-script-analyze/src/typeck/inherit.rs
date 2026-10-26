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
        explicit(&index, info, report);
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
        let name = &b.0;
        let Some(base) = index.base(b) else {
            continue;
        };
        if base.interface {
            continue;
        }
        classes += 1;
        let problem = if !info.class {
            Some(format!(
                "`{}` inherits interfaces only: `{}` is a class",
                info.name, name.text
            ))
        } else if classes > 1 {
            Some(format!(
                "`{}` inherits one class; `{}` is a second",
                info.name, name.text
            ))
        } else if i > 0 {
            Some(format!(
                "the base class `{}` comes first, before the interfaces",
                name.text
            ))
        } else if base.sealed {
            Some(format!("`{}` is sealed: nothing inherits it", name.text))
        } else if base.readonly && !info.readonly {
            Some(format!(
                "`{}` is never changed, so shared: what inherits it is `readonly` too",
                name.text
            ))
        } else {
            None
        };
        if let Some(message) = problem {
            report.error(script_problem::INHERIT, name.span, message);
        }
    }
}

/// Each `override` replaces something a base lets it.
fn overrides(index: &Index, info: &Info, report: &mut Report<'_>) {
    for m in info.members.iter().filter(|m| m.overrides) {
        let replaced = index
            .ancestors(info)
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

/// Each explicit implementation (`void ICollection<T>.Add(T item)`) is of
/// an interface the type inherits, one declaring the member, and says no
/// access of its own (C#'s CS0538, CS0539, CS0106).
fn explicit(index: &Index, info: &Info, report: &mut Report<'_>) {
    for m in &info.members {
        let Some(of) = &m.explicit else {
            continue;
        };
        let ancestors = index.ancestors(info);
        let problem = match ancestors.iter().find(|a| a.name == of) {
            None => Some(format!("`{}` doesn't implement `{of}`", info.name)),
            Some(a) if !a.interface => Some(format!("`{of}` isn't an interface")),
            Some(a) if !a.members.iter().any(|b| b.name == m.name) => {
                Some(format!("`{of}` has no `{}` to implement", m.name))
            }
            Some(_) if m.access => Some(format!(
                "`{of}.{}` is reached through `{of}` only: it says no access of its own",
                m.name
            )),
            Some(_) => None,
        };
        if let Some(message) = problem {
            report.error(script_problem::INHERIT, m.span, message);
        }
    }
}

/// A class that's made implements what its bases leave to it.
fn implemented(index: &Index, info: &Info, report: &mut Report<'_>) {
    let lineage: Vec<&Info> = std::iter::once(info).chain(index.ancestors(info)).collect();
    // An explicit implementation implements its own interface's only.
    let has = |name: &str, of: &str| {
        lineage
            .iter()
            .filter(|i| i.class)
            .flat_map(|i| i.members.iter().map(move |m| (i, m)))
            .filter(|(_, m)| m.explicit.as_deref().is_none_or(|e| e == of))
            // The runtime's class implements what it declares, bodies or not.
            .any(|(i, m)| m.name == name && (m.body || i.intrinsic))
    };
    for owner in &lineage {
        for m in owner
            .members
            .iter()
            .filter(|m| m.is_abstract || owner.interface && !m.body)
        {
            if !has(&m.name, owner.name) {
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
