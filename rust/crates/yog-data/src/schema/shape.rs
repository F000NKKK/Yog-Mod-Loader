//! Shapes: what a format looks like.

use super::{Map, Value};
use yog_ids::shape_kind as kind;

// ── Shapes ──────────────────────────────────────────────────────────────────

/// A shape, built on demand — so shapes of recursive types are fine.
pub type ShapeFn = fn() -> Shape;

/// What a format looks like: drives Yog-IDLE's property grids and pickers,
/// completion, hover docs and the docs' field tables.
#[derive(Debug, Clone)]
pub enum Shape {
    /// Any value.
    Any,
    Bool,
    Int {
        min: i64,
        max: i64,
    },
    /// A number with a fraction, `bits` wide (32 or 64): how a frame packs it.
    Float {
        bits: u32,
    },
    /// Plain text.
    Str,
    /// A name that repeats ([`crate::Atom`]): text everywhere, its number in a frame.
    Atom,
    /// Yog text, a `@Set.key` resource, or `@kind:id` (that content's name).
    Text,
    /// An image source or a `@Set.key` image resource.
    Image,
    /// `#RRGGBB`/`#AARRGGBB` or a `@Set.key` color resource.
    Color,
    /// Bytes as they are.
    Bytes,
    /// A mod resource, `@Set.key`.
    Res,
    /// Content of a kind (`""`: any kind, written `@kind:id`).
    Ref(&'static [&'static str]),
    /// A tag of content of a kind.
    Tag(&'static [&'static str]),
    /// Content of a kind or a tag of it.
    RefOrTag(&'static [&'static str]),
    /// Content of a kind (`@item:…`) or an image source.
    RefOrImage(&'static [&'static str]),
    List(ShapeFn),
    /// An object of any keys, every value this shape.
    Map(ShapeFn),
    /// A value or null.
    Optional(ShapeFn),
    Struct(StructShape),
    Enum(EnumShape),
}

#[derive(Debug, Clone)]
pub struct StructShape {
    pub name: &'static str,
    pub doc: &'static str,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: &'static str,
    pub doc: &'static str,
    pub shape: ShapeFn,
    /// Must be written (no default).
    pub required: bool,
    /// The numbers it holds must be in `min..=max` (`#[data(range = "1..=99")]`).
    pub range: Option<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct EnumShape {
    pub name: &'static str,
    pub doc: &'static str,
    /// The key naming the variant (`"type"`), `None` for an enum of names.
    pub tag: Option<&'static str>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: &'static str,
    pub doc: &'static str,
    /// The variant's fields (none for a name).
    pub fields: Vec<Field>,
}

mod key;

/// A description's map of `kind`.
fn of_kind(k: &str) -> Map {
    let mut m = Map::new();
    m.insert(key::KIND, Value::from(k));
    m
}

/// `doc` under its key, unless it's empty.
fn put_doc(m: &mut Map, doc: &str) {
    if !doc.is_empty() {
        m.insert(key::DOC, Value::from(doc));
    }
}

/// A reference to a struct or enum in `defs`.
fn def_ref(name: &str) -> Map {
    let mut m = of_kind(kind::DEF);
    m.insert(key::NAME, Value::from(name));
    m
}

fn describe_fields(fs: &[Field], defs: &mut Map) -> Value {
    Value::List(
        fs.iter()
            .map(|f| {
                let mut m = Map::new();
                m.insert(key::NAME, Value::from(f.name));
                put_doc(&mut m, f.doc);
                m.insert(key::SHAPE, (f.shape)().describe_into(defs));
                m.insert(key::REQUIRED, Value::Bool(f.required));
                if let Some((lo, hi)) = f.range {
                    m.insert(
                        key::RANGE,
                        Value::List(vec![Value::Float(lo), Value::Float(hi)]),
                    );
                }
                Value::Map(m)
            })
            .collect(),
    )
}

impl StructShape {
    /// Written into `defs` (once: a recursive type finds itself there).
    fn define(&self, defs: &mut Map) {
        if defs.get(self.name).is_some() {
            return;
        }
        // Placeholder first: a recursive type finds itself defined.
        defs.insert(self.name, Value::Null);
        let mut d = of_kind(kind::STRUCT);
        put_doc(&mut d, self.doc);
        d.insert(key::FIELDS, describe_fields(&self.fields, defs));
        defs.insert(self.name, Value::Map(d));
    }
}

impl EnumShape {
    /// Written into `defs` (once).
    fn define(&self, defs: &mut Map) {
        if defs.get(self.name).is_some() {
            return;
        }
        defs.insert(self.name, Value::Null);
        let mut d = of_kind(kind::ENUM);
        put_doc(&mut d, self.doc);
        if let Some(tag) = self.tag {
            d.insert(key::TAG, Value::from(tag));
        }
        let variants = self
            .variants
            .iter()
            .map(|v| {
                let mut m = Map::new();
                m.insert(key::NAME, Value::from(v.name));
                put_doc(&mut m, v.doc);
                if self.tag.is_some() {
                    m.insert(key::FIELDS, describe_fields(&v.fields, defs));
                }
                Value::Map(m)
            })
            .collect();
        d.insert(key::VARIANTS, Value::List(variants));
        defs.insert(self.name, Value::Map(d));
    }
}

impl Shape {
    /// The shape as Yog Data, for tools in other languages (Yog-IDLE).
    /// Structs and enums are written once in `defs` and referred to by name.
    pub fn describe(&self) -> Value {
        let mut defs = Map::new();
        let root = self.describe_into(&mut defs);
        let mut m = Map::new();
        m.insert(key::ROOT, root);
        m.insert(key::DEFS, Value::Map(defs));
        Value::Map(m)
    }

