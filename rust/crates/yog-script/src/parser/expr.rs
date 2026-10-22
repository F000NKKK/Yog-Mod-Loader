//! Expressions by precedence (`ys-grammar.md` §3.2). Operators written
//! with signs are here; operators that are words (`and`, `is`, `as`, …)
//! parse in their own files and are offered their level here.

use super::{PResult, Parser};
use crate::ast::{BinOp, Expr, ExprKind, UnOp};
use crate::token::{Punct, Tok};
use crate::words::{ALL, Level, Role};

/// An infix operator met here.
enum Infix {
    Sign {
        op: BinOp,
        level: Level,
    },
    Range {
        inclusive: bool,
    },
    Word {
        level: Level,
        parse: fn(&mut Parser, Expr, bool) -> PResult<Expr>,
        negated: bool,
    },
}

impl Infix {
    fn level(&self) -> Level {
        match self {
            Infix::Sign { level, .. } | Infix::Word { level, .. } => *level,
            Infix::Range { .. } => Level::Range,
        }
    }
}

const SIGNS: &[(Punct, BinOp, Level)] = &[
    (Punct::Coalesce, BinOp::Coalesce, Level::Coalesce),
    (Punct::OrOr, BinOp::Or, Level::Or),
    (Punct::AndAnd, BinOp::And, Level::And),
    (Punct::EqEq, BinOp::Eq, Level::Compare),
    (Punct::NotEq, BinOp::NotEq, Level::Compare),
    (Punct::Lt, BinOp::Lt, Level::Compare),
    (Punct::LtEq, BinOp::LtEq, Level::Compare),
    (Punct::Gt, BinOp::Gt, Level::Compare),
    (Punct::GtEq, BinOp::GtEq, Level::Compare),
    (Punct::Plus, BinOp::Add, Level::Sum),
    (Punct::Minus, BinOp::Sub, Level::Sum),
    (Punct::Star, BinOp::Mul, Level::Product),
    (Punct::Slash, BinOp::Div, Level::Product),
    (Punct::Percent, BinOp::Rem, Level::Product),
];

impl Level {
    /// The next tighter level (the right side of a left-associative one).
    pub(crate) fn tighter(self) -> Level {
        match self {
            Level::Coalesce => Level::Or,
            Level::Or => Level::And,
            Level::And => Level::Compare,
            Level::Compare => Level::Range,
            Level::Range => Level::Sum,
            Level::Sum => Level::Product,
            Level::Product | Level::Convert => Level::Convert,
        }
    }
}

impl Parser<'_, '_, '_> {
    pub(crate) fn expr(&mut self) -> PResult<Expr> {
        self.binary(Level::Coalesce)
    }

    /// An expression where `{` ends it (a condition, an iterated value).
    pub(crate) fn expr_no_brace(&mut self) -> PResult<Expr> {
        self.no_brace(Self::expr)
    }

    /// Operators of level `min` and tighter.
    pub(crate) fn binary(&mut self, min: Level) -> PResult<Expr> {
        let mut left = self.unary()?;
        while let Some(infix) = self.infix_here() {
            if infix.level() < min {
                break;
            }
            left = self.infix(left, infix)?;
        }
        Ok(left)
    }

    fn infix(&mut self, left: Expr, infix: Infix) -> PResult<Expr> {
        match infix {
            Infix::Word { parse, negated, .. } => {
                if negated {
                    self.bump();
                }
                parse(self, left, negated)
            }
            Infix::Range { inclusive } => {
                self.bump();
                let right = self.binary(Level::Sum)?;
                let span = left.span.to(right.span);
                let kind = ExprKind::Range {
                    from: Box::new(left),
                    to: Box::new(right),
                    inclusive,
                };
                Ok(Expr { kind, span })
            }
            Infix::Sign { op, level } => {
                self.bump();
                // `??` groups to the right; comparisons don't chain.
                let right_level = if level == Level::Coalesce {
                    level
                } else {
                    level.tighter()
                };
                let right = self.binary(right_level)?;
                Ok(Self::binary_node(op, left, right))
            }
        }
    }

    pub(crate) fn binary_node(op: BinOp, left: Expr, right: Expr) -> Expr {
        let span = left.span.to(right.span);
        Expr {
            kind: ExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        }
    }

    fn infix_here(&self) -> Option<Infix> {
        if let Tok::Punct(p) = self.tok() {
            match p {
                Punct::Range => return Some(Infix::Range { inclusive: true }),
                Punct::RangeExcl => return Some(Infix::Range { inclusive: false }),
                Punct::Bang => return self.word_infix(1, true),
                _ => {}
            }
            return SIGNS
                .iter()
                .find(|(s, ..)| s == p)
                .map(|&(_, op, level)| Infix::Sign { op, level });
        }
        self.word_infix(0, false)
    }

    /// An infix word `n` ahead (after `!` when `negated`).
    fn word_infix(&self, n: usize, negated: bool) -> Option<Infix> {
        ALL.iter().filter(|w| self.nth_word(n, w)).find_map(|w| {
            w.role(|r| match *r {
                Role::Infix {
                    level,
                    parse,
                    negatable,
                } if negatable || !negated => Some(Infix::Word {
                    level,
                    parse,
                    negated,
                }),
                _ => None,
            })
        })
    }

    /// `-x`, `!x`, a word's prefix form, or a primary with what follows it.
    pub(crate) fn unary(&mut self) -> PResult<Expr> {
        let start = self.span();
        let op = match self.tok() {
            Tok::Punct(Punct::Minus) => Some(UnOp::Neg),
            Tok::Punct(Punct::Bang) => Some(UnOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let expr = self.unary()?;
            let span = start.to(expr.span);
            return Ok(Expr {
                kind: ExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                },
                span,
            });
        }
        let expr = match self.prefix_word() {
            Some(parse) => parse(self)?,
            None => self.primary()?,
        };
        self.postfix(expr)
    }

    fn prefix_word(&self) -> Option<fn(&mut Parser) -> PResult<Expr>> {
        ALL.iter().find_map(|w| {
            w.role(|r| match *r {
                Role::Prefix { starts, parse } if self.starts(w, starts) => Some(parse),
                _ => None,
            })
        })
    }
}
