//! Expressions' types, operators, and whether a value fits where a type
//! is wanted.

use yog_ids::script_problem;

use super::Checker;
use super::model::{BLOCKS, SECONDS, TICKS, Type};
use crate::ast::{BinOp, Element, Expr, ExprKind, StrSeg, UnOp};
use crate::source::token::Span;

/// Unit suffixes the game has: ticks, seconds (and its multiples), blocks.
const TICK_SUFFIX: &str = "t";
const SECOND_SUFFIXES: [&str; 4] = ["s", "ms", "m", "h"];
const BLOCK_SUFFIX: &str = "b";

impl Checker<'_, '_, '_> {
    pub(super) fn expr(&mut self, e: &Expr) -> Type {
        if let Some(t) = literal(&e.kind) {
            return t;
        }
        match &e.kind {
            ExprKind::Str(segs) => {
                for s in segs {
                    if let StrSeg::Expr(e) = s {
                        self.expr(e);
                    }
                }
                Type::String
            }
            ExprKind::Bool(_) | ExprKind::Mc(_) | ExprKind::Is { .. } | ExprKind::Has { .. } => {
                self.walk_inner(e);
                Type::Bool
            }
            ExprKind::Ident(name) => self.name(name, e.span),
            ExprKind::This => self.this.clone().unwrap_or(Type::Unknown),
            ExprKind::Base => self.base(),
            ExprKind::Declare { ty, name, mutable } => {
                self.declare(ty.as_ref(), &name.text, *mutable)
            }
            ExprKind::NameOf(expr) => self.name_of(expr),
            ExprKind::Move(expr) => self.expr(expr),
            ExprKind::Default(Some(ty)) => self.type_of(ty),
            ExprKind::Member { expr, name, safe } => {
                let on = self.expr(expr);
                self.member(&on, &name.text, *safe, name.span)
            }
            ExprKind::Call { callee, args } => self.call(callee, args),
            ExprKind::Switch { expr, arms } => self.switch(expr, arms, e.span),
            ExprKind::Unary { op, expr } => self.unary(*op, expr),
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right, e.span),
            ExprKind::Conditional {
                cond,
                then,
                otherwise,
            } => {
                self.condition(cond);
                let t = self.expr(then);
                self.expr(otherwise);
                t
            }
            ExprKind::Cast { ty, expr } => {
                self.expr(expr);
                self.type_of(ty)
            }
            ExprKind::As { expr, ty } => {
                self.expr(expr);
                self.type_of(ty).nullable()
            }
            ExprKind::New { ty, args, init } => {
                args.iter().flatten().for_each(|a| {
                    self.expr(&a.value);
                });
                init.iter().for_each(|(_, v)| {
                    self.expr(v);
                });
                let t = ty.as_ref().map_or(Type::Unknown, |t| self.type_of(t));
                self.made(&t, e.span);
                t
            }
            ExprKind::NotNull(inner) => self.expr(inner).non_null().clone(),
            ExprKind::Index { expr, index } => {
                self.expr(index);
                match self.expr(expr) {
                    Type::List(item) => *item,
                    Type::Map(_, value) => *value,
                    _ => Type::Unknown,
                }
            }
            _ => {
                self.walk_inner(e);
                Type::Unknown
            }
        }
    }

    /// The parts of an expression whose type isn't known yet, checked inside.
    fn walk_inner(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Is { expr, .. }
            | ExprKind::Has { expr, .. }
            | ExprKind::TypeArgs { expr, .. } => {
                self.expr(expr);
            }
            ExprKind::Tuple(items) => items.iter().for_each(|i| {
                self.expr(i);
            }),
            ExprKind::Collection(items) => items.iter().for_each(|i| match i {
                Element::Item(e) | Element::Spread(e) => {
                    self.expr(e);
                }
                Element::Entry(k, v) => {
                    self.expr(k);
                    self.expr(v);
                }
            }),
            ExprKind::Range { from, to, .. } => {
                self.expr(from);
                self.expr(to);
            }
            ExprKind::Lambda { body, .. } => self.scoped(|c| c.lambda_body(body)),
            _ => {}
        }
    }

    /// A name: a local, a member of `this`, the file's own, a type.
    fn name(&mut self, name: &str, at: Span) -> Type {
        if let Some(t) = self.local(name) {
            return t.clone();
        }
        if let Some(this) = self.this.clone() {
            let t = self.own_member(&this, name);
            if t.known() {
                return t;
            }
        }
        if let Some(t) = self.own.values.get(name).cloned() {
            return self.type_of(&t);
        }
        if self.own.types.contains_key(name) {
            return Type::Static(name.to_string());
        }
        self.imported(name, at)
    }

    fn unary(&mut self, op: UnOp, expr: &Expr) -> Type {
        let t = self.expr(expr);
        match op {
            UnOp::Not => {
                self.want(&t, &Type::Bool, expr.span);
                Type::Bool
            }
            UnOp::Neg => t,
            UnOp::Try | UnOp::Await => Type::Unknown,
        }
    }

    fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, at: Span) -> Type {
        let (a, b) = (self.expr(left), self.expr(right));
        match op {
            BinOp::And | BinOp::Or => {
                self.want(&a, &Type::Bool, left.span);
                self.want(&b, &Type::Bool, right.span);
                Type::Bool
            }
            BinOp::Eq
            | BinOp::NotEq
            | BinOp::Lt
            | BinOp::LtEq
            | BinOp::Gt
            | BinOp::GtEq
            | BinOp::In => Type::Bool,
            BinOp::Coalesce => a.non_null().clone(),
            BinOp::Add if a == Type::String || b == Type::String => Type::String,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                if a.known() && b.known() && !(a.is_numeric() && b.is_numeric()) {
                    self.report.error(
                        script_problem::TYPE_MISMATCH,
                        at,
                        format!("`{a}` and `{b}` don't make a number together"),
                    );
                    return Type::Unknown;
                }
                Type::wider(&a, &b)
            }
        }
    }

    /// A condition: a `bool`.
    pub(super) fn condition(&mut self, e: &Expr) {
        let t = self.expr(e);
        self.want(&t, &Type::Bool, e.span);
    }

    /// `e`'s value where `wanted` is: reported when it doesn't fit. A
    /// number written as a literal fits any number and any quantity.
    pub(super) fn fits(&mut self, e: &Expr, wanted: &Type) {
        if let Type::Fn(params, ret) = wanted.non_null()
            && self.as_delegate(e, params, ret)
        {
            return;
        }
        let t = self.expr(e);
        if is_number_literal(e) && (wanted.non_null().is_numeric()) {
            return;
        }
        self.want(&t, wanted, e.span);
    }

    /// `got` where `wanted` is: reported when it doesn't convert.
    pub(super) fn want(&mut self, got: &Type, wanted: &Type, at: Span) {
        if self.converts(got, wanted) {
            return;
        }
        let (code, message) = if *got == Type::Null
            || got.is_nullable() && got.non_null().converts_to(wanted)
        {
            (
                script_problem::NULLABLE,
                format!(
                    "this may be `null`, but a `{wanted}` can't be: `??` gives a value for null, `!` says it isn't"
                ),
            )
        } else {
            (
                script_problem::TYPE_MISMATCH,
                format!("a `{got}` where a `{wanted}` is wanted"),
            )
        };
        self.report.error(code, at, message);
    }
}

