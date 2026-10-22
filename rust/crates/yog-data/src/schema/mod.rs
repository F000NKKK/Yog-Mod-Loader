//! Schemas: a format is a Rust type implementing [`Schema`] (usually
//! `#[derive(Schema)]`). One schema reads and writes the format, checks it
//! with the path of every problem, collects its references for the build to
//! check, and describes itself ([`Shape`]) for Yog-IDLE's editors and the
//! docs.

mod fields;
mod one_or_list;
mod plain;
mod reader;
mod shape;
mod typed_values;

pub use fields::{Fields, field_name};
pub use one_or_list::OneOrList;
pub use plain::*;
pub use reader::*;
pub use shape::*;
pub use typed_values::*;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::marker::PhantomData;

use crate::binary::{Decoder, Writer};
use crate::borrowed::{MapRef, ValueRef};
use crate::diagnostic::{Diagnostic, Errors};
use crate::frame::{Packer, Unpacker};
use crate::text::{self, Doc, LineCol};
use crate::typed;
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

    /// Writes itself in the binary form, straight — [`Schema::write`]'s
    /// value encoded, unless the type knows better (derived ones do).
    fn encode(&self, w: &mut Writer) {
        w.value(&self.write());
    }

    /// Reads itself from the binary form, straight; `None` when it isn't
    /// what [`Schema::encode`] writes — then [`read_bytes`] reads the slow
    /// way, which says what's wrong. Through a value unless the type knows
    /// better (derived ones do).
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        let arena = bumpalo::Bump::new();
        let v = d.value_ref(&arena).ok()?;
        read_ref_value(&v).ok()
    }

    /// Packs itself into a frame ([`crate::frame`]): its binary form as
    /// bytes, unless the type packs itself (derived ones and plain types do).
    fn pack(&self, p: &mut Packer) {
        p.put_tagged(self);
    }

    /// Reads itself out of a frame; `None` if the bytes aren't what
    /// [`Schema::pack`] writes.
    fn unpack(u: &mut Unpacker<'_>) -> Option<Self> {
        u.get_tagged()
    }

    /// A list of these packed: a count, then each one (floats raw).
    fn pack_list(items: &[Self], p: &mut Packer) {
        let at = p.begin_list(items.len());
        for item in items {
            item.pack(p);
        }
        p.end(at);
    }

    /// A list of these out of a frame.
    fn unpack_list(u: &mut Unpacker<'_>) -> Option<Vec<Self>> {
        let (n, end) = u.list()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(Self::unpack(u)?);
        }
        u.skip_to(end)?;
        Some(out)
    }

    /// A list of these written straight (numbers pack theirs).
    fn encode_list(items: &[Self], w: &mut Writer) {
        w.list(items.len());
        for item in items {
            item.encode(w);
        }
    }

    /// A list of these read straight.
    fn decode_list(d: &mut Decoder<'_>) -> Option<Vec<Self>> {
        let n = d.list()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(Self::decode(d)?);
        }
        d.leave();
        Some(out)
    }
}

/// Where a tagged enum's variant keeps what it wraps when that isn't a
/// struct (`Id(String)` is `{type: "id", value: "…"}`); a wrapped struct's
/// fields sit next to the tag instead.
pub const WRAPPED: &str = yog_ids::word::VALUE;

/// Whether a tagged enum's variant wrapping `T` lays `T`'s fields next to
/// the tag (a struct) or keeps `T` under [`WRAPPED`].
#[doc(hidden)]
pub fn flattens<T: Schema>() -> bool {
    matches!(T::shape(), Shape::Struct(_))
}

