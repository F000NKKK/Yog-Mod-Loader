//! Yog Data text (the `yog-data` crate: JSON plus comments, trailing commas
//! and unquoted keys) as `serde_json` values, for the readers here that
//! only look things up.

use serde_json::{Map, Value};

/// `v` as a `serde_json` value; typed values become their string forms.
pub fn to_json(v: &yog_data::Value) -> Value {
    match v {
        yog_data::Value::Null => Value::Null,
        yog_data::Value::Bool(b) => Value::Bool(*b),
        yog_data::Value::Int(n) => Value::from(*n),
        yog_data::Value::Float(f) => {
            serde_json::Number::from_f64(*f).map_or(Value::Null, Value::Number)
        }
        yog_data::Value::Str(s) => Value::String(s.clone()),
        yog_data::Value::List(items) => Value::Array(items.iter().map(to_json).collect()),
        yog_data::Value::Map(m) => {
            Value::Object(m.iter().map(|(k, v)| (k.to_string(), to_json(v))).collect())
        }
        typed => Value::String(yog_data::typed::typed_string(typed).unwrap_or_default()),
    }
}

/// `v` as Yog Data (plain values: no references or other typed strings).
pub fn from_json(v: &Value) -> yog_data::Value {
    match v {
        Value::Null => yog_data::Value::Null,
        Value::Bool(b) => yog_data::Value::Bool(*b),
        Value::Number(n) => n.as_i64().map_or_else(
            || yog_data::Value::Float(n.as_f64().unwrap_or(0.0)),
            yog_data::Value::Int,
        ),
        Value::String(s) => yog_data::Value::Str(s.clone()),
        Value::Array(items) => yog_data::Value::List(items.iter().map(from_json).collect()),
        Value::Object(m) => {
            let mut out = yog_data::Map::new();
            for (k, v) in m {
                out.insert(k.as_str(), from_json(v));
            }
            yog_data::Value::Map(out)
        }
    }
}

/// A Yog Data file that's one object, `None` if it isn't.
pub fn read_object(text: &str) -> Option<Map<String, Value>> {
    match to_json(&yog_data::text::parse_value(text).ok()?) {
        Value::Object(m) => Some(m),
        _ => None,
    }
}

/// `v` with its references as the runtime writes ids: `@item:x` → `x`,
/// `@tag:item:x` → `#x` (what yog-book's renderer and the hosts read).
pub fn runtime_ids(v: &mut Value) {
    match v {
        Value::String(s) if s.starts_with('@') => match yog_data::typed::classify(s) {
            Ok(yog_data::Value::Ref(r)) => *s = r.id,
            Ok(yog_data::Value::Tag(r)) => *s = format!("#{}", r.id),
            Ok(yog_data::Value::Str(plain)) => *s = plain,
            _ => {}
        },
        Value::Array(items) => items.iter_mut().for_each(runtime_ids),
        Value::Object(m) => m.values_mut().for_each(runtime_ids),
        _ => {}
    }
}

/// The value at `keys` of `v`, one map key after another.
pub fn at<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().try_fold(v, |v, k| v.get(*k))
}
