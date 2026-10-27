//! Expressions by precedence (`ys-grammar.md` §3.2). Operators written
//! with signs are here; operators that are words (`is`, `as`, `switch`, …)
//! parse in their own files and are offered their level here.

use super::{PResult, Parser};
use crate::ast::{BinOp, Expr, ExprKind, UnOp};
use crate::source::token::{Punct, Tok};
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
    /// `>>`: two `>` side by side (one token each, for `List<List<T>>`).
    ShiftRight,
    Word {
        level: Level,
        parse: fn(&mut Parser, Expr) -> PResult<Expr>,
    },
}

impl Infix {
    fn level(&self) -> Level {
        match self {
            Infix::Sign { level, .. } | Infix::Word { level, .. } => *level,
            Infix::Range { .. } => Level::Range,
            Infix::ShiftRight => Level::Shift,
        }
    }
}

const SIGNS: &[(Punct, BinOp, Level)] = &[
    (Punct::Coalesce, BinOp::Coalesce, Level::Coalesce),
    (Punct::OrOr, BinOp::Or, Level::Or),
    (Punct::AndAnd, BinOp::And, Level::And),
    (Punct::Pipe, BinOp::BitOr, Level::BitOr),
    (Punct::Caret, BinOp::BitXor, Level::BitXor),
    (Punct::Amp, BinOp::BitAnd, Level::BitAnd),
    (Punct::EqEq, BinOp::Eq, Level::Equality),
    (Punct::NotEq, BinOp::NotEq, Level::Equality),
    (Punct::Lt, BinOp::Lt, Level::Compare),
    (Punct::LtEq, BinOp::LtEq, Level::Compare),
    (Punct::Gt, BinOp::Gt, Level::Compare),
    (Punct::GtEq, BinOp::GtEq, Level::Compare),
    (Punct::Shl, BinOp::Shl, Level::Shift),
    (Punct::Plus, BinOp::Add, Level::Sum),
    (Punct::Minus, BinOp::Sub, Level::Sum),
    (Punct::Star, BinOp::Mul, Level::Product),
    (Punct::Slash, BinOp::Div, Level::Product),
    (Punct::Percent, BinOp::Rem, Level::Product),
];

const PREFIXES: &[(Punct, UnOp)] = &[
    (Punct::Minus, UnOp::Neg),
    (Punct::Bang, UnOp::Not),
    (Punct::Tilde, UnOp::BitNot),
    (Punct::Caret, UnOp::FromEnd),
    (Punct::PlusPlus, UnOp::Increment),
    (Punct::MinusMinus, UnOp::Decrement),
];

/// The postfix steps: `x++`, `x--`.
pub(crate) const POSTFIXES: &[(Punct, UnOp)] = &[
    (Punct::PlusPlus, UnOp::PostIncrement),
    (Punct::MinusMinus, UnOp::PostDecrement),
];

/// How `>>` is written: it's two tokens.
const SHIFT_RIGHT: &str = ">>";

/// The binary operator sign `p` is.
pub(crate) fn binary_of(p: Punct) -> Option<BinOp> {
    SIGNS.iter().find(|(s, ..)| *s == p).map(|(_, op, _)| *op)
}

/// How binary operator `op` is written.
pub(crate) fn sign_of(op: BinOp) -> &'static str {
    if op == BinOp::Shr {
        return SHIFT_RIGHT;
    }
    SIGNS
        .iter()
        .find(|(_, o, _)| *o == op)
        .map_or("", |(p, ..)| p.text())
}

/// The prefix operator sign `p` is.
pub(crate) fn prefix_of(p: Punct) -> Option<UnOp> {
    PREFIXES.iter().find(|(s, _)| *s == p).map(|(_, op)| *op)
}

impl Level {
    /// The next tighter level (the right side of a left-associative one).
    pub(crate) fn tighter(self) -> Level {
        match self {
            Level::Coalesce => Level::Or,
            Level::Or => Level::And,
            Level::And => Level::BitOr,
            Level::BitOr => Level::BitXor,
            Level::BitXor => Level::BitAnd,
            Level::BitAnd => Level::Equality,
            Level::Equality => Level::Compare,
            Level::Compare => Level::Range,
            Level::Range => Level::Shift,
            Level::Shift => Level::Sum,
            Level::Sum => Level::Product,
            Level::Product | Level::Switch => Level::Switch,
        }
    }
}

