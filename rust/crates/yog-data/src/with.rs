//! Field codecs: `#[data(with = "…")]` reads and writes a field through a
//! type other than its own — like serde's `with`. The field keeps the plain
//! type a program uses (`String`), the format says what it holds (an item
//! reference, a text), checked and resolved by the [`Reader`]'s
//! [`Resolve`](crate::schema::Resolve).
//!
//! ```ignore
//! #[derive(Schema)]
//! struct Spotlight {
//!     /// `@item:` or `@block:`, kept as the plain id.
//!     #[data(with = "RefTo<(kinds::Item, kinds::Block)>")]
//!     item: String,
//!     /// A text, a `.yres` text or a name, kept resolved.
//!     #[data(with = "Opt<TextOf>")]
//!     text: Option<String>,
//! }
//! ```

use std::collections::BTreeMap;
use std::marker::PhantomData;

use crate::borrowed::ValueRef;
use crate::schema::{Kinds, Reader, Schema, Shape, Text, is_metadata_key, read_ref};
use crate::value::{Map, Ref, Seg, Value};

/// How a field of type `F` is read and written.
pub trait With<F> {
    fn shape() -> Shape;
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<F>;
    fn write(f: &F) -> Value;
    /// The field's value when it isn't there (`None`: it's required).
    fn absent() -> Option<F> {
        None
    }
    /// Whether a struct leaves the field out.
    fn skip(_f: &F) -> bool {
        false
    }
}

/// A reference to content of kind(s) `K`, kept as the resolved id.
pub struct RefTo<K>(PhantomData<K>);

impl<K: Kinds> With<String> for RefTo<K> {
    fn shape() -> Shape {
        Shape::Ref(K::NAMES)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        // The usual case, a reference of a kind it takes: nothing copied but its id.
        if let ValueRef::Ref { kind, id } = v
            && (K::NAMES.is_empty() || K::NAMES.contains(kind))
        {
            if cx.keeps_found() || cx.resolves() {
                let r = Ref::new(*kind, *id);
                if cx.keeps_found() {
                    cx.note_ref(r.clone());
                }
                return cx.take_ref(r);
            }
            return Some(id.to_string());
        }
        let r = read_ref(v, cx, K::NAMES)?;
        cx.take_ref(r)
    }
    fn write(f: &String) -> Value {
        Value::Ref(Ref::new(
            K::NAMES.first().copied().unwrap_or(yog_ids::kind::ITEM),
            f.clone(),
        ))
    }
}

/// A text ([`Text`]: written, a `.yres` text, or a name), kept resolved.
pub struct TextOf;

impl With<String> for TextOf {
    fn shape() -> Shape {
        Shape::Text
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        let t = Text::read(v, cx)?;
        cx.take_text(t)
    }
    fn write(f: &String) -> Value {
        Value::Text(f.clone())
    }
}

/// An optional field through `W`.
pub struct Opt<W>(PhantomData<W>);

impl<F, W: With<F>> With<Option<F>> for Opt<W> {
    fn shape() -> Shape {
        Shape::Optional(W::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Option<F>> {
        if v.is_null() {
            Some(None)
        } else {
            W::read(v, cx).map(Some)
        }
    }
    fn write(f: &Option<F>) -> Value {
        f.as_ref().map_or(Value::Null, W::write)
    }
    fn absent() -> Option<Option<F>> {
        Some(None)
    }
    fn skip(f: &Option<F>) -> bool {
        f.is_none()
    }
}

/// A list, each item through `W`.
pub struct List<W>(PhantomData<W>);

impl<F, W: With<F>> With<Vec<F>> for List<W> {
    fn shape() -> Shape {
        Shape::List(W::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Vec<F>> {
        let ValueRef::List(items) = v else {
            cx.expected("a list", v);
            return None;
        };
        let mut out = Vec::with_capacity(items.len());
        let mut ok = true;
        for (i, item) in items.iter().enumerate() {
            match cx.at(Seg::Index(i), |cx| W::read(item, cx)) {
                Some(x) => out.push(x),
                None => ok = false,
            }
        }
        ok.then_some(out)
    }
    fn write(f: &Vec<F>) -> Value {
        Value::List(f.iter().map(W::write).collect())
    }
}

/// An object of any keys, each value through `W`.
pub struct Values<W>(PhantomData<W>);

impl<F, W: With<F>> With<BTreeMap<String, F>> for Values<W> {
    fn shape() -> Shape {
        Shape::Map(W::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<BTreeMap<String, F>> {
        let map = cx.map(v)?;
        let mut out = BTreeMap::new();
        let mut ok = true;
        for (k, item) in map.iter().filter(|(k, _)| !is_metadata_key(k)) {
            match cx.at(Seg::Key(k.to_string()), |cx| W::read(item, cx)) {
                Some(x) => {
                    out.insert(k.to_string(), x);
                }
                None => ok = false,
            }
        }
        ok.then_some(out)
    }
    fn write(f: &BTreeMap<String, F>) -> Value {
        Value::Map(
            f.iter()
                .map(|(k, v)| (k.clone(), W::write(v)))
                .collect::<Map>(),
        )
    }
}

/// The field's own schema — `with` spelled out (`List<Plain>` for a `Vec<T: Schema>`).
pub struct Plain;

impl<F: Schema> With<F> for Plain {
    fn shape() -> Shape {
        F::shape()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<F> {
        F::read(v, cx)
    }
    fn write(f: &F) -> Value {
        f.write()
    }
    fn absent() -> Option<F> {
        F::absent()
    }
    fn skip(f: &F) -> bool {
        f.skip()
    }
}

/// Bytes as they are (`Vec<u8>`): a payload a format doesn't look into.
pub struct Raw;

impl With<Vec<u8>> for Raw {
    fn shape() -> Shape {
        Shape::Bytes
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Vec<u8>> {
        match v {
            ValueRef::Bytes(b) => Some(b.to_vec()),
            other => {
                cx.expected("bytes", other);
                None
            }
        }
    }
    fn write(f: &Vec<u8>) -> Value {
        Value::Bytes(f.clone())
    }
    fn absent() -> Option<Vec<u8>> {
        Some(Vec::new())
    }
}
