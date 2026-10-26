//! Expressions' types, operators, and whether a value fits where a type
//! is wanted.

use yog_ids::script_problem;

use super::Checker;
use super::model::{BLOCKS, SECONDS, TICKS, Type};
use crate::ast::{BinOp, Expr, ExprKind, StrSeg, UnOp};
use crate::source::token::Span;

use yog_script_ids::units::{BLOCK_SUFFIX, SECOND_SUFFIXES, TICK_SUFFIX};

impl Checker<'_, '_, '_> {
    pub(super) fn expr(&mut self, e: &Expr) -> Type {
        let t = self.expr_kind(e);
        self.types.set(e, t.clone());
        t
    }

    fn expr_kind(&mut self, e: &Expr) -> Type {
        if let Some(t) = literal(&e.kind) {
            return t;
        }
        match &e.kind {
            ExprKind::Str(segs) => self.interpolated(segs),
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
            ExprKind::Call { callee, args } => self.call(e, callee, args),
            ExprKind::Switch { expr, arms } => self.switch(expr, arms, e.span),
            ExprKind::Lambda { params, body } => self.lambda_alone(params, body),
            ExprKind::Macro { name, args } => self.builtin_macro(name, args),
            ExprKind::Unary { op, expr } => self.unary(e, *op, expr),
            ExprKind::Binary { op, left, right } => self.binary(e, *op, left, right),
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
                let (got, to) = (self.expr(expr), self.type_of(ty));
                self.note_cast(e, &got, &to);
                to
            }
            ExprKind::As { expr, ty } => {
                self.expr(expr);
                self.type_of(ty).nullable()
            }
            ExprKind::New { ty, args, init } => {
                self.new_object(e, ty.as_ref(), args.as_deref(), init)
            }
            ExprKind::NotNull(inner) => self.expr(inner).non_null().clone(),
            ExprKind::Collection(items) => self.collection(items),
            ExprKind::TypeArgs { expr, args } => self.type_args_expr(expr, args),
            ExprKind::Index { expr, args } => {
                let on = self.expr(expr);
                self.indexed(on, args, e.span)
            }
            ExprKind::NewArray {
                elem,
                rank,
                sizes,
                init,
            } => self.new_array(elem.as_ref(), *rank, sizes, init.as_deref(), e.span),
            _ => {
                self.walk_inner(e);
                Type::Unknown
            }
        }
    }

    /// The parts of an expression whose type isn't known yet, checked inside.
    fn walk_inner(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Is { expr, .. } | ExprKind::Has { expr, .. } => {
                self.expr(expr);
            }
            ExprKind::Tuple(items) => items.iter().for_each(|i| {
                self.expr(i);
            }),
            ExprKind::Range { from, to, .. } => {
                self.expr(from);
                self.expr(to);
            }
            ExprKind::Lambda { params, body } => {
                self.lambda_alone(params, body);
            }
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
        if self.own.has(name) && self.own_in_reach(name) {
            return Type::Static(name.to_string(), Vec::new());
        }
        self.imported(name, at)
    }

    fn unary(&mut self, e: &Expr, op: UnOp, expr: &Expr) -> Type {
        let t = self.expr(expr);
        if op.steps() {
            self.written(expr);
        }
        if let Some(declared) = self.declared_unary(e, op, &t) {
            return declared;
        }
        match op {
            UnOp::Not => {
                self.want(&t, &Type::Bool, expr.span);
                Type::Bool
            }
            UnOp::Neg => t,
            UnOp::BitNot => self.flipped(&t, expr.span),
            UnOp::Increment | UnOp::Decrement | UnOp::PostIncrement | UnOp::PostDecrement => {
                self.stepped(&t, expr.span)
            }
            // `try r`: a `Result<T, E>`'s `T`.
            UnOp::Try => match t.non_null() {
                Type::Own(name, args) if name == yog_script_ids::system::RESULT => {
                    args.first().cloned().unwrap_or(Type::Unknown)
                }
                _ => Type::Unknown,
            },
            UnOp::Await => Type::Unknown,
        }
    }

    fn binary(&mut self, e: &Expr, op: BinOp, left: &Expr, right: &Expr) -> Type {
        let (a, b) = (self.expr(left), self.expr(right));
        if let Some((used, ret)) = self.declared_binary(op, &a, &b) {
            self.types.set_operator(e, used);
            return ret;
        }
        self.combined(op, (a, b), (left.span, right.span))
    }

    /// `a op b` as the language has it (no type declaring it): its type.
    pub(super) fn combined(
        &mut self,
        op: BinOp,
        (a, b): (Type, Type),
        (left, right): (Span, Span),
    ) -> Type {
        let at = left.to(right);
        match op {
            BinOp::And | BinOp::Or => {
                self.want(&a, &Type::Bool, left);
                self.want(&b, &Type::Bool, right);
                Type::Bool
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                self.bitwise(op, (&a, &b), at)
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
        if let Some((params, ret)) = self.delegate_sig(wanted)
            && self.as_delegate(e, &params, &ret)
        {
            return;
        }
        if let ExprKind::Collection(items) = &e.kind
            && self.collection_as(e, items, wanted.non_null())
        {
            return;
        }
        let t = self.expr(e);
        if is_number_literal(e) && (wanted.non_null().is_numeric()) {
            return;
        }
        self.want(&t, wanted, e.span);
        self.note_conversion(e, &t, wanted);
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

    /// `"a {x} b"`: a string, what's inside checked.
    fn interpolated(&mut self, segs: &[StrSeg]) -> Type {
        for s in segs {
            if let StrSeg::Expr(e) = s {
                self.expr(e);
            }
        }
        Type::String
    }

    /// `new T(args) { init }`: a `T`, one that may be made.
    fn new_object(
        &mut self,
        e: &Expr,
        ty: Option<&crate::ast::Type>,
        args: Option<&[crate::ast::Arg]>,
        init: &[(crate::ast::Name, Expr)],
    ) -> Type {
        args.into_iter().flatten().for_each(|a| {
            self.expr(&a.value);
        });
        init.iter().for_each(|(_, v)| {
            self.expr(v);
        });
        let t = ty.map_or(Type::Unknown, |t| self.type_of(t));
        self.made(&t, e.span);
        t
    }

    /// `Comparer<i32>` where a value goes: the type, for its statics; a
    /// generic method's name before its call is the call's (`calls.rs`).
    fn type_args_expr(&mut self, expr: &Expr, args: &[crate::ast::Type]) -> Type {
        let typed: Vec<Type> = args.iter().map(|t| self.type_of(t)).collect();
        match self.expr(expr) {
            Type::Static(name, _) => Type::Static(name, typed),
            other => other,
        }
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
pub(super) fn is_number_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) => true,
        ExprKind::Unary {
            op: UnOp::Neg,
            expr,
        } => is_number_literal(expr),
        _ => false,
    }
}
