//! What follows a type's name: generics, the primary constructor, bases,
//! constraints and the components it has.

use super::{PResult, Parser};
use crate::ast::{Expr, ExprKind, Name, Type, TypeHeader};
use crate::source::token::Punct;
use crate::words::{has, r#where};

impl Parser<'_, '_, '_> {
    /// `<T>(params) : Bases where T : X has C(…), D` — the parts a type of
    /// this kind may have; the others must not be there.
    pub(crate) fn header(&mut self, ctor: bool) -> PResult<TypeHeader> {
        let generics = self.generics()?;
        let ctor = if ctor && self.at(Punct::LParen) {
            Some(self.params()?)
        } else {
            None
        };
        let bases = self.bases()?;
        let constraints = self.constraints()?;
        let has = self.has_list()?;
        Ok(TypeHeader {
            generics,
            ctor,
            bases,
            constraints,
            has,
        })
    }

    /// `: A, B`, or nothing.
    pub(crate) fn bases(&mut self) -> PResult<Vec<Type>> {
        if !self.eat(Punct::Colon) {
            return Ok(Vec::new());
        }
        let mut out = vec![self.ty()?];
        while self.eat(Punct::Comma) {
            out.push(self.ty()?);
        }
        Ok(out)
    }

    /// `where T : A, B where U : C`, or nothing.
    pub(crate) fn constraints(&mut self) -> PResult<Vec<(Name, Vec<Type>)>> {
        let mut out = Vec::new();
        while self.eat_word(&r#where::WORD) {
            let name = self.name("a type parameter")?;
            self.need(Punct::Colon)?;
            let mut bounds = vec![self.ty()?];
            while self.eat(Punct::Comma) {
                bounds.push(self.ty()?);
            }
            out.push((name, bounds));
        }
        Ok(out)
    }

    /// `has Charge(1000), Owned`, or nothing: each a component and its
    /// arguments, as a call (`Charge(1000)`) or a path (`Owned`).
    fn has_list(&mut self) -> PResult<Vec<Expr>> {
        if !self.eat_word(&has::WORD) {
            return Ok(Vec::new());
        }
        let mut out = vec![self.component_use()?];
        while self.eat(Punct::Comma) {
            out.push(self.component_use()?);
        }
        Ok(out)
    }

    fn component_use(&mut self) -> PResult<Expr> {
        let start = self.span();
        let path = self.path("a component")?;
        let mut expr = Self::path_expr(path);
        if self.at(Punct::LParen) {
            let args = self.args()?;
            expr = Expr {
                kind: ExprKind::Call {
                    callee: Box::new(expr),
                    args,
                },
                span: start.to(self.prev()),
            };
        }
        Ok(expr)
    }

    /// `a.b.c` as the expression it reads as.
    pub(crate) fn path_expr(path: Vec<Name>) -> Expr {
        let mut names = path.into_iter();
        let first = names.next().map_or(
            Expr {
                kind: ExprKind::Error,
                span: crate::source::token::Span::default(),
            },
            |n| Expr {
                kind: ExprKind::Ident(n.text),
                span: n.span,
            },
        );
        names.fold(first, |expr, name| {
            let span = expr.span.to(name.span);
            Expr {
                kind: ExprKind::Member {
                    expr: Box::new(expr),
                    name,
                    safe: false,
                },
                span,
            }
        })
    }
}
