//! `Math`.

use super::Native;
use crate::value::Value;

/// The library's numbers' class.
const MATH: &str = "Math";

pub(super) fn call(of: &str, name: &str, args: &[Value]) -> Option<Native> {
    if of != MATH {
        return None;
    }
    let v = match (name, args) {
        ("Max", [Value::Int(a), Value::Int(b)]) => Value::Int(*a.max(b)),
        ("Min", [Value::Int(a), Value::Int(b)]) => Value::Int(*a.min(b)),
        ("Abs", [Value::Int(a)]) => Value::Int(a.abs()),
        ("Clamp", [Value::Int(x), Value::Int(lo), Value::Int(hi)]) if lo <= hi => {
            Value::Int(*x.clamp(lo, hi))
        }
        ("Max", [a, b]) => Value::Float(num(a)?.max(num(b)?)),
        ("Min", [a, b]) => Value::Float(num(a)?.min(num(b)?)),
        ("Sqrt", [a]) => Value::Float(num(a)?.sqrt()),
        ("Floor", [a]) => Value::Float(num(a)?.floor()),
        ("Round", [a]) => Value::Float(num(a)?.round()),
        _ => return None,
    };
    Some(Ok(v))
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Float(x) => Some(*x),
        _ => None,
    }
}
