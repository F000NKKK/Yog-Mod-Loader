//! A string's members.

use super::Native;
use crate::value::Value;

pub(super) fn member(s: &str, name: &str) -> Option<Value> {
    match name {
        "Length" => Some(Value::Int(
            i64::try_from(s.chars().count()).unwrap_or(i64::MAX),
        )),
        _ => None,
    }
}

pub(super) fn call(s: &str, name: &str, args: &[Value]) -> Option<Native> {
    let text = |k: usize| args.get(k).map(ToString::to_string).unwrap_or_default();
    let v = match (name, args.len()) {
        ("ToUpper", 0) => Value::str(&s.to_uppercase()),
        ("ToLower", 0) => Value::str(&s.to_lowercase()),
        ("Trim", 0) => Value::str(s.trim()),
        ("StartsWith", 1) => Value::Bool(s.starts_with(&text(0))),
        ("EndsWith", 1) => Value::Bool(s.ends_with(&text(0))),
        ("Contains", 1) => Value::Bool(s.contains(&text(0))),
        ("Substring", 1 | 2) => return Some(substring(s, args)),
        _ => return None,
    };
    Some(Ok(v))
}

/// `s.Substring(from)` / `s.Substring(from, length)`, by characters.
fn substring(s: &str, args: &[Value]) -> Native {
    let at = |k: usize| match args.get(k) {
        Some(Value::Int(n)) => usize::try_from(*n).ok(),
        _ => None,
    };
    let chars: Vec<char> = s.chars().collect();
    let from = at(0).ok_or("a substring starts at a whole number from 0")?;
    let len = if args.len() > 1 {
        at(1).ok_or("a substring's length is a whole number from 0")?
    } else {
        chars.len().saturating_sub(from)
    };
    chars
        .get(from..from + len)
        .map(|c| Value::str(&c.iter().collect::<String>()))
        .ok_or_else(|| {
            format!(
                "{from}..{} is out of the string's {} characters",
                from + len,
                chars.len()
            )
        })
}
