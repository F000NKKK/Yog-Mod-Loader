//! What follows a primary: calls, trailing lambdas, indexing, members,
//! type arguments.

use super::{PResult, Parser};
use crate::ast::{Arg, Expr, ExprKind};
use crate::token::{Punct, Tok};

impl Parser<'_, '_, '_> {
    pub(crate) fn postfix(&mut self, mut expr: Expr) -> PResult<Expr> {
        loop {
            expr = match self.tok() {
                Tok::Punct(Punct::LParen) => self.call(expr)?,
                Tok::Punct(Punct::LBrace) if !self.no_brace => self.trailing_lambda(expr)?,
                Tok::Punct(Punct::LBracket) => self.index(expr)?,
                Tok::Punct(p @ (Punct::Dot | Punct::SafeDot)) => {
                    let safe = *p == Punct::SafeDot;
                    self.bump();
                    let name = self.any_name("a member")?;
                    let span = expr.span.to(name.span);
                    Expr {
                        kind: ExprKind::Member {
                            expr: Box::new(expr),
                            name,
                            safe,
                        },
                        span,
                    }
                }
                Tok::Punct(Punct::Lt) if self.type_args_ahead().is_some() => {
                    let args = self.type_args()?;
                    let span = expr.span.to(self.prev());
                    Expr {
                        kind: ExprKind::TypeArgs {
                            expr: Box::new(expr),
                            args,
                        },
                        span,
                    }
                }
                _ => return Ok(expr),
            };
        }
    }

    fn call(&mut self, callee: Expr) -> PResult<Expr> {
        self.bump();
        let args = self.braces(|p| p.list(Punct::RParen, Self::arg))?;
        let span = callee.span.to(self.prev());
        Ok(Expr {
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
            span,
        })
    }

    /// `name = value` or `value`.
    fn arg(&mut self) -> PResult<Arg> {
        let named = matches!(self.tok(), Tok::Ident(_)) && *self.nth(1) == Tok::Punct(Punct::Eq);
        let name = if named {
            let name = self.name("an argument")?;
            self.bump();
            Some(name)
        } else {
            None
        };
        Ok(Arg {
            name,
            value: self.expr()?,
        })
    }

    /// `f { … }` / `f(a) { … }`: the lambda is the last argument.
    fn trailing_lambda(&mut self, callee: Expr) -> PResult<Expr> {
        let lambda = self.lambda()?;
        let span = callee.span.to(lambda.span);
        let arg = Arg {
            name: None,
            value: lambda,
        };
        let kind = match callee.kind {
            ExprKind::Call { callee, mut args } => {
                args.push(arg);
                ExprKind::Call { callee, args }
            }
            other => {
                let callee = Expr {
                    kind: other,
                    span: callee.span,
                };
                ExprKind::Call {
                    callee: Box::new(callee),
                    args: vec![arg],
                }
            }
        };
        Ok(Expr { kind, span })
    }

    fn index(&mut self, expr: Expr) -> PResult<Expr> {
        self.bump();
        let index = self.braces(Self::expr)?;
        self.need(Punct::RBracket)?;
        let span = expr.span.to(self.prev());
        Ok(Expr {
            kind: ExprKind::Index {
                expr: Box::new(expr),
                index: Box::new(index),
            },
            span,
        })
    }
}
