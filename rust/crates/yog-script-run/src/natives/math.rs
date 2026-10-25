//! `Math`.

use yog_script_ids::members::{ABS, CLAMP, FLOOR, MAX, MIN, ROUND, SQRT};
use yog_script_ids::system::MATH;

use super::Native;
use crate::value::Value;

pub(super) fn call(of: &str, name: &str, args: &[Value]) -> Option<Native> {
    if of != MATH {
        return None;
    }
    let v = match (name, args) {
        (MAX, [Value::Int(a), Value::Int(b)]) => Value::Int(*a.max(b)),
        (MIN, [Value::Int(a), Value::Int(b)]) => Value::Int(*a.min(b)),
        (ABS, [Value::Int(a)]) => Value::Int(a.abs()),
        (CLAMP, [Value::Int(x), Value::Int(lo), Value::Int(hi)]) if lo <= hi => {
            Value::Int(*x.clamp(lo, hi))
        }
        (MAX, [a, b]) => Value::Float(num(a)?.max(num(b)?)),
        (MIN, [a, b]) => Value::Float(num(a)?.min(num(b)?)),
        (SQRT, [a]) => Value::Float(num(a)?.sqrt()),
        (FLOOR, [a]) => Value::Float(num(a)?.floor()),
        (ROUND, [a]) => Value::Float(num(a)?.round()),
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
