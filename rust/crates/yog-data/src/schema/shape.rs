//! Shapes: what a format looks like.

#[allow(unused_imports)] // each part uses some of the module's names
use super::*;

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

impl Shape {
    /// The shape as Yog Data, for tools in other languages (Yog-IDLE).
    /// Structs and enums are written once in `defs` and referred to by name.
    pub fn describe(&self) -> Value {
        let mut defs = Map::new();
        let root = self.describe_into(&mut defs);
        let mut m = Map::new();
        m.insert("root", root);
        m.insert("defs", Value::Map(defs));
        Value::Map(m)
    }

    fn describe_into(&self, defs: &mut Map) -> Value {
        let kind = |k: &str| {
            let mut m = Map::new();
            m.insert("kind", Value::from(k));
            m
        };
        let fields = |fs: &[Field], defs: &mut Map| -> Value {
            Value::List(
                fs.iter()
                    .map(|f| {
                        let mut m = Map::new();
                        m.insert("name", Value::from(f.name));
                        if !f.doc.is_empty() {
                            m.insert("doc", Value::from(f.doc));
                        }
                        m.insert("shape", (f.shape)().describe_into(defs));
                        m.insert("required", Value::Bool(f.required));
                        if let Some((lo, hi)) = f.range {
                            m.insert(
                                "range",
                                Value::List(vec![Value::Float(lo), Value::Float(hi)]),
                            );
                        }
                        Value::Map(m)
                    })
                    .collect(),
            )
        };
        let m = match self {
            Shape::Any => kind("any"),
            Shape::Bool => kind("bool"),
            Shape::Int { min, max } => {
                let mut m = kind("int");
                m.insert("min", Value::Int(*min));
                m.insert("max", Value::Int(*max));
                m
            }
            Shape::Float { bits } => {
                let mut m = kind("float");
                m.insert("bits", Value::Int(i64::from(*bits)));
                m
            }
            Shape::Str => kind("string"),
            Shape::Atom => kind("atom"),
            Shape::Text => kind("text"),
            Shape::Image => kind("image"),
            Shape::Color => kind("color"),
            Shape::Bytes => kind("bytes"),
            Shape::Res => kind("res"),
            Shape::Ref(k) | Shape::Tag(k) | Shape::RefOrTag(k) | Shape::RefOrImage(k) => {
                let mut m = kind(match self {
                    Shape::Ref(_) => "ref",
                    Shape::Tag(_) => "tag",
                    Shape::RefOrTag(_) => "ref_or_tag",
                    _ => "ref_or_image",
                });
                m.insert(
                    "of",
                    Value::List(k.iter().map(|k| Value::from(*k)).collect()),
                );
                m
            }
            Shape::List(s) | Shape::Map(s) | Shape::Optional(s) => {
                let mut m = kind(match self {
                    Shape::List(_) => "list",
                    Shape::Map(_) => "map",
                    _ => "optional",
                });
                m.insert("of", s().describe_into(defs));
                m
            }
            Shape::Struct(s) => {
                if defs.get(s.name).is_none() {
                    // Placeholder first: a recursive type finds itself defined.
                    defs.insert(s.name, Value::Null);
                    let mut d = kind("struct");
                    if !s.doc.is_empty() {
                        d.insert("doc", Value::from(s.doc));
                    }
                    d.insert("fields", fields(&s.fields, defs));
                    defs.insert(s.name, Value::Map(d));
                }
                let mut m = kind("def");
                m.insert("name", Value::from(s.name));
                m
            }
            Shape::Enum(e) => {
                if defs.get(e.name).is_none() {
                    defs.insert(e.name, Value::Null);
                    let mut d = kind("enum");
                    if !e.doc.is_empty() {
                        d.insert("doc", Value::from(e.doc));
                    }
                    if let Some(tag) = e.tag {
                        d.insert("tag", Value::from(tag));
                    }
                    let variants = e
                        .variants
                        .iter()
                        .map(|v| {
                            let mut m = Map::new();
                            m.insert("name", Value::from(v.name));
                            if !v.doc.is_empty() {
                                m.insert("doc", Value::from(v.doc));
                            }
                            if e.tag.is_some() {
                                m.insert("fields", fields(&v.fields, defs));
                            }
                            Value::Map(m)
                        })
                        .collect();
                    d.insert("variants", Value::List(variants));
                    defs.insert(e.name, Value::Map(d));
                }
                let mut m = kind("def");
                m.insert("name", Value::from(e.name));
                m
            }
        };
        Value::Map(m)
    }
}
