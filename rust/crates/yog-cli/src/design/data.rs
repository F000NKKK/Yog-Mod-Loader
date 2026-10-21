//! Yog Data text (the `yog-data` crate: JSON plus comments, trailing commas
//! and unquoted keys) as `serde_json` values, for the readers here that
//! only look things up.

use serde_json::{Map, Value};

pub use yog_data::json::{from_json, to_json};

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
