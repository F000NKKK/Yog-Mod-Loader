//! Enums as Rust's: a case made with its data (`Mode.Charging(4)`), taken
//! apart by a pattern (`Mode.Charging(let rate)`), and a `switch` over one
//! that must cover every case.

use yog_ids::script_problem;

use super::Checker;
use super::members::{Callee, Signature};
use super::model::Type;
use crate::ast::{Arm, Expr, ExprKind, Pattern, TypeKind};
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// `Mode.Charging(…)`: case `name` of own enum `of`, made with its fields.
    pub(super) fn variant_ctor(&mut self, of: &str, name: &str) -> Callee {
        let Some((_, own)) = self.own.named(of) else {
            return Callee::Unknown;
        };
        let Some(fields) = own.variants.get(name).cloned() else {
            return Callee::Unknown;
        };
        // A generic enum's case: its parameters inferred from the call.
        let generics = own.generics.clone();
        let params_of = |g: &String| Type::Param(g.clone());
        let params: Vec<Type> = self.as_declared(of, &generics, |c| {
            fields.iter().map(|f| c.type_of(&f.ty)).collect()
        });
        let ret = Type::Own(of.to_string(), generics.iter().map(params_of).collect());
        Callee::Typed(vec![Signature {
            passes: vec![None; params.len()],
            required: params.len(),
            params,
            ret,
            generics,
            bound: Default::default(),
            rest: false,
        }])
    }

    /// `x switch { … }`: each arm with what its pattern binds; the first
    /// arm's value is the switch's; an enum's cases all covered.
    /// `on switch { arms }` (at `at`); `target`: the type wanted of it,
    /// each arm's value fitting it (C#'s target-typed `switch`), else
    /// its first arm's.
    pub(super) fn switch(
        &mut self,
        on: &Expr,
        arms: &[Arm],
        at: Span,
        target: Option<&Type>,
    ) -> Type {
        let t = self.expr(on);
        let mut result = target.cloned().unwrap_or(Type::Unknown);
        for a in arms {
            self.scoped(|c| {
                c.bind_arm(&a.pattern, &t);
                if let Some(g) = &a.guard {
                    c.condition(g);
                }
                let v = match target {
                    Some(wanted) => {
                        c.fits(&a.value, wanted);
                        wanted.clone()
                    }
                    None => c.expr(&a.value),
                };
                if !result.known() {
                    result = v;
                }
            });
        }
        self.exhaustive(&t, arms, at);
        result
    }

    /// The names pattern `p` binds (an arm's, an `is`'s), typed by the
    /// value it matches.
    pub(super) fn bind_arm(&mut self, p: &Pattern, on: &Type) {
        match p {
            Pattern::Let(n) => {
                self.types.set_binding(n, on.clone());
                self.bind_as(&n.text, on.clone(), true);
            }
            Pattern::Variant { path, args } => {
                let (fields, context) = match (on.non_null(), path.last()) {
                    (Type::Own(of, of_args), Some(case)) => (
                        self.own
                            .get(of, of_args.len())
                            .and_then(|t| t.variants.get(&case.text))
                            .cloned()
                            .unwrap_or_default(),
                        // Its fields' types as what's matched gives its parameters.
                        Some(self.context_of(of, of_args)),
                    ),
                    _ => (Vec::new(), None),
                };
                for (i, arg) in args.iter().enumerate() {
                    let ft = match (fields.get(i), &context) {
                        (Some(f), Some(c)) => self.in_context(c, |c| c.type_of(&f.ty)),
                        _ => Type::Unknown,
                    };
                    self.bind_arm(arg, &ft);
                }
            }
            Pattern::Or(a, b) | Pattern::And(a, b) => {
                self.bind_arm(a, on);
                self.bind_arm(b, on);
            }
            // `(let a, 0)`: each item's pattern on its item.
            Pattern::Tuple(items) => {
                let types = match on.non_null() {
                    Type::Tuple(types, _) => types.clone(),
                    _ => Vec::new(),
                };
                for (i, item) in items.iter().enumerate() {
                    let t = types.get(i).cloned().unwrap_or(Type::Unknown);
                    self.bind_arm(item, &t);
                }
            }
            Pattern::List(items, at) => self.list_pattern(items, on, *at),
            // `Monster { Health: let h }`: each member's pattern on the member.
            Pattern::Property { ty, fields } => {
                let t = ty.as_ref().map_or_else(|| on.clone(), |t| self.type_of(t));
                for (name, field) in fields {
                    let ft = self.own_member(&t, &name.text);
                    self.bind_arm(field, &ft);
                }
            }
            _ => self.bind_pattern(p),
        }
    }

    /// A `switch` over own enum `on` names every case, or has a `_`.
    fn exhaustive(&mut self, on: &Type, arms: &[Arm], at: Span) {
        let Type::Own(of, args) = on.non_null() else {
            return;
        };
        let Some(cases) = self.own.get(of, args.len()).map(|t| t.cases.clone()) else {
            return;
        };
        if cases.is_empty() {
            return;
        }
        let mut covered = Vec::new();
        for a in arms.iter().filter(|a| a.guard.is_none()) {
            if !cases_of(&a.pattern, &mut covered) {
                return;
            }
        }
        let missing: Vec<&String> = cases.iter().filter(|c| !covered.contains(c)).collect();
        if missing.is_empty() {
            return;
        }
        let names: Vec<String> = missing.iter().map(|c| format!("`{of}.{c}`")).collect();
        self.report.error(
            script_problem::NOT_EXHAUSTIVE,
            at,
            format!(
                "this `switch` misses {}: name them, or end with `_ =>`",
                names.join(", ")
            ),
        );
    }
}

/// The cases pattern `p` covers, added to `covered`; false when it covers
/// every value (`_`, `let x`).
fn cases_of(p: &Pattern, covered: &mut Vec<String>) -> bool {
    match p {
        Pattern::Wild(_) | Pattern::Let(_) => false,
        Pattern::Variant { path, args } => {
            // A case with its data matched only in part covers nothing whole.
            if args.iter().all(|a| !cases_of(a, &mut Vec::new())) {
                covered.extend(path.last().map(|n| n.text.clone()));
            }
            true
        }
        // `Mode.Idle` reads as a path: its last name is the case.
        Pattern::Type { ty, bind: None } => {
            if let TypeKind::Path { path, .. } = &ty.kind {
                covered.extend(path.last().map(|n| n.text.clone()));
            }
            true
        }
        Pattern::Value(e) => {
            if let ExprKind::Member { name, .. } = &e.kind {
                covered.push(name.text.clone());
            }
            true
        }
        Pattern::Or(a, b) => cases_of(a, covered) && cases_of(b, covered),
        _ => true,
    }
}

impl Checker<'_, '_, '_> {
    /// `x with { Y = 64 }`: a copy of a record or a struct (`x`'s type),
    /// each member named one of it, its value fitting it.
    pub(super) fn with(&mut self, on: &Expr, inits: &[(crate::ast::Name, Expr)], at: Span) -> Type {
        let t = self.expr(on);
        let copied = match t.non_null() {
            Type::Own(name, args) => self
                .own
                .get(name, args.len())
                .is_some_and(|o| o.record || !o.heap),
            _ => !t.known(),
        };
        if !copied {
            self.report.error(
                script_problem::TYPE_MISMATCH,
                at,
                format!("`with` copies a record or a struct, not a `{t}`"),
            );
        }
        for (name, value) in inits {
            let member = self.own_member(&t, &name.text);
            if !member.known() && t.known() {
                self.unknown_member(&t, &name.text, name.span);
            }
            self.fits(value, &member);
        }
        t
    }
}
