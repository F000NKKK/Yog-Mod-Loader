//! What a source declares and where each name is used: the definitions,
//! the scopes they're looked up in, and the references found.

use std::collections::HashMap;

use yog_ids::script_problem;

use super::Resolver;
use super::naming::Case;
use crate::ast::Name;
use crate::source::token::Span;

/// What a name stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    /// `const`: known at build time.
    Constant,
    /// `readonly`, a computed property, a `foreach`'s item: read only.
    ReadOnly,
    /// A local, a pattern's binding, a patch's written local.
    Variable,
    Param,
    Method,
    /// A type: a class, record, union, enum, interface, alias, unit, generic.
    Type,
    /// An enum's case, a union's variant.
    Case,
    /// A field or a property that can be set.
    Field,
    /// A block, an item, a mod's own kind of content; a screen.
    Content,
    Component,
    /// A name a `using` brings in.
    Import,
    /// `'a`, declared by a generic list.
    Lifetime,
}

impl DefKind {
    /// A local: `let mut` or `let`.
    pub fn local(mutable: bool) -> Self {
        if mutable {
            DefKind::Variable
        } else {
            DefKind::ReadOnly
        }
    }

    /// Whether it may be assigned to.
    pub fn assignable(self) -> bool {
        matches!(self, DefKind::Variable | DefKind::Param | DefKind::Field)
    }
}

/// One definition: its name, where, what it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Def {
    pub name: String,
    pub span: Span,
    pub kind: DefKind,
}

/// A source's definitions, and each use of one (where, which).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Symbols {
    pub defs: Vec<Def>,
    pub refs: Vec<(Span, usize)>,
}

impl Symbols {
    /// The definition the name at `offset` (a byte in the source) stands
    /// for: a use of it, or the definition itself.
    pub fn def_at(&self, offset: u32) -> Option<&Def> {
        let inside = |s: Span| s.start <= offset && offset < s.end;
        self.refs
            .iter()
            .find(|(s, _)| inside(*s))
            .and_then(|(_, d)| self.defs.get(*d))
            .or_else(|| self.defs.iter().find(|d| inside(d.span)))
    }
}

/// The names one scope declares.
#[derive(Default)]
pub(super) struct Scope {
    names: HashMap<String, usize>,
    /// The types declared here by name and arity: `Box` and `Box<T>` are
    /// two (C#'s), one name for lookup.
    types: HashMap<(String, usize), usize>,
}

impl Resolver<'_, '_> {
    /// Runs `f` in a scope of its own.
    pub(super) fn scoped<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.scopes.push(Scope::default());
        let out = f(self);
        self.scopes.pop();
        out
    }

    /// Declares `name` here, cased as `case` says; a second one of a name in
    /// one scope is an error, unless `again` allows it (a version's other branch).
    pub(super) fn define(&mut self, name: &Name, kind: DefKind, case: Case, again: bool) {
        let id = self.symbols.defs.len();
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };
        if let Some(&first) = scope.names.get(&name.text) {
            // A method beside methods of its name is an overload (C#'s): the
            // type checker tells them apart by their parameters.
            let overload =
                kind == DefKind::Method && self.symbols.defs[first].kind == DefKind::Method;
            if !again && !overload {
                let at = self.symbols.defs[first].span;
                let line = self.report.line_of(at);
                self.report.error(
                    script_problem::DUPLICATE,
                    name.span,
                    format!("`{}` is declared already (line {line})", name.text),
                );
            }
            return;
        }
        scope.names.insert(name.text.clone(), id);
        self.symbols.defs.push(Def {
            name: name.text.clone(),
            span: name.span,
            kind,
        });
        self.check_case(name, case);
    }

    /// Declares type `name` of `arity` type parameters: a second of the same
    /// name and arity is an error (unless `again`), one of another arity a
    /// type apart.
    pub(super) fn define_type(&mut self, name: &Name, arity: usize, case: Case, again: bool) {
        let key = (name.text.clone(), arity);
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };
        if let Some(&first) = scope.types.get(&key) {
            if !again {
                let at = self.symbols.defs[first].span;
                let line = self.report.line_of(at);
                self.report.error(
                    script_problem::DUPLICATE,
                    name.span,
                    format!(
                        "`{}` of {arity} type parameter(s) is declared already (line {line})",
                        name.text
                    ),
                );
            }
            return;
        }
        let id = self.symbols.defs.len();
        scope.types.insert(key, id);
        scope.names.entry(name.text.clone()).or_insert(id);
        self.symbols.defs.push(Def {
            name: name.text.clone(),
            span: name.span,
            kind: DefKind::Type,
        });
        self.check_case(name, case);
    }

    /// Declares `name` in a block: a later one of the name hides the earlier.
    pub(super) fn bind(&mut self, name: &Name, kind: DefKind) {
        let id = self.symbols.defs.len();
        if let Some(scope) = self.scopes.last_mut() {
            scope.names.insert(name.text.clone(), id);
            self.symbols.defs.push(Def {
                name: name.text.clone(),
                span: name.span,
                kind,
            });
        }
        self.check_case(name, Case::Camel);
    }

    /// The definition `name` stands for here, innermost first.
    pub(super) fn lookup(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.names.get(name).copied())
    }

    /// A use of `name` at `span`: recorded when it's one of the source's
    /// own (others are the game's and the library's, checked later).
    pub(super) fn refer(&mut self, name: &str, span: Span) -> Option<DefKind> {
        let id = self.lookup(name)?;
        self.symbols.refs.push((span, id));
        Some(self.symbols.defs[id].kind)
    }
}
