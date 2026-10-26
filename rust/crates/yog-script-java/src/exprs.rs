//! Expressions: Java's own where they mean the same, the runtime's where
//! they don't (`==` on objects is `Objects.equals`, a list's element a
//! checked read, a struct copied where it's handed on).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Expr, ExprKind, LambdaBody, StrSeg, UnOp};

use yog_script_ids::jvm::{DOUBLE, FLOAT, HELPERS, INT, LONG, OBJECT};
use yog_script_ids::members::TO_STRING;

use super::Gen;
use crate::index::Kind;
use crate::types::{RUNTIME, primitive};

impl Gen<'_> {
    /// `e` as handed on: a struct's object copied, as Yog Script copies it.
    pub(crate) fn handed_expr(&mut self, e: &Expr) -> String {
        let v = self.expr(e);
        let is_struct = matches!(self.types.of(e), Some(Type::Own(n, _)) if self.index.kind(n) == Some(Kind::Struct))
            || Self::is_tuple(self.types.of(e));
        if is_struct && matches!(e.kind, ExprKind::Ident(_) | ExprKind::Member { .. }) {
            format!("{v}.copy()")
        } else {
            v
        }
    }

    /// `e` in Java: the operator its type declares when it's one, through
    /// the implicit conversion it goes through when it does.
    pub(crate) fn expr(&mut self, e: &Expr) -> String {
        if let Some(used) = self.types.operator(e).cloned() {
            return self.operator_call(e, &used);
        }
        let v = self.expr_kind(e);
        let v = match self.types.conversion(e).cloned() {
            Some(used) => self.converted(&used, &v),
            None => v,
        };
        // Gone where it's wanted by variance: Java's generics have none.
        match self.types.recast(e) {
            Some(to) => format!("(({}) ({OBJECT}) {v})", self.java(to, false)),
            None => v,
        }
    }

    fn expr_kind(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(n) => self.number(&n.to_string(), e),
            ExprKind::Float(x) => self.number(&format!("{x:?}"), e),
            ExprKind::Unit { value, unit } => self.unit(value, unit, e),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Null => "null".into(),
            ExprKind::Char(c) => format!("'{}'", self.code.str(&c.to_string()).trim_matches('"')),
            ExprKind::RawStr(s) | ExprKind::Text(s) | ExprKind::Tag(s) => self.code.str(s),
            ExprKind::Content { id, .. } => self.code.str(id),
            ExprKind::Str(segs) => self.interpolated(segs),
            ExprKind::Ident(name) => Self::ident(name),
            ExprKind::This => "this".into(),
            ExprKind::Base => "super".into(),
            ExprKind::Member { expr, name, safe } => self.member(e, expr, &name.text, *safe),
            ExprKind::Call { callee, args } => self.call(e, callee, args),
            ExprKind::New { ty, args, init } => {
                self.new_object(e, ty.as_ref(), args.as_deref().unwrap_or_default(), init)
            }
            ExprKind::Unary { op, expr } => self.unary(*op, expr),
            ExprKind::Binary { op, left, right } => self.binary(e, *op, left, right),
            ExprKind::Conditional {
                cond,
                then,
                otherwise,
            } => {
                let (c, a, b) = (self.expr(cond), self.expr(then), self.expr(otherwise));
                format!("({c} ? {a} : {b})")
            }
            _ => self.compound(e),
        }
    }

    fn compound(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Cast { ty, expr } => {
                format!("(({}) {})", self.written(ty, false), self.expr(expr))
            }
            ExprKind::NotNull(inner)
            | ExprKind::Move(inner)
            | ExprKind::TypeArgs { expr: inner, .. } => self.expr(inner),
            ExprKind::Index { expr, args } => {
                let on = self.expr(expr);
                let at: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                if let Some(Type::Array(..)) = self.types.of(expr).map(Type::non_null) {
                    return Self::array_slot(&on, &at);
                }
                if self.has_indexer(expr) {
                    return Self::index_read(&on, &at.join(", "));
                }
                if let Some(host) = self.types.of(expr).and_then(|t| self.index.host(t)) {
                    return Self::index_read(
                        &host,
                        &[on].into_iter().chain(at).collect::<Vec<_>>().join(", "),
                    );
                }
                let at = at.join(", ");
                format!("{RUNTIME}.{HELPERS}.at({on}, {at}, {})", self.at(e.span))
            }
            ExprKind::NewArray { sizes, init, .. } => self.new_array(e, sizes, init.as_deref()),
            ExprKind::Collection(items) => self.collection(e, items),
            ExprKind::Tuple { items, .. } => self.tuple(items),
            ExprKind::Lambda { params, body } => self.lambda(e, params, body),
            ExprKind::Switch { expr, arms } => self.switch(e, expr, arms),
            ExprKind::Is { expr, pattern } => self.is(expr, pattern),
            ExprKind::As { expr, ty } => {
                let (v, t, tmp) = (self.expr(expr), self.written(ty, true), self.temp());
                format!("({v} instanceof {t} {tmp} ? {tmp} : null)")
            }
            ExprKind::NameOf(inner) => self.code.str(&last_name(inner)),
            ExprKind::TypeOf(ty) => match self.type_value(ty, &|_| None) {
                Some(v) => v,
                None => {
                    self.not_yet("`typeof` of a type parameter", e.span);
                    String::new()
                }
            },
            ExprKind::Macro { name, args } => self.macro_call(e, &name.text, args),
            ExprKind::Default(_) => self.types.of(e).map_or("null", |t| zero(t)).to_string(),
            _ => {
                self.not_yet("this expression", e.span);
                String::new()
            }
        }
    }

    /// `20t`, `5u8`: a number of its type.
    fn unit(&mut self, value: &str, unit: &str, e: &Expr) -> String {
        let _ = unit;
        self.number(value, e)
    }

    /// A number written `value`, of the type `e` is where it stands (`1L`
    /// where an `i64` is wanted, `2f` an `f32`).
    fn number(&self, value: &str, e: &Expr) -> String {
        match self.types.of(e).and_then(primitive) {
            Some(LONG) => format!("{value}L"),
            Some(FLOAT) => format!("{value}f"),
            Some(DOUBLE) => format!("{value}d"),
            Some(p) if p != INT => format!("(({p}) {value})"),
            _ => value.to_string(),
        }
    }

    /// `"a {x} b"`: Java's `+` from a string.
    fn interpolated(&mut self, segs: &[StrSeg]) -> String {
        if let [StrSeg::Text(t)] = segs {
            return self.code.str(t);
        }
        let mut parts = vec![self.code.str("")];
        for s in segs {
            match s {
                StrSeg::Text(t) => parts.push(self.code.str(t)),
                StrSeg::Expr(e) => {
                    let v = self.expr(e);
                    parts.push(format!("({})", self.as_text(e, v)));
                }
            }
        }
        format!("({})", parts.join(" + "))
    }

    /// `v` (`e`'s) where text is made of it: a number Java writes its own
    /// way (`2.0`, `true`) written as Yog Script does (`2`, `True`).
    fn as_text(&self, e: &Expr, v: String) -> String {
        match self.types.of(e).map(Type::non_null) {
            Some(t @ (Type::F32 | Type::F64 | Type::Bool | Type::Decimal)) => {
                match self.index.host(t) {
                    Some(host) => format!("{host}.{TO_STRING}({v})"),
                    None => v,
                }
            }
            _ => v,
        }
    }

    fn unary(&mut self, op: UnOp, expr: &Expr) -> String {
        match op {
            UnOp::Neg if self.types.of(expr).map(Type::non_null) == Some(&Type::Decimal) => {
                Self::decimal_negated(&self.expr(expr))
            }
            UnOp::Neg => format!("(-{})", self.expr(expr)),
            UnOp::Not => format!("(!{})", self.expr(expr)),
            UnOp::BitNot => format!("(~{})", self.expr(expr)),
            // Steps bind tightest: bare, so one stands as a statement.
            UnOp::Increment => format!("++{}", self.expr(expr)),
            UnOp::Decrement => format!("--{}", self.expr(expr)),
            UnOp::PostIncrement => format!("{}++", self.expr(expr)),
            UnOp::PostDecrement => format!("{}--", self.expr(expr)),
            UnOp::Try => self.try_value(expr),
            UnOp::Await => {
                self.not_yet("`await`", expr.span);
                String::new()
            }
        }
    }

    /// `try r`: an `Err` returned from here, else the value.
    fn try_value(&mut self, expr: &Expr) -> String {
        let r = self.expr(expr);
        let (t, err) = (self.temp(), self.temp());
        self.code.line(format!("var {t} = {r};"));
        self.code.line(format!("if ({t} instanceof {RUNTIME}.Result.Err<?, ?> {err}) return new {RUNTIME}.Result.Err<>({err}.Error());"));
        format!("{t}.Unwrap()")
    }

    fn binary(&mut self, e: &Expr, op: BinOp, left: &Expr, right: &Expr) -> String {
        let (l, r) = (self.expr(left), self.expr(right));
        if self.is_decimal(e, left, right)
            && let Some(v) = self.decimal_binary(op, (left, l.clone()), (right, r.clone()))
        {
            return v;
        }
        // Text put together: each piece written as Yog Script writes it.
        let (l, r) = if op == BinOp::Add && self.types.of(e) == Some(&Type::String) {
            (self.as_text(left, l), self.as_text(right, r))
        } else {
            (l, r)
        };
        let objects =
            !matches!(self.types.of(left).map(|t| t.non_null()), Some(t) if primitive(t).is_some());
        match op {
            BinOp::Eq if objects => format!("java.util.Objects.equals({l}, {r})"),
            BinOp::NotEq if objects => format!("!java.util.Objects.equals({l}, {r})"),
            BinOp::Coalesce => format!("java.util.Objects.requireNonNullElse({l}, {r})"),
            BinOp::In => self.contains(right, &l, &r),
            // A `u64`'s bits shift in zeros: Java's unsigned shift.
            BinOp::Shr if self.types.of(left).map(Type::non_null) == Some(&Type::U64) => {
                format!("({l} {UNSIGNED_SHR} {r})")
            }
            _ => format!("({l} {} {r})", op_text(op)),
        }
    }

    /// `x in r`: a range's ends compared, a collection's `contains`.
    fn contains(&mut self, right: &Expr, x: &str, r: &str) -> String {
        if let ExprKind::Range {
            from,
            to,
            inclusive,
        } = &right.kind
        {
            let (a, b) = (self.expr(from), self.expr(to));
            let cmp = if *inclusive { "<=" } else { "<" };
            return format!("({a} <= {x} && {x} {cmp} {b})");
        }
        match self.types.of(right).map(Type::non_null) {
            Some(Type::Own(..)) => format!("{r}.Contains({x})"),
            _ => format!("{r}.contains({x})"),
        }
    }

    fn lambda(
        &mut self,
        e: &Expr,
        params: &[yog_script_analyze::ast::LambdaParam],
        body: &LambdaBody,
    ) -> String {
        let void = matches!(self.types.of(e), Some(Type::Fn(_, ret)) if **ret == Type::Void);
        // None named: `it`, when its delegate takes one; none when none.
        let takes_none = matches!(self.types.of(e), Some(Type::Fn(p, _)) if p.is_empty());
        let names: Vec<String> = if params.is_empty() && takes_none {
            Vec::new()
        } else if params.is_empty() {
            vec![yog_script_analyze::ast::IT.to_string()]
        } else {
            params.iter().map(|p| Self::ident(&p.name.text)).collect()
        };
        let head = format!("({}) ->", names.join(", "));
        // Its body written apart: what it computes first stays inside it.
        let outer = std::mem::take(&mut self.code);
        self.code.open(&head);
        match body {
            // A value given to nothing: a statement, as one written alone.
            LambdaBody::Expr(body) if void => self.expr_stmt(body),
            LambdaBody::Expr(body) => {
                let v = self.expr(body);
                self.code.line(format!("return {v};"));
            }
            LambdaBody::Block(stmts) => stmts.iter().for_each(|s| self.stmt(s)),
        }
        self.code.close("");
        let text = std::mem::replace(&mut self.code, outer).finish();
        text.trim_end().to_string()
    }
}

/// Java's shift that fills with zeros.
const UNSIGNED_SHR: &str = ">>>";

pub(crate) fn op_text(op: BinOp) -> &'static str {
    match op {
        BinOp::Or => "||",
        BinOp::And => "&&",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Lt => "<",
        BinOp::LtEq => "<=",
        BinOp::Gt => ">",
        BinOp::GtEq => ">=",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::Coalesce | BinOp::In => "",
    }
}

/// A type's zero value (`default(T)`).
fn zero(t: &Type) -> &'static str {
    match primitive(t) {
        Some("boolean") => "false",
        Some("char") => "'\\0'",
        Some(_) => "0",
        None => "null",
    }
}

fn last_name(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Ident(n) => n.clone(),
        ExprKind::Member { name, .. } => name.text.clone(),
        _ => String::new(),
    }
}
