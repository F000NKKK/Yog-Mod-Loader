//! Lambdas: `x => …`, `(a, b) => …`, `(int a) => …`, and the block after a
//! call (`list.Select { it * 2 }`), whose parameter is `it`.

use super::{PResult, Parser};
use crate::ast::{Expr, ExprKind, LambdaBody, LambdaParam};
use crate::source::token::{Punct, Tok};

impl Parser<'_, '_, '_> {
    /// Whether `name =>` or `( … ) =>` starts here.
    pub(super) fn lambda_ahead(&self) -> bool {
        match self.tok() {
            Tok::Ident(_) => self.nth_is(1, Punct::FatArrow),
            Tok::Punct(Punct::LParen) => {
                let mut depth = 0usize;
                let mut n = 0;
                loop {
                    match self.nth(n) {
                        Tok::Punct(Punct::LParen) => depth += 1,
                        Tok::Punct(Punct::RParen) => {
                            depth -= 1;
                            if depth == 0 {
                                return self.nth_is(n + 1, Punct::FatArrow);
                            }
                        }
                        Tok::Eof | Tok::Punct(Punct::Semi | Punct::LBrace | Punct::RBrace) => {
                            return false;
                        }
                        _ => {}
                    }
                    n += 1;
                }
            }
            _ => false,
        }
    }

    /// `x => body` or `(params) => body`.
    pub(super) fn arrow_lambda(&mut self) -> PResult<Expr> {
        let start = self.span();
        let params = if self.at(Punct::LParen) {
            self.bump();
            self.list(Punct::RParen, Self::lambda_param)?
        } else {
            vec![LambdaParam {
                ty: None,
                name: self.name("a parameter")?,
            }]
        };
        self.need(Punct::FatArrow)?;
        let body = if self.at(Punct::LBrace) {
            self.need(Punct::LBrace)?;
            LambdaBody::Block(self.braces(|p| p.stmts(true))?)
        } else {
            LambdaBody::Expr(Box::new(self.expr()?))
        };
        Ok(Expr {
            kind: ExprKind::Lambda { params, body },
            span: start.to(self.prev()),
        })
    }

    /// `name` or `Type name`.
    fn lambda_param(&mut self) -> PResult<LambdaParam> {
        if matches!(self.tok(), Tok::Ident(_))
            && matches!(self.nth(1), Tok::Punct(Punct::Comma | Punct::RParen))
        {
            return Ok(LambdaParam {
                ty: None,
                name: self.name("a parameter")?,
            });
        }
        let ty = Some(self.ty()?);
        Ok(LambdaParam {
            ty,
            name: self.name("a parameter")?,
        })
    }

    /// `{ stmts }` after a call: a lambda of `it`, its last expression its value.
    pub(crate) fn block_lambda(&mut self) -> PResult<Expr> {
        let start = self.need(Punct::LBrace)?;
        let body = self.braces(|p| p.stmts(true))?;
        Ok(Expr {
            kind: ExprKind::Lambda {
                params: Vec::new(),
                body: LambdaBody::Block(body),
            },
            span: start.to(self.prev()),
        })
    }
}
