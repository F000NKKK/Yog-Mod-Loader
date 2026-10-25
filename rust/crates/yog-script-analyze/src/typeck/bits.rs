//! Bits and steps, as C#'s: `&` `|` `^` on whole numbers (their bits) or
//! on two `bool`s (both sides evaluated), `<<` `>>` a whole number shifted
//! by an `i32`'s count, `~` its bits flipped; `++` / `--` a number's place
//! stepped. A number narrower than `i32` comes out an `i32`.

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use crate::ast::BinOp;
use crate::parser::expr::sign_of;
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// `a op b` for `&` `|` `^` `<<` `>>`: its type, or a report.
    pub(super) fn bitwise(&mut self, op: BinOp, (a, b): (&Type, &Type), at: Span) -> Type {
        if !a.known() || !b.known() {
            return Type::Unknown;
        }
        let fits = match op {
            BinOp::Shl | BinOp::Shr => {
                (a.is_integer() && b.is_integer() && b.converts_to(&Type::I32)).then(|| promoted(a))
            }
            _ if *a == Type::Bool && *b == Type::Bool => Some(Type::Bool),
            _ if a.is_integer() && b.is_integer() => Some(promoted(&Type::wider(a, b))),
            _ => None,
        };
        fits.unwrap_or_else(|| {
            let sign = sign_of(op);
            self.mismatch(at, format!("`{a} {sign} {b}`: bits are whole numbers' (a shift's count an `i32`), `&` `|` `^` two `bool`s' too"));
            Type::Unknown
        })
    }

    /// `~x`: its type, or a report.
    pub(super) fn flipped(&mut self, t: &Type, at: Span) -> Type {
        if !t.known() || t.is_integer() {
            return promoted(t);
        }
        self.mismatch(
            at,
            format!("`~` flips a whole number's bits, not a `{t}`'s"),
        );
        Type::Unknown
    }

    /// `++x`, `x--`: its type, or a report.
    pub(super) fn stepped(&mut self, t: &Type, at: Span) -> Type {
        if !t.known() || t.is_numeric() {
            return t.clone();
        }
        self.mismatch(at, format!("`++` and `--` step a number, not a `{t}`"));
        Type::Unknown
    }

    fn mismatch(&mut self, at: Span, message: String) {
        self.report
            .error(script_problem::TYPE_MISMATCH, at, message);
    }
}

/// `t` as arithmetic takes it: a number narrower than `i32` as an `i32`.
fn promoted(t: &Type) -> Type {
    match t {
        Type::I8 | Type::I16 | Type::U8 | Type::U16 | Type::Char => Type::I32,
        _ => t.clone(),
    }
}
