//! Yog Data files as the designers look things up in them (their typed
//! strings as written: the schema reading them isn't run here).

use yog_data::Value;

/// A Yog Data file that's one object (a map), `None` if it isn't.
pub fn read_object(text: &str) -> Option<Value> {
    yog_data::text::parse_value(text)
        .ok()
        .filter(|v| matches!(v, Value::Map(_)))
}

/// `v` with its references as the runtime writes ids: `@item:x` → `x`,
/// `@tag:item:x` → `#x` (what yog-book's renderer and the hosts read).
pub fn runtime_ids(v: &mut Value) {
    match v {
        Value::Str(s) if s.starts_with('@') => match yog_data::typed::classify(s) {
            Ok(Value::Ref(r)) => *s = r.id,
            Ok(Value::Tag(r)) => *s = format!("#{}", r.id),
            Ok(Value::Str(plain)) => *s = plain,
            _ => {}
        },
        Value::List(items) => items.iter_mut().for_each(runtime_ids),
        Value::Map(m) => {
            let keys: Vec<String> = m.keys().map(str::to_string).collect();
            for k in keys {
                if let Some(x) = m.get_mut(&k) {
                    runtime_ids(x);
                }
            }
        }
        _ => {}
    }
}

/// The value at `keys` of `v`, one map key after another.
pub fn at<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().try_fold(v, |v, k| v.get(k))
}

/// A file of JSON (the game's) or Yog Data text as a value.
pub fn read_bytes(bytes: &[u8]) -> Option<Value> {
    yog_data::text::parse_value(std::str::from_utf8(bytes).ok()?).ok()
}
