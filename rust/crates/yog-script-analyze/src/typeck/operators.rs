//! Operators a type declares, as C#'s: `a + b` on a `Vec` is its
//! `operator +` (`op_Addition`), `(Energy)n` its `explicit operator`, an
//! `i32` handed where an `Energy` is wanted its `implicit operator`. Each
//! one's signature is typed once, its type's arguments substituted where
//! it's used; what an expression uses is noted for the compiler.

use std::collections::HashMap;

use yog_ids::script_problem;

use super::Checker;
use super::delegates::Sig;
use super::model::Type;
use super::own::TypeKey;
use super::types::OperatorUse;
use crate::ast::{BinOp, Decl, DeclKind, Expr, Modifier, OperatorKind, UnOp};
use crate::parser::expr::sign_of;
use crate::source::token::Span;

/// Each type's operators, typed: what each is, its method's name, its signature.
pub(super) type Operators = HashMap<TypeKey, Vec<(OperatorKind, String, Sig)>>;

/// The operators C# declares in pairs: one without the other is an error.
const PAIRS: [(BinOp, BinOp); 3] = [
    (BinOp::Eq, BinOp::NotEq),
    (BinOp::Lt, BinOp::Gt),
    (BinOp::LtEq, BinOp::GtEq),
];

impl Checker<'_, '_, '_> {
    /// Every own type's operators typed, quietly.
    pub(super) fn type_operators(&mut self) {
        let declared: Vec<(TypeKey, Vec<String>, Vec<crate::ast::Operator>)> = self
            .own
            .all()
            .filter(|(_, t)| !t.operators.is_empty())
            .map(|(k, t)| (k.clone(), t.generics.clone(), t.operators.clone()))
            .collect();
        let mut operators = Operators::new();
        self.quietly(|c| {
            for (key, generics, ops) in declared {
                let typed = c.as_declared(&key.name, &generics, |c| {
                    ops.iter()
                        .map(|op| {
                            (
                                op.kind,
                                op.method.name.text.clone(),
                                c.signature_types(&op.method),
                            )
                        })
                        .collect()
                });
                operators.insert(key, typed);
            }
        });
        self.operators = operators;
    }

    /// The operators of `kind` own type `t` and what it inherits declare,
    /// their signatures in `t`'s arguments.
    fn declared(&self, t: &Type, kind: OperatorKind) -> Vec<(OperatorUse, Sig)> {
        let Type::Own(name, args) = t.non_null() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (key, args) in self.lineage_of(name, args) {
            let bound = self.bound_of(&key.name, &args);
            for (k, method, (params, ret)) in self.operators.get(&key).into_iter().flatten() {
                if *k == kind {
                    let owner = Type::Own(key.name.clone(), args.clone());
                    let sig = (
                        params.iter().map(|p| p.substitute(&bound)).collect(),
                        ret.substitute(&bound),
                    );
                    out.push((
                        OperatorUse {
                            owner,
                            method: method.clone(),
                        },
                        sig,
                    ));
                }
            }
        }
        out
    }

    /// The operator `kind` of `given`'s types, as one of them declares it:
    /// the first whose parameters take them.
    fn chosen(&self, kind: OperatorKind, given: &[Type]) -> Option<(OperatorUse, Type)> {
        given
            .iter()
            .flat_map(|t| self.declared(t, kind))
            .find(|(_, (params, _))| {
                params.len() == given.len()
                    && given
                        .iter()
                        .zip(params)
                        .all(|(g, p)| self.converts_plainly(g, p))
            })
            .map(|(used, (_, ret))| (used, ret))
    }

    /// `a op b` as an own type declares it: the operator, its result.
    pub(super) fn declared_binary(
        &self,
        op: BinOp,
        a: &Type,
        b: &Type,
    ) -> Option<(OperatorUse, Type)> {
        self.chosen(OperatorKind::Binary(op), &[a.clone(), b.clone()])
    }

    /// `op x` as an own type declares it, noted on `e`: its result.
    pub(super) fn declared_unary(&mut self, e: &Expr, op: UnOp, t: &Type) -> Option<Type> {
        let declared = OperatorKind::Unary(op.declared());
        let (used, ret) = self.chosen(declared, std::slice::from_ref(t))?;
        self.types.set_operator(e, used);
        Some(ret)
    }

    /// The conversion of `kind` from `got` to `wanted` either declares.
    fn conversion(&self, kind: OperatorKind, got: &Type, wanted: &Type) -> Option<OperatorUse> {
        [got, wanted]
            .into_iter()
            .flat_map(|t| self.declared(t, kind))
            .find(|(_, (params, ret))| {
                params.len() == 1
                    && self.converts_plainly(got, &params[0])
                    && self.converts_plainly(ret, wanted)
            })
            .map(|(used, _)| used)
    }

    /// The implicit conversion from `got` to `wanted`, if one is declared.
    pub(super) fn implicit(&self, got: &Type, wanted: &Type) -> Option<OperatorUse> {
        self.conversion(OperatorKind::Implicit, got, wanted)
    }

    /// `e` (of `got`) handed where `wanted` is: the implicit conversion it
    /// goes through noted, when it needs one.
    pub(super) fn note_conversion(&mut self, e: &Expr, got: &Type, wanted: &Type) {
        if !self.converts_plainly(got, wanted)
            && let Some(used) = self.implicit(got, wanted)
        {
            self.types.set_conversion(e, used);
        }
    }

    /// `(T)e` (`e` of `got`): a conversion declared (explicit, or implicit
    /// written out) noted on the cast.
    pub(super) fn note_cast(&mut self, cast: &Expr, got: &Type, to: &Type) {
        if self.converts_plainly(got, to) {
            return;
        }
        let used = self
            .conversion(OperatorKind::Explicit, got, to)
            .or_else(|| self.implicit(got, to));
        if let Some(used) = used {
            self.types.set_operator(cast, used);
        }
    }

    /// A type's body's operators as C# allows them: `public static`, of
    /// the type itself, the paired ones together.
    pub(super) fn check_operators(&mut self, this: &Type, body: &[Decl]) {
        let mut binaries = Vec::new();
        for d in body {
            let DeclKind::Operator(op) = &d.kind else {
                continue;
            };
            let at = op.method.name.span;
            if !(d.has(Modifier::Public) && d.has(Modifier::Static)) {
                self.bad_operator(at, "an operator is `public static`");
            }
            let (params, ret) = self.signature_types(&op.method);
            let own = |t: &Type| t.non_null() == this.non_null();
            let of_this = match op.kind {
                OperatorKind::Implicit | OperatorKind::Explicit => {
                    params.iter().any(own) || own(&ret)
                }
                _ => params.iter().any(own),
            };
            if !of_this {
                self.bad_operator(at, &format!("an operator of `{this}` takes a `{this}`"));
            }
            if let OperatorKind::Binary(b) = op.kind {
                binaries.push((b, at));
            }
        }
        for (one, other) in PAIRS {
            for (a, b) in [(one, other), (other, one)] {
                if let Some((_, at)) = binaries.iter().find(|(o, _)| *o == a)
                    && !binaries.iter().any(|(o, _)| *o == b)
                {
                    let (a, b) = (sign_of(a), sign_of(b));
                    self.bad_operator(*at, &format!("`operator {a}` comes with `operator {b}`"));
                }
            }
        }
    }

    fn bad_operator(&mut self, at: Span, why: &str) {
        self.report
            .error(script_problem::OPERATOR, at, why.to_string());
    }
}
