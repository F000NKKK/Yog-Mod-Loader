//! Expressions: Java's own where they mean the same, the runtime's where
//! they don't (`==` on objects is `Objects.equals`, a list's element a
//! checked read, a struct copied where it's handed on).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Expr, ExprKind, LambdaBody, StrSeg, UnOp};

use super::Gen;
use crate::index::Kind;
use crate::types::{RUNTIME, primitive};

impl Gen<'_> {
    /// `e` as handed on: a struct's object copied, as Yog Script copies it.
    pub(crate) fn handed_expr(&mut self, e: &Expr) -> String {
        let v = self.expr(e);
        let is_struct = matches!(self.types.of(e), Some(Type::Own(n, _)) if self.index.kind(n) == Some(Kind::Struct));
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
        match self.types.conversion(e).cloned() {
            Some(used) => self.converted(&used, &v),
            None => v,
        }
    }

    fn expr_kind(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Float(x) => format!("{x:?}"),
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
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right),
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
            ExprKind::Index { expr, index } => {
                let (on, at) = (self.expr(expr), self.expr(index));
                if self.has_indexer(expr) {
                    return Self::index_read(&on, &at);
                }
                match self.types.of(expr) {
                    Some(Type::String) => format!("{on}.charAt({at})"),
                    Some(Type::Array(_)) => format!("{on}[{at}]"),
                    _ => format!("{RUNTIME}.Ys.at({on}, {at}, {})", self.at(e.span)),
                }
            }
            ExprKind::Collection(items) => self.collection(e, items),
            ExprKind::Lambda { params, body } => self.lambda(e, params, body),
            ExprKind::Switch { expr, arms } => self.switch(e, expr, arms),
            ExprKind::Is { expr, pattern } => self.is(expr, pattern),
            ExprKind::As { expr, ty } => {
                let (v, t, tmp) = (self.expr(expr), self.written(ty, true), self.temp());
                format!("({v} instanceof {t} {tmp} ? {tmp} : null)")
            }
            ExprKind::NameOf(inner) => self.code.str(&last_name(inner)),
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
        match self.types.of(e).and_then(primitive) {
            Some("long") => format!("{value}L"),
            Some("float") => format!("{value}f"),
            Some("double") => format!("{value}d"),
            Some(p) if p != "int" => format!("(({p}) {value})"),
            _ => {
                let _ = unit;
                value.to_string()
            }
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
                StrSeg::Expr(e) => parts.push(format!("({})", self.expr(e))),
            }
        }
        format!("({})", parts.join(" + "))
    }

    fn unary(&mut self, op: UnOp, expr: &Expr) -> String {
        match op {
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

    fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr) -> String {
        let (l, r) = (self.expr(left), self.expr(right));
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
        let names: Vec<String> = if params.is_empty() {
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
