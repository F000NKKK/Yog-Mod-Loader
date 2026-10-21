//! Building an imported content file's value: its fields in the order
//! they're set, numbers whole where they are (`16`, not `16.0`).

pub(super) use yog_data::Value as Data;

/// Whole numbers below this are written as integers.
const WHOLE_BELOW: f64 = 1e15;

pub(super) fn obj<K: Into<String>>(fields: impl IntoIterator<Item = (K, Data)>) -> Data {
    Data::Map(fields.into_iter().collect())
}

pub(super) fn number(n: f64) -> Data {
    if n.fract() == 0.0 && n.abs() < WHOLE_BELOW {
        Data::Int(n as i64)
    } else {
        Data::Float(n)
    }
}

pub(super) fn string(s: impl Into<String>) -> Data {
    Data::Str(s.into())
}

pub(super) fn nums(ns: &[f64]) -> Data {
    Data::List(ns.iter().map(|n| number(*n)).collect())
}

/// An object's fields, changed.
pub(super) trait Fields {
    /// Sets `key` (in place if it's there, else last).
    fn set(&mut self, key: &str, value: Data);
    /// Whether it has any.
    fn has_fields(&self) -> bool;
    /// Leaves `key` out.
    fn without(&mut self, key: &str);
}

impl Fields for Data {
    fn set(&mut self, key: &str, value: Data) {
        if let Data::Map(m) = self {
            m.insert(key, value);
        }
    }

    fn has_fields(&self) -> bool {
        self.as_map().is_some_and(|m| !m.is_empty())
    }

    fn without(&mut self, key: &str) {
        if let Data::Map(m) = self {
            m.remove(key);
        }
    }
}
