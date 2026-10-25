//! Lambdas and methods where a delegate is wanted (`Player.Join += e => …`,
//! `Charged += OnCharged`): the lambda's parameters typed by the delegate,
//! its body checked with them; a method matched by its signature.

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use crate::ast::{Expr, ExprKind, IT, LambdaBody, LambdaParam, Stmt};
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// `e` where delegate `params → ret` is wanted, when it's a lambda or
    /// a method: checked as one; whether it was one.
    pub(super) fn as_delegate(&mut self, e: &Expr, params: &[Type], ret: &Type) -> bool {
        match &e.kind {
            ExprKind::Lambda {
                params: written,
                body,
            } => {
                self.lambda_as(written, body, params, ret, e.span);
                true
            }
            ExprKind::Ident(name) if self.local(name).is_none() => {
                self.method_as(name, params, ret, e.span)
            }
            _ => false,
        }
    }

    fn lambda_as(
        &mut self,
        written: &[LambdaParam],
        body: &LambdaBody,
        params: &[Type],
        ret: &Type,
        at: Span,
    ) {
        if !written.is_empty() && written.len() != params.len() {
            self.report.error(
                script_problem::WRONG_ARGS,
                at,
                format!(
                    "the lambda takes {} parameters, the delegate gives {}",
                    written.len(),
                    params.len()
                ),
            );
        }
        let returns = self.returns.replace(ret.clone());
        self.scoped(|c| {
            match (written, params) {
                ([], [one]) => c.bind(IT, one.clone()),
                _ => {
                    for (w, given) in written.iter().zip(params) {
                        let t = match &w.ty {
                            Some(declared) => {
                                let declared = c.type_of(declared);
                                c.want(given, &declared, w.name.span);
                                declared
                            }
                            None => given.clone(),
                        };
                        c.bind(&w.name.text, t);
                    }
                }
            }
            match body {
                LambdaBody::Expr(e) if *ret == Type::Void => {
                    c.expr(e);
                }
                LambdaBody::Expr(e) => c.fits(e, ret),
                LambdaBody::Block(stmts) => c.lambda_block(stmts, ret),
            }
        });
        self.returns = returns;
    }

    /// A block lambda's statements; its last expression without `;` its value.
    fn lambda_block(&mut self, stmts: &[Stmt], ret: &Type) {
        match stmts.split_last() {
            Some((Stmt::Value(v), before)) if *ret != Type::Void => {
                self.stmts(before);
                self.fits(v, ret);
            }
            _ => self.stmts(stmts),
        }
    }

    /// Method `name` where delegate `params → ret` is wanted: one of its
    /// overloads must take those and give that.
    fn method_as(&mut self, name: &str, params: &[Type], ret: &Type, at: Span) -> bool {
        let methods = self.callable(name);
        if methods.is_empty() {
            return false;
        }
        let fits = methods.iter().any(|(m, context)| {
            let (taken, given) = self.in_context(context, |c| c.signature_types(m));
            taken.len() == params.len()
                && params.iter().zip(&taken).all(|(p, t)| p.converts_to(t))
                && given.converts_to(ret)
        });
        if !fits {
            let wanted = Type::Fn(params.to_vec(), Box::new(ret.clone()));
            self.report.error(
                script_problem::TYPE_MISMATCH,
                at,
                format!("no overload of `{name}` is a `{wanted}`"),
            );
        }
        true
    }
}

impl Checker<'_, '_, '_> {
    /// A lambda with nothing it must fit: a delegate of its parameters'
    /// written types and its body's value (unknown when one isn't written).
    pub(super) fn lambda_alone(&mut self, params: &[LambdaParam], body: &LambdaBody) -> Type {
        let typed: Vec<Option<Type>> = params
            .iter()
            .map(|p| p.ty.as_ref().map(|t| self.type_of(t)))
            .collect();
        let ret = self.scoped(|c| {
            for (p, t) in params.iter().zip(&typed) {
                c.bind(&p.name.text, t.clone().unwrap_or(Type::Unknown));
            }
            match body {
                LambdaBody::Expr(e) => c.expr(e),
                LambdaBody::Block(stmts) => {
                    c.stmts(stmts);
                    Type::Unknown
                }
            }
        });
        match typed.into_iter().collect::<Option<Vec<Type>>>() {
            Some(params) => Type::Fn(params, Box::new(ret)),
            None => Type::Unknown,
        }
    }
}