/// The unit a suffix names: the game's, or a mod's own by its name.
impl Checker<'_, '_, '_> {
    /// `out let x`: `x` bound, of its written type (else unknown).
    fn declare(&mut self, ty: Option<&crate::ast::Type>, name: &str, mutable: bool) -> Type {
        let t = ty.map_or(Type::Unknown, |t| self.type_of(t));
        self.bind_as(name, t.clone(), !mutable);
        t
    }

    /// `nameof(x)`: a string, `x` checked.
    fn name_of(&mut self, expr: &Expr) -> Type {
        self.expr(expr);
        Type::String
    }
}

/// A literal's type: what it is as written.
fn literal(kind: &ExprKind) -> Option<Type> {
    let t = match kind {
        ExprKind::Int(_) => Type::I32,
        ExprKind::Float(_) => Type::F64,
        ExprKind::Unit { unit, .. } => super::builtin::by_word(unit)
            .filter(super::builtin::is_number)
            .unwrap_or_else(|| Type::Unit(unit_of(unit))),
        ExprKind::RawStr(_) => Type::String,
        ExprKind::Char(_) => Type::Char,
        ExprKind::Text(_) => Type::Text,
        ExprKind::Content { kind, .. } => Type::Content(kind.clone()),
        ExprKind::Tag(_) => Type::Tag,
        ExprKind::Null => Type::Null,
        _ => return None,
    };
    Some(t)
}

fn unit_of(suffix: &str) -> String {
    match suffix {
        TICK_SUFFIX => TICKS.to_string(),
        BLOCK_SUFFIX => BLOCKS.to_string(),
        s if SECOND_SUFFIXES.contains(&s) => SECONDS.to_string(),
        own => own.to_string(),
    }
}

/// `3`, `-0.5`: a number as written.
fn is_number_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) => true,
        ExprKind::Unary {
            op: UnOp::Neg,
            expr,
        } => is_number_literal(expr),
        _ => false,
    }
}
