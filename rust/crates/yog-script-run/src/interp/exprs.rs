//! Expressions: literals, names, and the forms made of others.

use std::rc::Rc;

use yog_script_analyze::ast::{Element, Expr, ExprKind, StrSeg};

use super::{Interp, Run};
use crate::value::{Closure, Value};

impl<'p> Interp<'p> {
    pub(crate) fn eval(&mut self, e: &Expr) -> Run<Value> {
        if let Some(v) = literal(&e.kind) {
            return Ok(v);
        }
        match &e.kind {
            ExprKind::Str(segs) => {
                let mut out = String::new();
                for s in segs {
                    match s {
                        StrSeg::Text(t) => out.push_str(t),
                        StrSeg::Expr(e) => out.push_str(&self.eval(e)?.to_string()),
                    }
                }
                Ok(Value::str(&out))
            }
            ExprKind::Ident(name) => self.name(name, e.span),
            ExprKind::This => Ok(self.this.clone().unwrap_or(Value::Null)),
            ExprKind::Member { expr, name, safe } => {
                let on = self.eval(expr)?;
                if *safe && on == Value::Null {
                    return Ok(Value::Null);
                }
                self.member(&on, &name.text, e.span)
            }
            ExprKind::Call { callee, args } => self.call_expr(callee, args, e.span),
            ExprKind::New { ty, args, init } => self.new_object(
                ty.as_ref(),
                args.as_deref().unwrap_or_default(),
                init,
                e.span,
            ),
            ExprKind::Unary { op, expr } => self.unary(*op, expr, e.span),
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right, e.span),
            ExprKind::Range {
                from,
                to,
                inclusive,
            } => match (self.eval(from)?, self.eval(to)?) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Range(a, b, *inclusive)),
                _ => self.panic("a range is of whole numbers", e.span),
            },
            ExprKind::Conditional {
                cond,
                then,
                otherwise,
            } => {
                if self.eval(cond)?.truthy() {
                    self.eval(then)
                } else {
                    self.eval(otherwise)
                }
            }
            _ => self.compound(e),
        }
    }

    /// The forms made of others, past the common ones.
    fn compound(&mut self, e: &Expr) -> Run<Value> {
        match &e.kind {
            ExprKind::Tuple { items, .. } => Ok(Value::list(
                items.iter().map(|i| self.eval(i)).collect::<Run<_>>()?,
            )),
            ExprKind::Collection(items) => self.collection(items),
            ExprKind::Lambda { params, body, .. } => Ok(Value::Fn(Rc::new(Closure {
                params: params.clone(),
                body: body.clone(),
                scopes: self.scopes.clone(),
                this: self.this.clone(),
            }))),
            ExprKind::Switch { expr, arms } => self.switch(expr, arms, e.span),
            ExprKind::Is { expr, pattern } => {
                let v = self.eval(expr)?;
                self.matches(pattern, &v)
            }
            ExprKind::As { expr, ty } => {
                let v = self.eval(expr)?;
                Ok(if self.is_type(&v, ty) { v } else { Value::Null })
            }
            ExprKind::Cast { ty, expr } => {
                let v = self.eval(expr)?;
                Ok(self.cast(v, ty))
            }
            ExprKind::Index { expr, args } => {
                let on = self.eval(expr)?;
                args.iter().try_fold(on, |on, i| self.index(on, i, expr))
            }
            ExprKind::NewArray {
                elem, sizes, init, ..
            } => self.new_array(elem.as_ref(), sizes, init.as_deref()),
            ExprKind::NotNull(inner) => match self.eval(inner)? {
                Value::Null => self.panic("`!` on a value that is null", e.span),
                v => Ok(v),
            },
            ExprKind::Move(inner) | ExprKind::TypeArgs { expr: inner, .. } => self.eval(inner),
            ExprKind::NameOf(inner) => Ok(Value::str(&last_name(inner))),
            ExprKind::Macro { name, args } => self.builtin_macro(name, args, e.span),
            ExprKind::Declare { name, .. } => {
                self.bind(&name.text, Value::Null);
                Ok(Value::Null)
            }
            ExprKind::Default(_) => Ok(Value::Null),
            _ => self.panic("this can't run yet", e.span),
        }
    }

    fn collection(&mut self, items: &[Element]) -> Run<Value> {
        let mut out = Vec::new();
        for i in items {
            match i {
                Element::Item(e) => out.push(self.eval(e)?),
                Element::Spread(e) => match self.eval(e)? {
                    Value::List(more) => out.extend(more.borrow().iter().cloned()),
                    other => out.push(other),
                },
                Element::Entry(k, v) => out.push(Value::list(vec![self.eval(k)?, self.eval(v)?])),
            }
        }
        Ok(Value::list(out))
    }

    /// `on[index]` (`expr` is `on`, as written); a rank's next index reads
    /// the row the last gave.
    pub(super) fn index(&mut self, on: Value, index: &Expr, expr: &Expr) -> Run<Value> {
        let at = self.eval(index)?;
        match (&on, at) {
            (Value::List(items), Value::Int(k)) => match usize::try_from(k)
                .ok()
                .and_then(|k| items.borrow().get(k).cloned())
            {
                Some(v) => Ok(v),
                None => self.panic(
                    format!("index {k} out of range ({} items)", items.borrow().len()),
                    index.span,
                ),
            },
            (Value::Str(s), Value::Int(k)) => {
                match usize::try_from(k).ok().and_then(|k| s.chars().nth(k)) {
                    Some(c) => Ok(Value::Char(c)),
                    None => self.panic(format!("index {k} out of range"), index.span),
                }
            }
            _ => self.panic(format!("`{on}` isn't indexed"), expr.span),
        }
    }

    /// A name: a local, a field or a computed property of `this`, a type.
    fn name(&mut self, name: &str, at: yog_script_analyze::token::Span) -> Run<Value> {
        if let Some(v) = self.lookup(name) {
            return Ok(v);
        }
        if let Some(this) = self.this.clone()
            && let Some(v) = self.field(&this, name)?
        {
            return Ok(v);
        }
        if self.program.types.contains_key(name) {
            return Ok(Value::Type(name.to_string()));
        }
        self.panic(format!("`{name}` has no value here"), at)
    }
}

/// A literal's value.
fn literal(kind: &ExprKind) -> Option<Value> {
    Some(match kind {
        ExprKind::Int(n) => Value::Int(*n),
        ExprKind::Float(x) => Value::Float(*x),
        ExprKind::Unit { value, .. } => value
            .parse::<i64>()
            .map(Value::Int)
            .or_else(|_| value.parse::<f64>().map(Value::Float))
            .ok()?,
        ExprKind::RawStr(s) | ExprKind::Text(s) | ExprKind::Tag(s) => Value::str(s),
        ExprKind::Content { id, .. } => Value::str(id),
        ExprKind::Char(c) => Value::Char(*c),
        ExprKind::Bool(b) => Value::Bool(*b),
        ExprKind::Null => Value::Null,
        _ => return None,
    })
}

/// `nameof(a.B)`: `B`.
fn last_name(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Ident(n) => n.clone(),
        ExprKind::Member { name, .. } => name.text.clone(),
        _ => String::new(),
    }
}
