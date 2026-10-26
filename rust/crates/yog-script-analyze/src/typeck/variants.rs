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
        }])
    }

    /// `x switch { … }`: each arm with what its pattern binds; the first
    /// arm's value is the switch's; an enum's cases all covered.
    pub(super) fn switch(&mut self, on: &Expr, arms: &[Arm], at: Span) -> Type {
        let t = self.expr(on);
        let mut result = Type::Unknown;
        for a in arms {
            self.scoped(|c| {
                c.bind_arm(&a.pattern, &t);
                if let Some(g) = &a.guard {
                    c.condition(g);
                }
                let v = c.expr(&a.value);
                if !result.known() {
                    result = v;
                }
            });
        }
        self.exhaustive(&t, arms, at);
        result
    }

    /// The names an arm's pattern binds, typed by the value it matches.
    fn bind_arm(&mut self, p: &Pattern, on: &Type) {
        match p {
            Pattern::Let(n) => self.bind_as(&n.text, on.clone(), true),
            Pattern::Variant { path, args } => {
                let fields = match (on.non_null(), path.last()) {
                    (Type::Own(of, args), Some(case)) => self
                        .own
                        .get(of, args.len())
                        .and_then(|t| t.variants.get(&case.text))
                        .cloned()
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
                for (i, arg) in args.iter().enumerate() {
                    let ft = fields.get(i).map_or(Type::Unknown, |f| self.type_of(&f.ty));
                    self.bind_arm(arg, &ft);
                }
            }
            Pattern::Or(a, b) | Pattern::And(a, b) => {
                self.bind_arm(a, on);
                self.bind_arm(b, on);
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
