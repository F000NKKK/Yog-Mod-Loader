//! `checked` arithmetic in Java (C#'s): inside `checked(…)` or a
//! `checked { … }` block, whole numbers' `+ - *`, `-x`, `op=` and
//! narrowing casts panic on overflow (`dev.yog.script.Checked`) — a
//! narrower type than `i64` computed in `long` and its range checked,
//! `i64`'s by exact operations, `u64`'s unsigned ones. `unchecked` wraps
//! again, as Java does.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Block, Expr};
use yog_script_analyze::token::Span;
use yog_script_ids::jvm::checked::{
    ADD, CLASS, FROM_FLOAT, MUL, NEG, RANGE, SUB, UNSIGNED_ADD, UNSIGNED_AT_MOST, UNSIGNED_FROM_FLOAT,
    UNSIGNED_MUL, UNSIGNED_NEG, UNSIGNED_SUB,
};

use super::Gen;
use crate::types::primitive;

/// Java's `long`, what a narrower number is computed in.
use yog_script_ids::java::LONG_SUFFIX;
const LONG: &str = yog_script_ids::java::LONG;
const DOUBLE: &str = yog_script_ids::java::DOUBLE;

impl Gen<'_> {
    /// `checked(inner)` / `unchecked(inner)`.
    pub(crate) fn checked_expr(&mut self, inner: &Expr, checked: bool) -> String {
        let outer = std::mem::replace(&mut self.checked, checked);
        let v = self.expr(inner);
        self.checked = outer;
        v
    }

    /// `checked { … }` / `unchecked { … }`.
    pub(crate) fn checked_block(&mut self, b: &Block, checked: bool) {
        let outer = std::mem::replace(&mut self.checked, checked);
        self.code.open("");
        self.block_stmts(b);
        self.code.close("");
        self.checked = outer;
    }

    /// `l op r` (of type `t`, at `at`) in a checked context, when it's
    /// whole numbers' `+ - *`.
    pub(crate) fn checked_binary(&self, t: Option<&Type>, op: BinOp, (l, r): (&str, &str), at: Span) -> Option<String> {
        if !self.checked {
            return None;
        }
        let (exact, unsigned) = match op {
            BinOp::Add => (ADD, UNSIGNED_ADD),
            BinOp::Sub => (SUB, UNSIGNED_SUB),
            BinOp::Mul => (MUL, UNSIGNED_MUL),
            _ => return None,
        };
        let t = t?.non_null();
        let at = self.at(at);
        match t {
            Type::I64 => return Some(format!("{CLASS}.{exact}({l}, {r}, {at})")),
            Type::U64 => return Some(format!("{CLASS}.{unsigned}({l}, {r}, {at})")),
            _ => {}
        }
        let (min, max) = range(t)?;
        let java = primitive(t)?;
        let op = crate::exprs::op_text(op);
        Some(format!("(({java}) {CLASS}.{RANGE}(({LONG}) ({l}) {op} ({LONG}) ({r}), {min}{LONG_SUFFIX}, {max}{LONG_SUFFIX}, {at}))"))
    }

    /// `-v` (of type `t`) in a checked context.
    pub(crate) fn checked_neg(&self, t: Option<&Type>, v: &str, at: Span) -> Option<String> {
        if !self.checked {
            return None;
        }
        let t = t?.non_null();
        let at = self.at(at);
        match t {
            Type::I64 => return Some(format!("{CLASS}.{NEG}({v}, {at})")),
            Type::U64 => return Some(format!("{CLASS}.{UNSIGNED_NEG}({v}, {at})")),
            _ => {}
        }
        let (min, max) = range(t)?;
        let java = primitive(t)?;
        Some(format!("(({java}) {CLASS}.{RANGE}(-({LONG}) ({v}), {min}{LONG_SUFFIX}, {max}{LONG_SUFFIX}, {at}))"))
    }

    /// `(to) v` (`v` of type `from`) in a checked context: out of `to`'s
    /// range, a panic.
    pub(crate) fn checked_cast(&self, (from, to): (Option<&Type>, Option<&Type>), v: &str, at: Span) -> Option<String> {
        if !self.checked {
            return None;
        }
        let (from, to) = (from?.non_null(), to?.non_null());
        let at = self.at(at);
        let float = matches!(from, Type::F32 | Type::F64);
        // To a `u64`: from a float, in its range; from a whole number, not below zero.
        if *to == Type::U64 {
            return Some(if float {
                format!("{CLASS}.{UNSIGNED_FROM_FLOAT}(({DOUBLE}) ({v}), {at})")
            } else {
                format!("{CLASS}.{RANGE}(({LONG}) ({v}), 0{LONG_SUFFIX}, {}{LONG_SUFFIX}, {at})", i64::MAX)
            });
        }
        let (min, max) = match to {
            Type::I64 => (i64::MIN, i64::MAX),
            other => range(other)?,
        };
        let java = primitive(to)?;
        // From a `u64`: no more than the type's top (unsigned).
        if *from == Type::U64 {
            return Some(format!("(({java}) {CLASS}.{UNSIGNED_AT_MOST}({v}, {max}{LONG_SUFFIX}, {at}))"));
        }
        let read = if float {
            format!("{CLASS}.{FROM_FLOAT}(({DOUBLE}) ({v}), {min}{LONG_SUFFIX}, {max}{LONG_SUFFIX}, {at})")
        } else if range(from).is_some() || *from == Type::I64 {
            format!("{CLASS}.{RANGE}(({LONG}) ({v}), {min}{LONG_SUFFIX}, {max}{LONG_SUFFIX}, {at})")
        } else {
            return None;
        };
        Some(format!("(({java}) {read})"))
    }
}

/// A whole-number type narrower than `i64`'s range, as `long`s.
fn range(t: &Type) -> Option<(i64, i64)> {
    Some(match t {
        Type::I8 => (i8::MIN.into(), i8::MAX.into()),
        Type::I16 => (i16::MIN.into(), i16::MAX.into()),
        Type::I32 => (i32::MIN.into(), i32::MAX.into()),
        Type::U8 => (u8::MIN.into(), u8::MAX.into()),
        Type::U16 => (u16::MIN.into(), u16::MAX.into()),
        Type::U32 => (u32::MIN.into(), u32::MAX.into()),
        _ => return None,
    })
}
