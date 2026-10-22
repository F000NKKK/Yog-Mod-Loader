//! Primary expressions: literals, names, brackets, lists, maps, lambdas.

use super::{PResult, Parser};
use crate::ast::{Expr, ExprKind, Name, StrSeg};
use crate::token::{Punct, Span, StrPart, Tok, Token};

impl Parser<'_, '_, '_> {
    pub(crate) fn primary(&mut self) -> PResult<Expr> {
        let span = self.span();
        let kind = match self.tok().clone() {
            Tok::Int(v) => ExprKind::Int(v),
            Tok::Float(v) => ExprKind::Float(v),
            Tok::Unit(value, unit) => ExprKind::Unit { value, unit },
            Tok::RawStr(s) => ExprKind::RawStr(s),
            Tok::Text(key) => ExprKind::Text(key),
            Tok::Content(kind, id) => ExprKind::Content { kind, id },
            Tok::Tag(id) => ExprKind::Tag(id),
            Tok::Ident(name) => ExprKind::Ident(name),
            Tok::Str(parts) => {
                self.bump();
                return Ok(Expr {
                    kind: ExprKind::Str(self.segments(&parts, span)),
                    span,
                });
            }
            Tok::Punct(Punct::LParen) => return self.braces(Self::paren),
            Tok::Punct(Punct::LBracket) => return self.braces(Self::list_expr),
            Tok::Punct(Punct::LBrace) => return self.braces(Self::map_or_lambda),
            _ => return Err(self.expected("an expression")),
        };
        self.bump();
        Ok(Expr { kind, span })
    }

    /// A string's parts, each `{expr}` parsed.
    fn segments(&mut self, parts: &[StrPart], span: Span) -> Vec<StrSeg> {
        parts
            .iter()
            .map(|part| match part {
                StrPart::Text(t) => StrSeg::Text(t.clone()),
                StrPart::Expr(toks) => StrSeg::Expr(self.interpolated(toks, span)),
            })
            .collect()
    }

    fn interpolated(&mut self, toks: &[Token], span: Span) -> Expr {
        let mut toks = toks.to_vec();
        let end = toks.last().map_or(span, |t| t.span);
        toks.push(Token {
            tok: Tok::Eof,
            span: Span {
                start: end.end,
                end: end.end,
            },
        });
        let mut sub = Parser::new(&toks, &mut *self.report, self.nanites);
        let expr = sub.expr().and_then(|e| {
            if *sub.tok() == Tok::Eof {
                Ok(e)
            } else {
                Err(sub.expected("the end of `{…}`"))
            }
        });
        expr.unwrap_or(Expr {
            kind: ExprKind::Error,
            span,
        })
    }

    /// `()`, `(e)`, `(a, b)`.
    fn paren(&mut self) -> PResult<Expr> {
        let start = self.need(Punct::LParen)?;
        self.skip_newlines();
        if self.eat(Punct::RParen) {
            return Ok(Expr {
                kind: ExprKind::Tuple(Vec::new()),
                span: start.to(self.prev()),
            });
        }
        let mut first = self.expr()?;
        self.skip_newlines();
        if self.eat(Punct::RParen) {
            first.span = start.to(self.prev());
            return Ok(first);
        }
        self.need(Punct::Comma)?;
        let mut items = vec![first];
        items.extend(self.list(Punct::RParen, Self::expr)?);
        Ok(Expr {
            kind: ExprKind::Tuple(items),
            span: start.to(self.prev()),
        })
    }

    fn list_expr(&mut self) -> PResult<Expr> {
        let start = self.need(Punct::LBracket)?;
        let items = self.list(Punct::RBracket, Self::expr)?;
        Ok(Expr {
            kind: ExprKind::List(items),
            span: start.to(self.prev()),
        })
    }

    /// `{ k: v, … }` (a `:` in the first entry), `{}`, or a lambda.
    fn map_or_lambda(&mut self) -> PResult<Expr> {
        if !self.map_ahead() {
            return self.lambda();
        }
        let start = self.need(Punct::LBrace)?;
        let entries = self.list(Punct::RBrace, |p| {
            let key = p.expr()?;
            p.need(Punct::Colon)?;
            Ok((key, p.expr()?))
        })?;
        Ok(Expr {
            kind: ExprKind::Map(entries),
            span: start.to(self.prev()),
        })
    }

    fn map_ahead(&self) -> bool {
        let mut n = 1;
        while *self.nth(n) == Tok::Newline {
            n += 1;
        }
        if *self.nth(n) == Tok::Punct(Punct::RBrace) {
            return true;
        }
        if matches!(self.nth(n), Tok::Keyword(_)) {
            return false;
        }
        let mut depth = 0usize;
        loop {
            match self.nth(n) {
                Tok::Punct(Punct::LParen | Punct::LBracket | Punct::LBrace) => depth += 1,
                Tok::Punct(Punct::RParen | Punct::RBracket) => depth = depth.saturating_sub(1),
                Tok::Punct(Punct::RBrace) if depth == 0 => return false,
                Tok::Punct(Punct::RBrace) => depth -= 1,
                Tok::Punct(Punct::Colon) if depth == 0 => return true,
                Tok::Punct(Punct::Comma) | Tok::Newline if depth == 0 => return false,
                Tok::Eof => return false,
                _ => {}
            }
            n += 1;
        }
    }

    /// `{ a, b -> stmts }` or `{ stmts }` (its value: `it`).
    pub(crate) fn lambda(&mut self) -> PResult<Expr> {
        let start = self.need(Punct::LBrace)?;
        let params = if self.lambda_params_ahead() {
            self.lambda_params()?
        } else {
            Vec::new()
        };
        let body = self.braces(Self::stmts)?;
        Ok(Expr {
            kind: ExprKind::Lambda { params, body },
            span: start.to(self.prev()),
        })
    }

    fn lambda_params_ahead(&self) -> bool {
        let mut n = 0;
        loop {
            if !matches!(self.nth(n), Tok::Ident(_)) {
                return false;
            }
            match self.nth(n + 1) {
                Tok::Punct(Punct::Arrow) => return true,
                Tok::Punct(Punct::Comma) => n += 2,
                _ => return false,
            }
        }
    }

    fn lambda_params(&mut self) -> PResult<Vec<Name>> {
        let mut params = vec![self.name("a parameter")?];
        while self.eat(Punct::Comma) {
            params.push(self.name("a parameter")?);
        }
        self.need(Punct::Arrow)?;
        Ok(params)
    }
}