    /// The word a plain shape (one with nothing more to say) is described by.
    fn plain_kind(&self) -> Option<&'static str> {
        Some(match self {
            Shape::Any => kind::ANY,
            Shape::Bool => kind::BOOL,
            Shape::Str => kind::STRING,
            Shape::Atom => kind::ATOM,
            Shape::Text => kind::TEXT,
            Shape::Image => kind::IMAGE,
            Shape::Color => kind::COLOR,
            Shape::Bytes => kind::BYTES,
            Shape::Res => kind::RES,
            _ => return None,
        })
    }

    fn describe_into(&self, defs: &mut Map) -> Value {
        if let Some(k) = self.plain_kind() {
            return Value::Map(of_kind(k));
        }
        let m = match self {
            Shape::Int { min, max } => {
                let mut m = of_kind(kind::INT);
                m.insert(key::MIN, Value::Int(*min));
                m.insert(key::MAX, Value::Int(*max));
                m
            }
            Shape::Float { bits } => {
                let mut m = of_kind(kind::FLOAT);
                m.insert(key::BITS, Value::Int(i64::from(*bits)));
                m
            }
            Shape::Ref(k) | Shape::Tag(k) | Shape::RefOrTag(k) | Shape::RefOrImage(k) => {
                let mut m = of_kind(match self {
                    Shape::Ref(_) => kind::REF,
                    Shape::Tag(_) => kind::TAG,
                    Shape::RefOrTag(_) => kind::REF_OR_TAG,
                    _ => kind::REF_OR_IMAGE,
                });
                m.insert(
                    key::OF,
                    Value::List(k.iter().map(|k| Value::from(*k)).collect()),
                );
                m
            }
            Shape::List(s) | Shape::Map(s) | Shape::Optional(s) => {
                let mut m = of_kind(match self {
                    Shape::List(_) => kind::LIST,
                    Shape::Map(_) => kind::MAP,
                    _ => kind::OPTIONAL,
                });
                m.insert(key::OF, s().describe_into(defs));
                m
            }
            Shape::Struct(s) => {
                s.define(defs);
                def_ref(s.name)
            }
            Shape::Enum(e) => {
                e.define(defs);
                def_ref(e.name)
            }
            _ => Map::new(),
        };
        Value::Map(m)
    }
}