/// `v` in the binary form, written straight ([`Schema::encode`]).
pub fn to_bytes<T: Schema>(v: &T) -> Vec<u8> {
    let mut w = Writer::new();
    v.encode(&mut w);
    w.finish()
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
pub fn from_value<T: Schema>(v: &Value) -> Result<Read<T>, Vec<Diagnostic>> {
    let arena = bumpalo::Bump::new();
    read_with(&v.borrow(&arena), Reader::new())
}

/// Reads `v` as `T`, nothing else: what the game wants of content (the
/// references met aren't kept).
pub fn read_value<T: Schema>(v: &Value) -> Result<T, Vec<Diagnostic>> {
    let arena = bumpalo::Bump::new();
    read_ref_value(&v.borrow(&arena))
}

/// [`read_value`] of a borrowed value.
pub fn read_ref_value<T: Schema>(v: &ValueRef) -> Result<T, Vec<Diagnostic>> {
    let reader = Reader {
        value_only: true,
        ..Reader::new()
    };
    read_with(v, reader).map(|r| r.value)
}

/// Reads the binary form as `T`, borrowed all the way: no string is
/// copied but into `T` itself — content as the runtime takes it.
pub fn read_bytes<T: Schema>(bytes: &[u8]) -> Result<T, Vec<Diagnostic>> {
    // Straight, the usual case; the slow way says what's wrong.
    if let Ok(mut d) = Decoder::open(bytes)
        && let Some(v) = T::decode(&mut d)
        && d.at_end()
    {
        return Ok(v);
    }
    let arena = bumpalo::Bump::new();
    let v = crate::binary::decode_ref(bytes, &arena)
        .map_err(|e| vec![Diagnostic::error(yog_ids::data_problem::BAD_BINARY, e)])?;
    read_ref_value(&v)
}

/// `from_value`, references and texts going through `resolver`.
pub fn from_value_with<T: Schema>(
    v: &Value,
    resolver: &mut dyn Resolve,
) -> Result<Read<T>, Vec<Diagnostic>> {
    let arena = bumpalo::Bump::new();
    read_with(&v.borrow(&arena), Reader::with_resolver(resolver))
}

fn read_with<T: Schema>(v: &ValueRef, mut cx: Reader<'_>) -> Result<Read<T>, Vec<Diagnostic>> {
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

/// A table a crate ships (`include_str!` of its `.ydata`), read: what it
/// says, or — when it won't read, which its crate's tests catch — the
/// error logged (`what`: which table) and nothing.
pub fn built_in<T: Schema + Default>(what: &str, text: &str) -> T {
    match from_text::<T>(text) {
        Ok(read) => read.value,
        Err(e) => {
            yog_logging::error!("{what} won't read: {e}");
            T::default()
        }
    }
}

/// Parses `text` and reads it as `T`; problems come with their line and column.
pub fn from_text<T: Schema>(text: &str) -> Result<Read<T>, Errors> {
    let doc = text::parse(text)?;
    from_doc(text, &doc)
}

/// Parses TOML `text` and reads it as `T`.
#[cfg(feature = "toml")]
pub fn from_toml<T: Schema>(text: &str) -> Result<Read<T>, Errors> {
    let doc = crate::toml::parse(text)?;
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
    read: Result<Read<T>, Vec<Diagnostic>>,
) -> Result<Read<T>, Errors> {
    read.map_err(|problems| {
        let mut located: Vec<Diagnostic> =
            problems.into_iter().map(|p| placed(text, doc, p)).collect();
        // In file order, as an editor lists them.
        located.sort_by_key(|d| (d.line, d.col));
        Errors(located)
    })
}

/// `d` (about a value of `doc`, parsed from `text`) at its line and column:
/// its key's for one about the key (an unknown field).
pub fn placed(text: &str, doc: &Doc, d: Diagnostic) -> Diagnostic {
    let spans = doc.spans_at(&d.path);
    let span = match (d.on_key, spans.key) {
        (true, Some(key)) => key,
        _ => spans.span,
    };
    let at = LineCol::of(text, span.start as usize);
    d.at(at)
}

/// `value` as text, indented.
pub fn to_text<T: Schema>(value: &T) -> String {
    text::to_string_pretty(&value.write())
}
