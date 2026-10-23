//! Blocks and statements: a word's statement (`if`, `foreach`, `return`, …),
//! a local declared type first, else an expression, maybe assigned to.

use super::{PResult, Parser};
use crate::ast::{BinOp, Binding, Block, Expr, Stmt};
use crate::source::token::{Punct, Tok};
use crate::words::{ALL, Role};

/// Assignments and the operator each applies first (`=` none).
const ASSIGN: &[(Punct, Option<BinOp>)] = &[
    (Punct::Eq, None),
    (Punct::PlusEq, Some(BinOp::Add)),
    (Punct::MinusEq, Some(BinOp::Sub)),
    (Punct::StarEq, Some(BinOp::Mul)),
    (Punct::SlashEq, Some(BinOp::Div)),
    (Punct::PercentEq, Some(BinOp::Rem)),
    (Punct::NullAssign, Some(BinOp::Coalesce)),
];

impl Parser<'_, '_, '_> {
    /// `{ stmts }`.
    pub(crate) fn block(&mut self) -> PResult<Block> {
        let start = self.need(Punct::LBrace)?;
        let stmts = self.braces(|p| p.stmts(false))?;
        Ok(Block {
            stmts,
            span: start.to(self.prev()),
        })
    }

    /// `{ stmts }` whose last may be an expression without `;`: its value
    /// (a patch's argument, a wrapped call).
    pub(crate) fn value_block(&mut self) -> PResult<Block> {
        let start = self.need(Punct::LBrace)?;
        let stmts = self.braces(|p| p.stmts(true))?;
        Ok(Block {
            stmts,
            span: start.to(self.prev()),
        })
    }

    /// Statements up to and with the `}` closing them; in a lambda
    /// (`value`), the last may be an expression without `;`: its value.
    pub(crate) fn stmts(&mut self, value: bool) -> PResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            self.skip_docs();
            if self.eat(Punct::RBrace) {
                return Ok(stmts);
            }
            if *self.tok() == Tok::Eof {
                return Err(self.expected("`}`"));
            }
            match self.stmt_in(value) {
                Ok(s) => stmts.push(s),
                Err(_) => self.recover(),
            }
        }
    }

    /// One statement, with its `;`.
    pub(crate) fn stmt(&mut self) -> PResult<Stmt> {
        self.stmt_in(false)
    }

    fn stmt_in(&mut self, value: bool) -> PResult<Stmt> {
        if self.at(Punct::LBrace) {
            return self.block().map(Stmt::Block);
        }
        let word = ALL.iter().find_map(|w| {
            w.role(|r| match *r {
                Role::Stmt { parse } if self.at_word(w) => Some(parse),
                _ => None,
            })
        });
        if let Some(parse) = word {
            return parse(self);
        }
        if let Some(len) = self.typed_ahead()
            && matches!(self.nth(len + 1), Tok::Punct(Punct::Eq | Punct::Semi))
        {
            let s = self.typed_local()?;
            self.end()?;
            return Ok(s);
        }
        let expr = self.expr()?;
        if value && self.at(Punct::RBrace) {
            return Ok(Stmt::Value(expr));
        }
        let s = self.assign_or(expr)?;
        self.end()?;
        Ok(s)
    }

    /// `T name (= value)?`, without the `;`.
    pub(crate) fn typed_local(&mut self) -> PResult<Stmt> {
        let ty = self.ty()?;
        let name = self.name("a name")?;
        let value = if self.eat(Punct::Eq) {
            Some(self.expr()?)
        } else {
            None
        };
        Ok(Stmt::Local {
            ty: Some(ty),
            binding: Binding::One(name),
            value,
        })
    }

    /// An expression, or an assignment to it when one follows; no `;`.
    pub(crate) fn assign_or(&mut self, expr: Expr) -> PResult<Stmt> {
        let assign = ASSIGN.iter().find(|(p, _)| self.at(*p));
        let Some(&(_, op)) = assign else {
            return Ok(Stmt::Expr(expr));
        };
        self.bump();
        let value = self.expr()?;
        Ok(Stmt::Assign {
            place: expr,
            op,
            value,
        })
    }

    /// A value after `return` / `skip`, unless the statement ends there.
    pub(crate) fn optional_value(&mut self) -> PResult<Option<Expr>> {
        if self.at(Punct::Semi) {
            Ok(None)
        } else {
            self.expr().map(Some)
        }
    }

    /// `(cond)`.
    pub(crate) fn paren_cond(&mut self) -> PResult<Expr> {
        self.need(Punct::LParen)?;
        let cond = self.braces(Self::expr)?;
        self.need(Punct::RParen)?;
        Ok(cond)
    }
}
