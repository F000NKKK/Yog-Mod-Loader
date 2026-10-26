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
                asynchronous,
            } => {
                // An `async` one's `return` gives what its task finishes with.
                let gives = if *asynchronous {
                    self.async_returns(ret, e.span)
                } else {
                    ret.clone()
                };
                self.lambda_as(written, body, params, &gives, e.span);
                // Its type the delegate's, not what it was alone.
                let own = Type::Fn(params.to_vec(), Box::new(ret.clone()));
                self.types.set(e, own);
                true
            }
            _ => self.group_as(e, params, ret),
        }
    }

    /// Lambda `e` where expression tree `wanted` is: typed as its
    /// delegate's, noted as quoted; one of a block can't be a tree (C#'s
    /// CS0834). Whether it was one.
    pub(super) fn quoted_as(&mut self, e: &Expr, wanted: &Type) -> bool {
        let (ExprKind::Lambda { body, .. }, Some(delegate)) =
            (&e.kind, Self::quoted_delegate(wanted))
        else {
            return false;
        };
        let delegate = delegate.clone();
        let Some((params, ret)) = self.delegate_sig(&delegate) else {
            return false;
        };
        if matches!(body, LambdaBody::Block(_)) {
            self.report.error(
                script_problem::TYPE_MISMATCH,
                e.span,
                "an expression tree is a lambda of an expression, not of a block".to_string(),
            );
        }
        self.as_delegate(e, &params, &ret);
        self.types.set(e, wanted.non_null().clone());
        self.types.set_quoted(
            e,
            super::types::Quoted {
                delegate,
                params,
                ret,
            },
        );
        true
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
            c.bind_lambda(written, params);
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

    /// A lambda's parameters bound to what its delegate gives (`it`, when
    /// it names none and takes one).
    fn bind_lambda(&mut self, written: &[LambdaParam], params: &[Type]) {
        match (written, params) {
            ([], [one]) => self.bind(IT, one.clone()),
            _ => {
                for (w, given) in written.iter().zip(params) {
                    let t = match &w.ty {
                        Some(declared) => {
                            let declared = self.type_of(declared);
                            self.want(given, &declared, w.name.span);
                            declared
                        }
                        None => given.clone(),
                    };
                    self.bind(&w.name.text, t);
                }
            }
        }
    }

    /// What lambda `e` gives taking `params`: its expression's type, or
    /// what its block returns (the first `return`'s, or its last value).
    pub(super) fn lambda_result(&mut self, e: &Expr, params: &[Type]) -> Type {
        let ExprKind::Lambda {
            params: written,
            body,
            asynchronous,
        } = &e.kind
        else {
            return Type::Unknown;
        };
        let returns = self.returns.take();
        let inferred = self.inferred.replace(Vec::new());
        let t = self.scoped(|c| {
            c.bind_lambda(written, params);
            match body {
                LambdaBody::Expr(e) => c.expr(e),
                LambdaBody::Block(stmts) => {
                    let last = match stmts.split_last() {
                        Some((Stmt::Value(v), before)) => {
                            c.stmts(before);
                            Some(c.expr(v))
                        }
                        _ => {
                            c.stmts(stmts);
                            None
                        }
                    };
                    let returned = c.inferred.as_ref().and_then(|r| r.first().cloned());
                    returned.or(last).unwrap_or(Type::Void)
                }
            }
        });
        self.returns = returns;
        self.inferred = inferred;
        if *asynchronous {
            // An `async` one gives a task of it.
            let args = if t == Type::Void { Vec::new() } else { vec![t] };
            return Type::Own(yog_script_ids::system::TASK.to_string(), args);
        }
        t
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
