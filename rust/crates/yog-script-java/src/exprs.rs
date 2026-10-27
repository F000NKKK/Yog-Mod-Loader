//! Expressions: Java's own where they mean the same, the runtime's where
//! they don't (`==` on objects is `Objects.equals`, a list's element a
//! checked read, a struct copied where it's handed on).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Expr, ExprKind, LambdaBody, Stmt, StrSeg, UnOp};

use yog_script_ids::java::{
    BOOLEAN, CHAR, DOUBLE, FALSE, FLOAT, INT, LONG, NEW, NULL, RETURN, ZERO, ZERO_CHAR,
};
use yog_script_ids::jvm::{
    COPY, EQUALS, OBJECT, OBJECTS, REF_VALUE, REQUIRE_NON_NULL_ELSE, TEXT_OF, TEXT_WITH,
};
use yog_script_ids::members::TO_STRING;
use yog_script_ids::units::SECOND_SUFFIXES;

use super::Gen;
use crate::index::Kind;
use crate::types::{RUNTIME, primitive};
use yog_script_ids::java::UNSIGNED_SHR;

impl Gen<'_> {
    /// `e` as handed on: a struct's object copied, as Yog Script copies it.
    pub(crate) fn handed_expr(&mut self, e: &Expr) -> String {
        let v = self.expr(e);
        let is_struct = matches!(self.types.of(e), Some(Type::Own(n, args)) if self.index.kind(&(n.clone(), args.len())) == Some(Kind::Struct))
            || Self::is_tuple(self.types.of(e));
        if is_struct && matches!(e.kind, ExprKind::Ident(_) | ExprKind::Member { .. }) {
            format!("{v}.{COPY}()")
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
        if let Some(group) = self.group(e) {
            return group;
        }
        match &e.kind {
            ExprKind::Int(n) => self.number(&n.to_string(), e),
            ExprKind::Float(x) => self.number(&format!("{x:?}"), e),
            ExprKind::Unit { value, unit } => self.unit(value, unit, e),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Null => NULL.into(),
            ExprKind::Char(c) => format!("'{}'", self.code.str(&c.to_string()).trim_matches('"')),
            ExprKind::RawStr(s) | ExprKind::Tag(s) => self.code.str(s),
            // `t"key"`: Yog's `Text` of the key.
            ExprKind::Text(key) => format!(
                "{}.{TEXT_OF}({})",
                self.index.place(yog_script_ids::system::TEXT_TYPE, 0),
                self.code.str(key)
            ),
            ExprKind::Content { id, .. } => self.code.str(id),
            ExprKind::Str(segs) => self.interpolated(segs),
            ExprKind::Ident(name) if let Some(field) = self.field_of(name) => field.to_string(),
            ExprKind::Ident(name) if self.cells.contains(name) => {
                format!("{}.{REF_VALUE}", Self::ident(name))
            }
            // A generic type's static field: its arguments' set's.
            ExprKind::Ident(name)
                if !self.types.is_local(e)
                    && self
                        .owner
                        .as_ref()
                        .is_some_and(|o| self.index.static_per_type(o, name)) =>
            {
                self.own_static(name)
            }
            // A property of `this` by its name: its getter.
            ExprKind::Ident(name)
                if self
                    .owner
                    .as_ref()
                    .is_some_and(|o| self.index.member_called(o, name)) =>
            {
                format!("{}()", Self::ident(name))
            }
            ExprKind::Ident(name) => Self::ident(name),
            ExprKind::This => self.this_ref(),
            ExprKind::Base => self.base_ref(),
            ExprKind::Member { expr, name, safe } => self.member(e, expr, &name.text, *safe),
            ExprKind::Call { callee, args } if let Some(m) = self.types.message(e).cloned() => {
                self.message_sent(e, (callee, args), &m)
            }
            // `t"key"(a, b)`: `Text.With`, its arguments as objects.
            ExprKind::Call { callee, args } if matches!(callee.kind, ExprKind::Text(_)) => {
                let text = self.expr(callee);
                let a = self.args(args);
                format!("{text}.{TEXT_WITH}({NEW} {OBJECT}[] {{ {a} }})")
            }
            ExprKind::Call { callee, args } => self.giving_back(e, |g| g.call(e, callee, args)),
            ExprKind::New { .. } => self.giving_back(e, |g| g.new_expr(e)),
            ExprKind::Unary { op, expr } => self.unary(*op, expr),
            ExprKind::Binary { op, left, right } => self.binary(e, *op, left, right),
            ExprKind::Conditional {
                cond,
                then,
                otherwise,
            } => {
                if let Some(v) = self.split_conditional(e, (cond, then, otherwise)) {
                    return v;
                }
                let (c, a, b) = (self.expr(cond), self.expr(then), self.expr(otherwise));
                format!("({c} ? {a} : {b})")
            }
            _ => self.compound(e),
        }
    }

    fn compound(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Cast { ty, expr } => match self.enum_cast(e, expr) {
                Some(v) => v,
                // A number made a `decimal`: by its conversion, not Java's cast.
                None if self.types.of(e).map(Type::non_null) == Some(&Type::Decimal) => {
                    let v = self.expr(expr);
                    self.as_decimal(expr, v)
                }
                None => format!("(({}) {})", self.written(ty, false), self.expr(expr)),
            },
            ExprKind::NotNull(inner)
            | ExprKind::Move(inner)
            | ExprKind::TypeArgs { expr: inner, .. } => self.expr(inner),
            ExprKind::Index { expr, args, safe } => self.index_expr(e, expr, args, *safe),
            ExprKind::NewArray { sizes, init, .. } => self.new_array(e, sizes, init.as_deref()),
            ExprKind::Collection(items) => self.collection(e, items),
            ExprKind::Tuple { items, .. } => self.tuple(items),
            ExprKind::Lambda {
                params,
                body,
                asynchronous: true,
            } => self.async_lambda(e, params, body),
            ExprKind::Lambda { params, body, .. } => match self.types.quoted(e).cloned() {
                Some(quoted) => self.quoted_lambda(e, (params, body), &quoted),
                None => self.lambda(e, params, body),
            },
            ExprKind::Switch { expr, arms } => self.switch(e, expr, arms),
            ExprKind::With { expr, inits } => self.with(e, expr, inits),
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
            ExprKind::Default(_) => match self.types.of(e) {
                Some(t) if let Some(z) = self.param_zero(t) => z,
                t => t.map_or(NULL, |t| zero(t)).to_string(),
            },
            _ => {
                self.not_yet("this expression", e.span);
                String::new()
            }
        }
    }

    /// `20t`, `5u8`: a number of its type.
    /// A quantity as written (`250ms`): a number of its unit, a multiple of
    /// seconds as seconds (`0.25`).
    fn unit(&mut self, value: &str, unit: &str, e: &Expr) -> String {
        let factor = SECOND_SUFFIXES
            .iter()
            .find(|(s, _)| *s == unit)
            .map_or(1.0, |(_, f)| *f);
        match value.parse::<f64>() {
            Ok(v) if factor != 1.0 => self.number(&format!("{:?}", v * factor), e),
            _ => self.number(value, e),
        }
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
        self.text_of(self.types.of(e), v)
    }

    /// Value `v` of type `t` as Yog Script writes it in text: a number,
    /// a `bool`, a `decimal` as .NET does (`2`, `True`), boxed too; else Java's.
    pub(crate) fn text_of(&self, t: Option<&Type>, v: String) -> String {
        match t.map(Type::non_null) {
            Some(t @ (Type::F32 | Type::F64 | Type::Bool | Type::Decimal)) => {
                match self.index.host(t) {
                    Some(host) => format!("{host}.{TO_STRING}({v})"),
                    None => v,
                }
            }
            // What may hold one boxed: as its own type writes it.
            Some(Type::Object | Type::Dynamic | Type::Param(_)) => {
                match self.index.host(&Type::Object) {
                    Some(host) => format!("{host}.{TO_STRING}({v})"),
                    None => v,
                }
            }
            _ => v,
        }
    }

    fn unary(&mut self, op: UnOp, expr: &Expr) -> String {
        if let Some(v) = self.enum_unary(op, expr) {
            return v;
        }
        match op {
            UnOp::Neg if self.types.of(expr).map(Type::non_null) == Some(&Type::Decimal) => {
                Self::decimal_negated(&self.expr(expr))
            }
            UnOp::Neg => format!("(-{})", self.expr(expr)),
            UnOp::FromEnd => self.counted_back(expr),
            UnOp::Not => format!("(!{})", self.expr(expr)),
            UnOp::BitNot => format!("(~{})", self.expr(expr)),
            // Steps bind tightest: bare, so one stands as a statement.
            UnOp::Increment => format!("++{}", self.expr(expr)),
            UnOp::Decrement => format!("--{}", self.expr(expr)),
            UnOp::PostIncrement => format!("{}++", self.expr(expr)),
            UnOp::PostDecrement => format!("{}--", self.expr(expr)),
            UnOp::Try => self.try_value(expr),
            UnOp::Await => self.await_value(expr, expr),
        }
    }

    /// `try r`: an `Err` returned from here, else the value.
    fn try_value(&mut self, expr: &Expr) -> String {
        let r = self.expr(expr);
        let err = self.temp();
        // In a machine: kept in a field (an `await` may come after), its
        // `Err` finishing the task.
        let t = if self.machine.is_some() {
            let t = self.types.of(expr).cloned();
            self.field_of_type(t.as_ref(), &r)
        } else {
            let t = self.temp();
            self.code.line(format!("var {t} = {r};"));
            t
        };
        let failed = format!("new {RUNTIME}.Result.Err<>({err}.Error())");
        self.code.open(format!(
            "if ({t} instanceof {RUNTIME}.Result.Err<?, ?> {err})"
        ));
        if let Some(gives) = self.machine.as_ref().map(|m| m.gives.clone()) {
            // The task's own `Result`: Java infers none from the `Err` taken.
            self.async_return(Some(format!("({gives}) ({OBJECT}) {failed}")));
        } else {
            self.code.line(format!("return {failed};"));
        }
        self.code.close("");
        format!("{t}.Unwrap()")
    }

    fn binary(&mut self, e: &Expr, op: BinOp, left: &Expr, right: &Expr) -> String {
        if let Some(v) = self.enum_binary(op, left, right) {
            return v;
        }
        if let Some(v) = self.short_circuit(e, op, (left, right)) {
            return v;
        }
        let l = self.expr(left);
        let l = self.kept_before(l, left, crate::machine::awaits(right));
        let r = self.expr(right);
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
            BinOp::Eq if objects => format!("{OBJECTS}.{EQUALS}({l}, {r})"),
            BinOp::NotEq if objects => format!("!{OBJECTS}.{EQUALS}({l}, {r})"),
            BinOp::Coalesce => format!("{OBJECTS}.{REQUIRE_NON_NULL_ELSE}({l}, {r})"),
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
        let takes_none = matches!(self.types.of(e), Some(Type::Fn(p, _)) if p.is_empty());
        let names = self.lambda_names(params, takes_none);
        self.lambda_of(&names, body, void)
    }

    /// A lambda's parameters' names; none named: `it`, when its delegate
    /// takes one (`takes_none`: none).
    pub(crate) fn lambda_names(
        &self,
        params: &[yog_script_analyze::ast::LambdaParam],
        takes_none: bool,
    ) -> Vec<String> {
        if params.is_empty() && takes_none {
            Vec::new()
        } else if params.is_empty() {
            vec![yog_script_analyze::ast::IT.to_string()]
        } else {
            params.iter().map(|p| Self::ident(&p.name.text)).collect()
        }
    }

    /// Java's lambda of `names` running `body`; `void`: it gives nothing.
    pub(crate) fn lambda_of(&mut self, names: &[String], body: &LambdaBody, void: bool) -> String {
        let head = format!("({}) ->", names.join(", "));
        // Its body written apart: what it computes first stays inside it,
        // its `return` its own (not an `async` method's around it).
        let outer = std::mem::take(&mut self.code);
        let machine = self.machine.take();
        self.code.open(&head);
        match body {
            // A value given to nothing: a statement, as one written alone.
            LambdaBody::Expr(body) if void => self.expr_stmt(body),
            LambdaBody::Expr(body) => {
                let v = self.handed_expr(body);
                self.code.line(format!("{RETURN} {v};"));
            }
            LambdaBody::Block(stmts) => {
                for s in stmts {
                    match s {
                        // Its last expression, without `;`: its value.
                        Stmt::Value(v) if !void => {
                            let v = self.handed_expr(v);
                            self.code.line(format!("{RETURN} {v};"));
                        }
                        Stmt::Value(v) => self.expr_stmt(v),
                        s => self.stmt(s),
                    }
                }
            }
        }
        self.code.close("");
        self.machine = machine;
        let text = std::mem::replace(&mut self.code, outer).finish();
        text.trim_end().to_string()
    }
}

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
/// A value of `t` before one's set: its zero, `false`, `null`.
pub(crate) fn zero(t: &Type) -> &'static str {
    match primitive(t) {
        Some(BOOLEAN) => FALSE,
        Some(CHAR) => ZERO_CHAR,
        Some(_) => ZERO,
        None => NULL,
    }
}

fn last_name(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Ident(n) => n.clone(),
        ExprKind::Member { name, .. } => name.text.clone(),
        _ => String::new(),
    }
}
