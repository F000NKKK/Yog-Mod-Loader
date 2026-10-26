//! Primary expressions: literals, names, brackets and tuples, collections.

use super::{PResult, Parser};
use crate::ast::{Arg, Element, Expr, ExprKind, Name, StrSeg};
use crate::source::token::{Punct, Span, StrPart, Tok, Token};

impl Parser<'_, '_, '_> {
    pub(crate) fn primary(&mut self) -> PResult<Expr> {
        let span = self.span();
        let kind = match self.tok().clone() {
            Tok::Int(v) => ExprKind::Int(v),
            Tok::Float(v) => ExprKind::Float(v),
            Tok::Unit(value, unit) => ExprKind::Unit { value, unit },
            Tok::RawStr(s) => ExprKind::RawStr(s),
            Tok::Char(c) => ExprKind::Char(c),
            Tok::Text(key) => ExprKind::Text(key),
            Tok::Content(kind, id) => ExprKind::Content { kind, id },
            Tok::Tag(id) => ExprKind::Tag(id),
            Tok::Ident(_) if self.macro_ahead() => return self.macro_call(),
            Tok::Ident(name) => ExprKind::Ident(name),
            // `i32.Parse(…)`, `char.IsDigit(c)`: a built-in type's statics, C#'s `int.Parse`.
            Tok::Keyword(w) if w.is_type() && self.nth_is(1, Punct::Dot) => {
                ExprKind::Ident(w.text.to_string())
            }
            Tok::Str(parts) => {
                self.bump();
                return Ok(Expr {
                    kind: ExprKind::Str(self.segments(&parts, span)),
                    span,
                });
            }
            Tok::Punct(Punct::LParen) => return self.braces(Self::paren),
            Tok::Punct(Punct::LBracket) => return self.braces(Self::collection),
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
                StrPart::Expr {
                    tokens,
                    alignment,
                    format,
                } => {
                    let value = self.interpolated(tokens, span);
                    StrSeg::Expr(self.formatted(value, (alignment.as_deref(), format.as_deref())))
                }
            })
            .collect()
    }

    /// A hole's value, formatted and padded as written: `{x,5:F2}` is
    /// `x.ToString("F2").PadLeft(5)` (a negative alignment `PadRight`).
    fn formatted(
        &mut self,
        value: Expr,
        (alignment, format): (Option<&str>, Option<&str>),
    ) -> Expr {
        use yog_script_ids::members::{PAD_LEFT, PAD_RIGHT, TO_STRING};
        let span = value.span;
        let text = |s: &str| Expr {
            kind: ExprKind::Str(vec![StrSeg::Text(s.to_string())]),
            span,
        };
        let call = |on: Expr, name: &str, args: Vec<Expr>| Expr {
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    kind: ExprKind::Member {
                        expr: Box::new(on),
                        name: Name {
                            text: name.to_string(),
                            span,
                        },
                        safe: false,
                    },
                    span,
                }),
                args: args
                    .into_iter()
                    .map(|value| Arg {
                        name: None,
                        pass: None,
                        value,
                    })
                    .collect(),
            },
            span,
        };
        let mut out = match format {
            Some(f) => call(value, TO_STRING, vec![text(f)]),
            None if alignment.is_some() => call(value, TO_STRING, Vec::new()),
            None => value,
        };
        if let Some(a) = alignment {
            let Ok(width) = a.parse::<i64>() else {
                self.report.error(
                    yog_ids::script_problem::EXPECTED,
                    span,
                    format!("a hole's alignment is a whole number, not `{a}`"),
                );
                return out;
            };
            let (pad, by) = if width < 0 {
                (PAD_RIGHT, -width)
            } else {
                (PAD_LEFT, width)
            };
            out = call(
                out,
                pad,
                vec![Expr {
                    kind: ExprKind::Int(by),
                    span,
                }],
            );
        }
        out
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

    /// `(e)`, `(a, b)`, `(X: 1, Y: 2)`.
    fn paren(&mut self) -> PResult<Expr> {
        let start = self.need(Punct::LParen)?;
        let (first_name, mut first) = self.tuple_item()?;
        if first_name.is_none() && self.eat(Punct::RParen) {
            first.span = start.to(self.prev());
            return Ok(first);
        }
        self.need(Punct::Comma)?;
        let rest = self.list(Punct::RParen, Self::tuple_item)?;
        let (names, items) = std::iter::once((first_name, first)).chain(rest).unzip();
        Ok(Expr {
            kind: ExprKind::Tuple { items, names },
            span: start.to(self.prev()),
        })
    }

    /// A tuple's item, `Name:` before it when it's named.
    fn tuple_item(&mut self) -> PResult<(Option<Name>, Expr)> {
        let named = matches!(self.tok(), Tok::Ident(_)) && self.nth_is(1, Punct::Colon);
        let name = if named {
            let name = self.name("an item's name")?;
            self.bump();
            Some(name)
        } else {
            None
        };
        Ok((name, self.expr()?))
    }

    /// `[a, b]`, `[k => v]`, `[..xs]`.
    fn collection(&mut self) -> PResult<Expr> {
        let start = self.need(Punct::LBracket)?;
        let items = self.list(Punct::RBracket, |p| {
            if p.eat(Punct::Range) {
                return Ok(Element::Spread(p.conditional()?));
            }
            let item = p.conditional()?;
            if p.eat(Punct::FatArrow) {
                return Ok(Element::Entry(item, p.expr()?));
            }
            Ok(Element::Item(item))
        })?;
        Ok(Expr {
            kind: ExprKind::Collection(items),
            span: start.to(self.prev()),
        })
    }
}

impl Parser<'_, '_, '_> {
    /// `name!(` with nothing between: a macro's call (`x!` alone asserts
    /// `x` isn't null).
    fn macro_ahead(&self) -> bool {
        let [name, bang, open] = [0, 1, 2].map(|n| self.span_at(n));
        self.nth_is(1, Punct::Bang)
            && self.nth_is(2, Punct::LParen)
            && name.end == bang.start
            && bang.end == open.start
    }

    /// `name!(a, b)`.
    fn macro_call(&mut self) -> PResult<Expr> {
        let name = self.name("a macro")?;
        self.need(Punct::Bang)?;
        self.need(Punct::LParen)?;
        let args = self.braces(|p| p.list(Punct::RParen, Self::expr))?;
        let span = name.span.to(self.prev());
        Ok(Expr {
            kind: ExprKind::Macro { name, args },
            span,
        })
    }
}
