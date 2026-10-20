//! The schemas of plain types: numbers, strings, lists, maps.

#[allow(unused_imports)] // each part uses some of the module's names
use super::*;

/// What a typed value's string starts with (`@item:…`).
fn typed_mark() -> char {
    yog_ids::sep::TYPED.chars().next().unwrap_or_default()
}

// ── Plain types ─────────────────────────────────────────────────────────────

impl Schema for Value {
    fn shape() -> Shape {
        Shape::Any
    }
    fn read(v: &ValueRef<'_>, _: &mut Reader) -> Option<Self> {
        Some(v.to_value())
    }
    fn write(&self) -> Value {
        self.clone()
    }
}

impl Schema for bool {
    fn shape() -> Shape {
        Shape::Bool
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        v.as_bool().or_else(|| {
            cx.expected("true or false", v);
            None
        })
    }
    fn write(&self) -> Value {
        Value::Bool(*self)
    }
    fn encode(&self, w: &mut Writer) {
        w.bool(*self);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        d.bool()
    }
}

macro_rules! int_schema {
    ($($t:ty),*) => {$(
        impl Schema for $t {
            fn shape() -> Shape {
                Shape::Int { min: <$t>::MIN as i64, max: (<$t>::MAX as u64).min(i64::MAX as u64) as i64 }
            }
            fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
                let Some(n) = v.as_i64() else {
                    cx.expected("an integer", v);
                    return None;
                };
                <$t>::try_from(n).ok().or_else(|| {
                    cx.problem(format!("{n} is out of range ({}..={})", <$t>::MIN, <$t>::MAX));
                    None
                })
            }
            fn write(&self) -> Value {
                Value::Int(*self as i64)
            }
            fn encode(&self, w: &mut Writer) {
                w.int(*self as i64);
            }
            fn decode(d: &mut Decoder<'_>) -> Option<Self> {
                d.int().and_then(|n| <$t>::try_from(n).ok())
            }
            fn encode_list(items: &[Self], w: &mut Writer) {
                w.ints(items.iter().map(|&n| n as i64));
            }
            fn decode_list(d: &mut Decoder<'_>) -> Option<Vec<Self>> {
                let n = d.ints()?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(<$t>::try_from(d.packed_int()?).ok()?);
                }
                Some(out)
            }
        }
    )*};
}
int_schema!(i8, i16, i32, i64, u8, u16, u32, u64, usize);

macro_rules! float_schema {
    ($($t:ty => $one:ident, $many:ident, $read_many:ident);*) => {$(
        impl Schema for $t {
            fn shape() -> Shape {
                Shape::Float
            }
            fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
                v.as_f64().map(|f| f as $t).or_else(|| {
                    cx.expected("a number", v);
                    None
                })
            }
            fn write(&self) -> Value {
                Value::Float(*self as f64)
            }
            fn encode(&self, w: &mut Writer) {
                w.$one(*self);
            }
            fn decode(d: &mut Decoder<'_>) -> Option<Self> {
                d.float().map(|f| f as $t)
            }
            fn encode_list(items: &[Self], w: &mut Writer) {
                w.$many(items);
            }
            fn decode_list(d: &mut Decoder<'_>) -> Option<Vec<Self>> {
                d.$read_many()
            }
        }
    )*};
}
float_schema!(f32 => float32, floats32, floats32; f64 => float, floats, floats);

impl Schema for String {
    fn shape() -> Shape {
        Shape::Str
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        match v {
            // Plain text, the usual case: copied once, into the result.
            ValueRef::Str(s) if !s.starts_with('@') => Some(s.to_string()),
            ValueRef::Str(s) => match typed::classify(s) {
                Ok(Value::Str(s)) => Some(s),
                Ok(other) => {
                    cx.problem(format!(
                        "expected plain text, found {} (write `@@` for a literal `@`)",
                        other.kind_name()
                    ));
                    None
                }
                Err(e) => {
                    cx.problem(e);
                    None
                }
            },
            other => {
                cx.expected("a string", other);
                None
            }
        }
    }
    fn write(&self) -> Value {
        Value::Str(self.clone())
    }
    fn encode(&self, w: &mut Writer) {
        w.str(self);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        // A typed value (`@…`) is the slow way's to tell apart.
        d.str().filter(|s| !s.starts_with(typed_mark())).map(str::to_owned)
    }
}

