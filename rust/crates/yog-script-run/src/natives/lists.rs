//! A list's members.

use std::cell::RefCell;

use super::Native;
use crate::value::Value;

pub(super) fn member(items: &[Value], name: &str) -> Option<Value> {
    match name {
        "Count" => Some(Value::Int(i64::try_from(items.len()).unwrap_or(i64::MAX))),
        _ => None,
    }
}

pub(super) fn call(items: &RefCell<Vec<Value>>, name: &str, args: &[Value]) -> Option<Native> {
    let v = match (name, args) {
        ("Add", [v]) => {
            items.borrow_mut().push(v.clone());
            Value::Void
        }
        ("Contains", [v]) => Value::Bool(items.borrow().contains(v)),
        ("IndexOf", [v]) => Value::Int(
            items
                .borrow()
                .iter()
                .position(|x| x == v)
                .and_then(|k| i64::try_from(k).ok())
                .unwrap_or(-1),
        ),
        ("Clear", []) => {
            items.borrow_mut().clear();
            Value::Void
        }
        ("RemoveAt", [Value::Int(k)]) => {
            let mut items = items.borrow_mut();
            return Some(
                match usize::try_from(*k).ok().filter(|k| *k < items.len()) {
                    Some(k) => Ok(items.remove(k)),
                    None => Err(format!("index {k} out of range ({} items)", items.len())),
                },
            );
        }
        _ => return None,
    };
    Some(Ok(v))
}