impl Parser<'_, '_, '_> {
    /// An expression: a lambda, or `c ? a : b` and tighter.
    pub(crate) fn expr(&mut self) -> PResult<Expr> {
        if self.lambda_ahead() {
            return self.arrow_lambda();
        }
        // `async x => …`: a lambda that awaits.
        if self.at_word(&crate::words::r#async::WORD) && self.lambda_ahead_at(1) {
            let start = self.span();
            self.bump();
            let mut lambda = self.arrow_lambda()?;
            if let ExprKind::Lambda { asynchronous, .. } = &mut lambda.kind {
                *asynchronous = true;
            }
            lambda.span = start.to(lambda.span);
            return Ok(lambda);
        }
        self.conditional()
    }

    /// `c ? a : b`, or what's tighter.
    pub(crate) fn conditional(&mut self) -> PResult<Expr> {
        let cond = self.binary(Level::Coalesce)?;
        if !self.eat(Punct::Question) {
            return Ok(cond);
        }
        let then = self.expr()?;
        self.need(Punct::Colon)?;
        let otherwise = self.expr()?;
        let span = cond.span.to(otherwise.span);
        Ok(Expr {
            kind: ExprKind::Conditional {
                cond: Box::new(cond),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            },
            span,
        })
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
            Infix::Word { parse, .. } => parse(self, left),
            Infix::Range { inclusive } => {
                self.bump();
                let right = self.binary(Level::Shift)?;
                let span = left.span.to(right.span);
                let kind = ExprKind::Range {
                    from: Box::new(left),
                    to: Box::new(right),
                    inclusive,
                };
                Ok(Expr { kind, span })
            }
            Infix::ShiftRight => {
                self.bump();
                self.bump();
                let right = self.binary(Level::Shift.tighter())?;
                Ok(Self::binary_node(BinOp::Shr, left, right))
            }
            Infix::Sign { op, level } => {
                self.bump();
                // `??` groups to the right.
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
                Punct::Gt if self.joined(Punct::Gt) => return Some(Infix::ShiftRight),
                // `>>=`: an assignment, the statement's.
                Punct::Gt if self.joined(Punct::GtEq) => return None,
                _ => {}
            }
            return SIGNS
                .iter()
                .find(|(s, ..)| s == p)
                .map(|&(_, op, level)| Infix::Sign { op, level });
        }
        ALL.iter().filter(|w| self.at_word(w)).find_map(|w| {
            w.role(|r| match *r {
                Role::Infix { level, parse } => Some(Infix::Word { level, parse }),
                _ => None,
            })
        })
    }

    /// `-x`, `!x`, `(T)x`, a word's prefix form, or a primary with what
    /// follows it.
    pub(crate) fn unary(&mut self) -> PResult<Expr> {
        let start = self.span();
        if let Some(&(p, op)) = PREFIXES.iter().find(|(p, _)| self.at(*p)) {
            self.need(p)?;
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
        if self.cast_ahead() {
            self.bump();
            let ty = self.ty()?;
            self.need(Punct::RParen)?;
            let expr = self.unary()?;
            let span = start.to(expr.span);
            return Ok(Expr {
                kind: ExprKind::Cast {
                    ty,
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

    /// `(T)` before what starts an operand: a cast, not brackets.
    fn cast_ahead(&self) -> bool {
        if !self.at(Punct::LParen) {
            return false;
        }
        let Some(len) = self.type_len(1) else {
            return false;
        };
        if !self.nth_is(1 + len, Punct::RParen) {
            return false;
        }
        match self.nth(len + 2) {
            Tok::Ident(_) | Tok::Int(_) | Tok::Float(_) | Tok::Unit(..) | Tok::Str(_) => true,
            Tok::Punct(Punct::LParen) => true,
            Tok::Keyword(w) => w
                .role(|r| matches!(r, Role::Prefix { .. }).then_some(()))
                .is_some(),
            _ => false,
        }
    }

    fn prefix_word(&self) -> Option<fn(&mut Parser) -> PResult<Expr>> {
        ALL.iter().find_map(|w| {
            w.role(|r| match *r {
                Role::Prefix { starts, parse } if self.starts(w, starts) => Some(parse),
                _ => None,
            })
        })
    }

    /// A prefix operator's operand: `await x`, `try f()`.
    pub(crate) fn prefixed(&mut self, op: UnOp) -> PResult<Expr> {
        let start = self.bump().span;
        let expr = self.unary()?;
        let span = start.to(expr.span);
        Ok(Expr {
            kind: ExprKind::Unary {
                op,
                expr: Box::new(expr),
            },
            span,
        })
    }
}