impl<T: Schema> Schema for Option<T> {
    fn shape() -> Shape {
        Shape::Optional(T::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        if v.is_null() {
            Some(None)
        } else {
            T::read(v, cx).map(Some)
        }
    }
    fn write(&self) -> Value {
        self.as_ref().map_or(Value::Null, T::write)
    }
    fn encode(&self, w: &mut Writer) {
        match self {
            Some(v) => v.encode(w),
            None => {
                w.null();
            }
        }
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        if d.null() { Some(None) } else { T::decode(d).map(Some) }
    }
    fn absent() -> Option<Self> {
        Some(None)
    }
    fn skip(&self) -> bool {
        self.is_none()
    }
}

impl<T: Schema> Schema for Box<T> {
    fn shape() -> Shape {
        T::shape()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        T::read(v, cx).map(Box::new)
    }
    fn write(&self) -> Value {
        (**self).write()
    }
    fn encode(&self, w: &mut Writer) {
        (**self).encode(w);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        T::decode(d).map(Box::new)
    }
    fn absent() -> Option<Self> {
        T::absent().map(Box::new)
    }
    fn skip(&self) -> bool {
        (**self).skip()
    }
}

impl<T: Schema> Schema for Vec<T> {
    fn shape() -> Shape {
        Shape::List(T::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let ValueRef::List(items) = v else {
            cx.expected("a list", v);
            return None;
        };
        let mut out = Vec::with_capacity(items.len());
        let mut ok = true;
        for (i, item) in items.iter().enumerate() {
            match cx.at(Seg::Index(i), |cx| T::read(item, cx)) {
                Some(x) => out.push(x),
                None => ok = false,
            }
        }
        ok.then_some(out)
    }
    fn write(&self) -> Value {
        Value::List(self.iter().map(T::write).collect())
    }
    fn encode(&self, w: &mut Writer) {
        T::encode_list(self, w);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        T::decode_list(d)
    }
}

/// A string that won't change: a pointer and a length, no spare capacity
/// (`String`'s third word) — what big tables of names keep.
impl Schema for Box<str> {
    fn shape() -> Shape {
        Shape::Str
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        String::read(v, cx).map(String::into_boxed_str)
    }
    fn write(&self) -> Value {
        Value::Str(self.to_string())
    }
    fn encode(&self, w: &mut Writer) {
        w.str(self);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        String::decode(d).map(String::into_boxed_str)
    }
}

/// A string shared without copying.
impl Schema for std::sync::Arc<str> {
    fn shape() -> Shape {
        Shape::Str
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        String::read(v, cx).map(Into::into)
    }
    fn write(&self) -> Value {
        Value::Str(self.to_string())
    }
    fn encode(&self, w: &mut Writer) {
        w.str(self);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        String::decode(d).map(Into::into)
    }
}

/// A list that won't change: no spare capacity.
impl<T: Schema> Schema for Box<[T]> {
    fn shape() -> Shape {
        Shape::List(T::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        Vec::<T>::read(v, cx).map(Vec::into_boxed_slice)
    }
    fn write(&self) -> Value {
        Value::List(self.iter().map(T::write).collect())
    }
    fn encode(&self, w: &mut Writer) {
        T::encode_list(self, w);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        T::decode_list(d).map(Vec::into_boxed_slice)
    }
}

impl<T: Schema, const N: usize> Schema for [T; N] {
    fn shape() -> Shape {
        Shape::List(T::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let items = Vec::<T>::read(v, cx)?;
        let len = items.len();
        items.try_into().ok().or_else(|| {
            cx.problem(format!("expected {N} items, found {len}"));
            None
        })
    }
    fn write(&self) -> Value {
        Value::List(self.iter().map(T::write).collect())
    }
    fn encode(&self, w: &mut Writer) {
        T::encode_list(self, w);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        T::decode_list(d)?.try_into().ok()
    }
}

/// Nothing: what a call answers when it only says it's done.
impl Schema for () {
    fn shape() -> Shape {
        Shape::Any
    }
    fn read(_v: &ValueRef<'_>, _cx: &mut Reader) -> Option<Self> {
        Some(())
    }
    fn write(&self) -> Value {
        Value::Null
    }
}

/// What may fail: `{ "ok": T }` or `{ "err": E }`.
impl<T: Schema, E: Schema> Schema for Result<T, E> {
    fn shape() -> Shape {
        Shape::Any
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let ValueRef::Map(m) = v else {
            cx.expected("{ ok } or { err }", v);
            return None;
        };
        if let Some(ok) = m.get(RESULT_OK) {
            return cx.at(Seg::Key(RESULT_OK.into()), |cx| T::read(ok, cx)).map(Ok);
        }
        let Some(err) = m.get(RESULT_ERR) else {
            cx.expected("{ ok } or { err }", v);
            return None;
        };
        cx.at(Seg::Key(RESULT_ERR.into()), |cx| E::read(err, cx)).map(Err)
    }
    fn write(&self) -> Value {
        let (key, value) = match self {
            Ok(t) => (RESULT_OK, t.write()),
            Err(e) => (RESULT_ERR, e.write()),
        };
        Value::Map([(key.to_owned(), value)].into_iter().collect())
    }
}

/// The keys of a [`Result`]'s two sides.
const RESULT_OK: &str = "ok";
const RESULT_ERR: &str = "err";

/// A pair, written `[a, b]` (a point, a size).
impl<A: Schema, B: Schema> Schema for (A, B) {
    fn shape() -> Shape {
        Shape::List(Value::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let ValueRef::List(items) = v else {
            cx.expected("a pair [a, b]", v);
            return None;
        };
        if items.len() != 2 {
            cx.problem(format!("expected 2 items, found {}", items.len()));
            return None;
        }
        let a = cx.at(Seg::Index(0), |cx| A::read(&items[0], cx));
        let b = cx.at(Seg::Index(1), |cx| B::read(&items[1], cx));
        Some((a?, b?))
    }
    fn write(&self) -> Value {
        Value::List(vec![self.0.write(), self.1.write()])
    }
}

fn read_map<T: Schema>(v: &ValueRef, cx: &mut Reader, mut put: impl FnMut(String, T)) -> Option<()> {
    let map = cx.map(v)?;
    let mut ok = true;
    for (k, item) in map.iter() {
        if is_metadata_key(k) {
            continue;
        }
        match cx.at(Seg::Key(k.to_string()), |cx| T::read(item, cx)) {
            Some(x) => put(k.to_string(), x),
            None => ok = false,
        }
    }
    ok.then_some(())
}

impl<T: Schema> Schema for BTreeMap<String, T> {
    fn shape() -> Shape {
        Shape::Map(T::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let mut out = BTreeMap::new();
        read_map(v, cx, |k, x| {
            out.insert(k, x);
        })?;
        Some(out)
    }
    fn write(&self) -> Value {
        Value::Map(self.iter().map(|(k, v)| (k.clone(), v.write())).collect())
    }
}

impl<T: Schema> Schema for HashMap<String, T> {
    fn shape() -> Shape {
        Shape::Map(T::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let mut out = HashMap::new();
        read_map(v, cx, |k, x| {
            out.insert(k, x);
        })?;
        Some(out)
    }
    fn write(&self) -> Value {
        let mut keys: Vec<&String> = self.keys().collect();
        keys.sort();
        Value::Map(
            keys.into_iter()
                .map(|k| (k.clone(), self[k].write()))
                .collect(),
        )
    }
}

/// An object kept in file order, values of one schema.
#[derive(Debug, Clone, PartialEq)]
pub struct Ordered<T>(pub Vec<(String, T)>);

impl<T> Default for Ordered<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: Schema> Schema for Ordered<T> {
    fn shape() -> Shape {
        Shape::Map(T::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let mut out = Vec::new();
        read_map(v, cx, |k, x| out.push((k, x)))?;
        Some(Ordered(out))
    }
    fn write(&self) -> Value {
        Value::Map(self.0.iter().map(|(k, v)| (k.clone(), v.write())).collect())
    }
}
