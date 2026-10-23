//! Operators: arithmetic, comparisons, logic, `??`, `in`, casts.

use yog_script_analyze::ast::{BinOp, Expr, Type, TypeKind, UnOp};
use yog_script_analyze::token::Span;
use yog_script_analyze::words;

use super::{Interp, Run, Stop};
use crate::value::Value;

impl<'p> Interp<'p> {
    pub(crate) fn unary(&mut self, op: UnOp, expr: &Expr, at: Span) -> Run<Value> {
        let v = self.eval(expr)?;
        match (op, v) {
            (UnOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
            (UnOp::Neg, Value::Float(x)) => Ok(Value::Float(-x)),
            (UnOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnOp::Try, v) => self.try_value(v, at),
            (_, v) => self.panic(format!("this operator doesn't take `{v}`"), at),
        }
    }

    /// `try r`: an `Ok`'s value; an `Err` returned from the method.
    fn try_value(&self, v: Value, at: Span) -> Run<Value> {
        match &v {
            Value::Case(c) if c.of == yog_script_analyze::system::RESULT => {
                if self.is_ok_case(c) {
                    Ok(c.fields.first().cloned().unwrap_or(Value::Void))
                } else {
                    Err(Stop::Return(v.clone()))
                }
            }
            _ => self.panic(format!("`try` takes a Result, not `{v}`"), at),
        }
    }

    /// Whether case `c` of `Result` is its first (the value's) case.
    pub(crate) fn is_ok_case(&self, c: &crate::value::Case) -> bool {
        self.program
            .types
            .get(c.of.as_str())
            .and_then(|t| t.variants.first())
            .is_some_and(|v| v.name.text == c.name)
    }

    pub(crate) fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, at: Span) -> Run<Value> {
        let l = self.eval(left)?;
        // `&&`, `||`, `??` read their right side only when it decides.
        match (op, &l) {
            (BinOp::And, Value::Bool(false)) => return Ok(l),
            (BinOp::Or, Value::Bool(true)) => return Ok(l),
            (BinOp::Coalesce, v) if *v != Value::Null => return Ok(l),
            _ => {}
        }
        let r = self.eval(right)?;
        self.binary_values(op, l, r, at)
    }

    pub(crate) fn binary_values(&self, op: BinOp, l: Value, r: Value, at: Span) -> Run<Value> {
        let v = match (op, &l, &r) {
            (BinOp::And | BinOp::Or | BinOp::Coalesce, _, _) => r,
            (BinOp::Eq, _, _) => Value::Bool(l == r),
            (BinOp::NotEq, _, _) => Value::Bool(l != r),
            (BinOp::In, _, _) => Value::Bool(contains(&r, &l)),
            (BinOp::Add, Value::Str(a), b) => Value::str(&format!("{a}{b}")),
            (BinOp::Add, a, Value::Str(b)) => Value::str(&format!("{a}{b}")),
            (_, Value::Int(a), Value::Int(b)) => return self.ints(op, *a, *b, at),
            (_, Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
                floats(op, num(&l), num(&r))
            }
            _ => return self.panic(format!("`{l}` and `{r}` don't take this operator"), at),
        };
        Ok(v)
    }

    fn ints(&self, op: BinOp, a: i64, b: i64, at: Span) -> Run<Value> {
        let checked = match op {
            BinOp::Add => a.checked_add(b),
            BinOp::Sub => a.checked_sub(b),
            BinOp::Mul => a.checked_mul(b),
            BinOp::Div if b == 0 => return self.panic("division by zero", at),
            BinOp::Div => a.checked_div(b),
            BinOp::Rem if b == 0 => return self.panic("division by zero", at),
            BinOp::Rem => a.checked_rem(b),
            _ => return Ok(compare(op, a.cmp(&b))),
        };
        match checked {
            Some(n) => Ok(Value::Int(n)),
            None => self.panic("a whole number overflowed", at),
        }
    }

    /// `(T)x`: a number made the type's kind; anything else as it is.
    pub(crate) fn cast(&self, v: Value, ty: &Type) -> Value {
        let float = [&words::f32::WORD, &words::f64::WORD, &words::decimal::WORD];
        match (&v, type_name(ty)) {
            (Value::Float(x), Some(n)) if !float.iter().any(|w| w.text == n) && is_number(n) => {
                Value::Int(*x as i64)
            }
            (Value::Int(i), Some(n)) if float.iter().any(|w| w.text == n) => {
                Value::Float(*i as f64)
            }
            _ => v,
        }
    }

    /// Whether `v` is a `ty` (an object of it or of a class inheriting it).
    pub(crate) fn is_type(&self, v: &Value, ty: &Type) -> bool {
        let Some(name) = type_name(ty) else {
            return false;
        };
        match v {
            Value::Object(o) => {
                let class = o.borrow().class.clone();
                self.program
                    .types
                    .get_key_value(class.as_str())
                    .is_some_and(|(k, _)| self.program.lineage(k).contains(&name))
                    || class == name
            }
            Value::Case(c) => c.of == name,
            Value::Str(_) => name == words::string::WORD.text,
            Value::Bool(_) => name == words::bool::WORD.text,
            Value::Int(_) | Value::Float(_) => is_number(name),
            _ => false,
        }
    }
}

fn type_name(ty: &Type) -> Option<&str> {
    match &ty.kind {
        TypeKind::Path { path, .. } => path.last().map(|n| n.text.as_str()),
        TypeKind::Nullable(inner) => type_name(inner),
        _ => None,
    }
}

fn is_number(name: &str) -> bool {
    [
        &words::i8::WORD,
        &words::i16::WORD,
        &words::i32::WORD,
        &words::i64::WORD,
        &words::u8::WORD,
        &words::u16::WORD,
        &words::u32::WORD,
        &words::u64::WORD,
        &words::f32::WORD,
        &words::f64::WORD,
        &words::decimal::WORD,
    ]
    .iter()
    .any(|w| w.text == name)
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Int(n) => *n as f64,
        Value::Float(x) => *x,
        _ => f64::NAN,
    }
}

fn floats(op: BinOp, a: f64, b: f64) -> Value {
    match op {
        BinOp::Add => Value::Float(a + b),
        BinOp::Sub => Value::Float(a - b),
        BinOp::Mul => Value::Float(a * b),
        BinOp::Div => Value::Float(a / b),
        BinOp::Rem => Value::Float(a % b),
        _ => a
            .partial_cmp(&b)
            .map_or(Value::Bool(false), |o| compare(op, o)),
    }
}

fn compare(op: BinOp, o: std::cmp::Ordering) -> Value {
    Value::Bool(match op {
        BinOp::Lt => o == std::cmp::Ordering::Less,
        BinOp::LtEq => o != std::cmp::Ordering::Greater,
        BinOp::Gt => o == std::cmp::Ordering::Greater,
        BinOp::GtEq => o != std::cmp::Ordering::Less,
        _ => false,
    })
}

/// `x in r`: a range's, a list's or a string's.
fn contains(r: &Value, x: &Value) -> bool {
    match (r, x) {
        (Value::Range(a, b, inclusive), Value::Int(n)) => {
            *n >= *a && (if *inclusive { *n <= *b } else { *n < *b })
        }
        (Value::List(items), x) => items.borrow().contains(x),
        (Value::Str(s), Value::Str(part)) => s.contains(&**part),
        (Value::Str(s), Value::Char(c)) => s.contains(*c),
        _ => false,
    }
}
