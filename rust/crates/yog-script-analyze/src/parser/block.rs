//! Blocks and statements: a word's statement (`for`, `return`, …), else an
//! expression, maybe assigned to.

use super::{PResult, Parser};
use crate::ast::{BinOp, Block, Stmt};
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
        let stmts = self.braces(Self::stmts)?;
        Ok(Block {
            stmts,
            span: start.to(self.prev()),
        })
    }

    /// Statements up to and with the `}` closing them.
    pub(crate) fn stmts(&mut self) -> PResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            self.eat(Punct::Semi);
            if self.eat(Punct::RBrace) {
                return Ok(stmts);
            }
            if *self.tok() == Tok::Eof {
                return Err(self.expected("`}`"));
            }
            match self.stmt().and_then(|s| self.end_stmt().map(|()| s)) {
                Ok(s) => stmts.push(s),
                Err(_) => self.recover(),
            }
        }
    }

    pub(crate) fn stmt(&mut self) -> PResult<Stmt> {
        let word = ALL.iter().find_map(|w| {
            w.role(|r| match *r {
                Role::Stmt { parse } if self.at_word(w) => Some(parse),
                _ => None,
            })
        });
        if let Some(parse) = word {
            return parse(self);
        }
        let expr = self.expr()?;
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
    pub(crate) fn optional_value(&mut self) -> PResult<Option<crate::ast::Expr>> {
        if self.at_stmt_end() {
            Ok(None)
        } else {
            self.expr().map(Some)
        }
    }
}
