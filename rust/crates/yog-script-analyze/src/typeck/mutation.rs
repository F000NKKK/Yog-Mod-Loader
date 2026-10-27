//! What may change (`ys.md` §3, Values and objects): a field is written,
//! or a method that isn't `readonly` (Rust's `&mut self`) is called, only
//! through something that may change here — not a `let`, an `in`
//! parameter, a `foreach` item, a pattern's name, nor `this` in a
//! `readonly` member.

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use crate::ast::{Expr, ExprKind, Method};
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// `place = …` / `place += …` on a member or an element: what holds it
    /// may change.
    pub(super) fn written(&mut self, place: &Expr) {
        self.synced_written(place);
        if let ExprKind::Index { expr, .. } = &place.kind {
            self.index_written(expr, place.span);
        }
        let why = match &place.kind {
            ExprKind::Member { expr, .. } if self.is_record(expr) => Some(
                "a record or a `readonly class` (never changed; a record by `with`)".to_string(),
            ),
            ExprKind::Member { expr, .. } | ExprKind::Index { expr, .. } => self.fixed_root(expr),
            // A field of `this` by its name.
            ExprKind::Ident(name) if self.local(name).is_none() => {
                self.readonly_this.then(this_why)
            }
            _ => None,
        };
        if let Some(why) = why {
            self.report.error(
                script_problem::BORROW,
                place.span,
                format!("{why} is read-only here: this changes it"),
            );
        }
    }

    /// `on.name(…)` (`on` None: `name(…)` on `this`), a method of own type
    /// `of`: one that changes its object is called on one that may change.
    pub(super) fn called_on(&mut self, on: Option<&Expr>, of: &str, name: &str, at: Span) {
        let readonly = self.own.lineage_named(of).iter().any(|o| {
            self.own
                .of_key(o)
                .is_some_and(|t| t.readonly_all || t.readonly.iter().any(|r| r == name))
        });
        if readonly {
            return;
        }
        let why = match on {
            Some(e) => self.fixed_root(e),
            None => self.readonly_this.then(this_why),
        };
        if let Some(why) = why {
            self.report.error(
                script_problem::BORROW,
                at,
                format!("{why} is read-only here, and `{name}` changes it: make `{name}` `readonly`, or change something that may change"),
            );
        }
    }

    /// Why what `e` reaches can't change here, if it can't: the local or
    /// `this` it's a part of. Only the source's own types are tracked.
    fn fixed_root(&self, e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::Member { expr, .. } | ExprKind::Index { expr, .. } => self.fixed_root(expr),
            ExprKind::Ident(name) => {
                let slot = self.slot(name)?;
                (slot.fixed && matches!(slot.ty.non_null(), Type::Own(..)))
                    .then(|| format!("`{name}` (a `let`, an `in`, an item or a pattern's name)"))
            }
            ExprKind::This => self.readonly_this.then(this_why),
            _ => None,
        }
    }
}

impl Checker<'_, '_, '_> {
    /// Whether method `m` of `this`'s type implements or overrides a
    /// `readonly` one (`object`'s `ToString`, an interface's): then it's
    /// `readonly` too, or a call through that one (on an `in`) would
    /// change what mustn't.
    pub(super) fn implements_readonly(&self, m: &Method) -> bool {
        let Some(Type::Own(of, _)) = &self.this else {
            return false;
        };
        let name = &m.name.text;
        self.own.lineage_named(of).into_iter().skip(1).any(|k| {
            self.own
                .of_key(&k)
                .is_some_and(|t| t.readonly.iter().any(|r| r == name))
        })
    }
}

fn this_why() -> String {
    "`this` (in a `readonly` member)".to_string()
}

impl Checker<'_, '_, '_> {
    /// Whether `e` — a local or `this` — is a record: set once, never written.
    fn is_record(&self, e: &Expr) -> bool {
        let t = match &e.kind {
            ExprKind::Ident(name) => self.local(name),
            ExprKind::This => self.this.as_ref(),
            _ => None,
        };
        match t.map(Type::non_null) {
            Some(Type::Own(name, args)) => {
                self.own.get(name, args.len()).is_some_and(|t| t.shared())
            }
            _ => false,
        }
    }

    /// Whether `this` is a `readonly struct`: every member borrows it read-only.
    pub(super) fn this_readonly(&self) -> bool {
        match &self.this {
            Some(Type::Own(name, args)) => self
                .own
                .get(name, args.len())
                .is_some_and(|t| t.readonly_all),
            _ => false,
        }
    }
}
