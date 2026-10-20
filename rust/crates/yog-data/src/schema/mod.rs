//! Schemas: a format is a Rust type implementing [`Schema`] (usually
//! `#[derive(Schema)]`). One schema reads and writes the format, checks it
//! with the path of every problem, collects its references for the build to
//! check, and describes itself ([`Shape`]) for Yog-IDLE's editors and the
//! docs.

mod plain;
mod reader;
mod shape;
mod typed_values;

pub use plain::*;
pub use reader::*;
pub use shape::*;
pub use typed_values::*;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::marker::PhantomData;

use crate::text::{self, Doc, LineCol};
use crate::typed;
use crate::borrowed::{MapRef, ValueRef};
use crate::value::{Map, Path, Ref, Res, Seg, Value};

/// A Yog Data format.
pub trait Schema: Sized {
    /// What the format looks like.
    fn shape() -> Shape;

    /// Reads `v`. Problems go to `cx` at its current path; `None` when the
    /// value couldn't be read (the problem is in `cx`).
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self>;

    /// The value, as the format writes it.
    fn write(&self) -> Value;

    /// The value of a field that isn't there — `None`: the field is required.
    fn absent() -> Option<Self> {
        None
    }

    /// Whether a struct leaves this field out when writing it.
    fn skip(&self) -> bool {
        false
    }
}

/// The field names of a struct's shape (none for anything else).
pub fn field_names(shape: &Shape) -> Vec<&'static str> {
    match shape {
        Shape::Struct(s) => s.fields.iter().map(|f| f.name).collect(),
        _ => Vec::new(),
    }
}

/// `A` and `B` read from the same object — each allowing the other's
/// fields, like serde's `flatten`: a recipe's common fields and its kind's.
pub fn read_both<A: Schema, B: Schema>(
    v: &ValueRef,
    cx: &mut Reader,
    also: &[&'static str],
) -> Option<(A, B)> {
    let (an, bn) = (field_names(&A::shape()), field_names(&B::shape()));
    let mut b_keys = bn.clone();
    b_keys.extend_from_slice(also);
    let mut a_keys = an;
    a_keys.extend_from_slice(also);
    let a = cx.with_allowed_all(&b_keys, |cx| A::read(v, cx));
    let b = cx.with_allowed_all(&a_keys, |cx| B::read(v, cx));
    Some((a?, b?))
}

/// Reads `v` as `T`: the value and its references, or every problem.
pub fn from_value<T: Schema>(v: &Value) -> Result<Read<T>, Vec<Problem>> {
    let arena = bumpalo::Bump::new();
    read_with(&v.borrow(&arena), Reader::new())
}

/// Reads `v` as `T`, nothing else: what the game wants of content (the
/// references met aren't kept).
pub fn read_value<T: Schema>(v: &Value) -> Result<T, Vec<Problem>> {
    let arena = bumpalo::Bump::new();
    read_ref_value(&v.borrow(&arena))
}

/// [`read_value`] of a borrowed value.
pub fn read_ref_value<T: Schema>(v: &ValueRef) -> Result<T, Vec<Problem>> {
    let reader = Reader { value_only: true, ..Reader::new() };
    read_with(v, reader).map(|r| r.value)
}

/// Reads the binary form as `T`, borrowed all the way: no string is
/// copied but into `T` itself — content as the runtime takes it.
pub fn read_bytes<T: Schema>(bytes: &[u8]) -> Result<T, Vec<Problem>> {
    let arena = bumpalo::Bump::new();
    let v = crate::binary::decode_ref(bytes, &arena).map_err(|e| {
        vec![Problem { path: Path::root(), message: e.to_string(), at_key: false }]
    })?;
    read_ref_value(&v)
}

/// `from_value`, references and texts going through `resolver`.
pub fn from_value_with<T: Schema>(
    v: &Value,
    resolver: &mut dyn Resolve,
) -> Result<Read<T>, Vec<Problem>> {
    let arena = bumpalo::Bump::new();
    read_with(&v.borrow(&arena), Reader::with_resolver(resolver))
}

fn read_with<T: Schema>(v: &ValueRef, mut cx: Reader<'_>) -> Result<Read<T>, Vec<Problem>> {
    let value = T::read(v, &mut cx);
    match value {
        Some(value) if cx.problems.is_empty() => Ok(Read {
            value,
            refs: cx.refs,
            tags: cx.tags,
            resources: cx.resources,
        }),
        _ => Err(cx.problems),
    }
}

/// Parses `text` and reads it as `T`; problems come with their line and column.
pub fn from_text<T: Schema>(text: &str) -> Result<Read<T>, Errors> {
    let doc = text::parse(text).map_err(|e| {
        Errors(vec![Located {
            at: Some(e.at),
            path: Path::root(),
            message: e.message,
        }])
    })?;
    from_doc(text, &doc)
}

/// Parses TOML `text` and reads it as `T`.
#[cfg(feature = "toml")]
pub fn from_toml<T: Schema>(text: &str) -> Result<Read<T>, Errors> {
    let doc = crate::toml::parse(text).map_err(|e| {
        Errors(vec![Located {
            at: Some(e.at),
            path: Path::root(),
            message: e.message,
        }])
    })?;
    from_doc(text, &doc)
}

/// Reads a parsed document as `T`; `text` is its source, for positions.
pub fn from_doc<T: Schema>(text: &str, doc: &Doc) -> Result<Read<T>, Errors> {
    locate(text, doc, from_value(&doc.value))
}

/// `from_doc`, references and texts going through `resolver`.
pub fn from_doc_with<T: Schema>(
    text: &str,
    doc: &Doc,
    resolver: &mut dyn Resolve,
) -> Result<Read<T>, Errors> {
    locate(text, doc, from_value_with(&doc.value, resolver))
}

fn locate<T>(
    text: &str,
    doc: &Doc,
    read: Result<Read<T>, Vec<Problem>>,
) -> Result<Read<T>, Errors> {
    read.map_err(|problems| {
        let mut located: Vec<(u32, Located)> = problems
            .into_iter()
            .map(|p| {
                let spans = doc.spans_at(&p.path);
                let span = if p.at_key {
                    spans.key.unwrap_or(spans.span)
                } else {
                    spans.span
                };
                (
                    span.start,
                    Located {
                        at: Some(LineCol::of(text, span.start as usize)),
                        path: p.path,
                        message: p.message,
                    },
                )
            })
            .collect();
        // In file order, as an editor lists them.
        located.sort_by_key(|(offset, _)| *offset);
        Errors(located.into_iter().map(|(_, l)| l).collect())
    })
}

/// `value` as text, indented.
pub fn to_text<T: Schema>(value: &T) -> String {
    text::to_string_pretty(&value.write())
}
