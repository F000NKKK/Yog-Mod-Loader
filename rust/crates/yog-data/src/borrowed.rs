//! [`ValueRef`]: a value borrowed from where it was read — what schemas
//! read. Decoded from the binary form ([`crate::binary::decode_ref`]) no
//! string is copied: every one points into the bytes, and lists and maps
//! are slices in an arena ([`Bump`]). An owned [`Value`] lends itself the
//! same way ([`Value::borrow`]).

use bumpalo::Bump;

use crate::value::{Map, Ref, Res, Value};

/// A borrowed [`Value`]: the same cases, strings `&'a str`, lists and maps slices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueRef<'a> {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'a str),
    List(&'a [ValueRef<'a>]),
    Map(MapRef<'a>),
    /// `@item:minecraft:stick`.
    Ref { kind: &'a str, id: &'a str },
    /// `@tag:item:minecraft:planks`.
    Tag { kind: &'a str, id: &'a str },
    /// `@Strings.title`.
    Res { set: &'a str, key: &'a str },
    /// `@lang:gui.done`.
    Lang(&'a str),
    /// Yog text.
    Text(&'a str),
    /// An image source.
    Image(&'a str),
    /// `0xAARRGGBB`.
    Color(u32),
}

/// A borrowed map: entries in file order.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MapRef<'a>(pub &'a [(&'a str, ValueRef<'a>)]);

impl<'a> MapRef<'a> {
    pub fn get(&self, key: &str) -> Option<&'a ValueRef<'a>> {
        self.0.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&'a str, &'a ValueRef<'a>)> + 'a {
        self.0.iter().map(|(k, v)| (*k, v))
    }

    pub fn keys(&self) -> impl ExactSizeIterator<Item = &'a str> + 'a {
        self.0.iter().map(|(k, _)| *k)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'a> ValueRef<'a> {
    /// The type's name, for messages: `"a string"`, `"a list"`, ...
    pub fn kind_name(&self) -> &'static str {
        match self {
            ValueRef::Null => "null",
            ValueRef::Bool(_) => "true/false",
            ValueRef::Int(_) => "an integer",
            ValueRef::Float(_) => "a number",
            ValueRef::Str(_) => "a string",
            ValueRef::List(_) => "a list",
            ValueRef::Map(_) => "an object",
            ValueRef::Ref { .. } => "a reference",
            ValueRef::Tag { .. } => "a tag",
            ValueRef::Res { .. } => "a resource",
            ValueRef::Lang(_) => "a translation key",
            ValueRef::Text(_) => "a text",
            ValueRef::Image(_) => "an image",
            ValueRef::Color(_) => "a color",
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, ValueRef::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ValueRef::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ValueRef::Int(n) => Some(*n),
            ValueRef::Float(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ValueRef::Int(n) => Some(*n as f64),
            ValueRef::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// The string of a `Str` (and of the string-like typed values: `Text`, `Image`).
    pub fn as_str(&self) -> Option<&'a str> {
        match self {
            ValueRef::Str(s) | ValueRef::Text(s) | ValueRef::Image(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&'a [ValueRef<'a>]> {
        match self {
            ValueRef::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<MapRef<'a>> {
        match self {
            ValueRef::Map(m) => Some(*m),
            _ => None,
        }
    }

    /// `self[key]` of a map, `None` for anything else.
    pub fn get(&self, key: &str) -> Option<&'a ValueRef<'a>> {
        self.as_map()?.get(key)
    }

    /// An owned copy.
    pub fn to_value(&self) -> Value {
        match *self {
            ValueRef::Null => Value::Null,
            ValueRef::Bool(b) => Value::Bool(b),
            ValueRef::Int(n) => Value::Int(n),
            ValueRef::Float(f) => Value::Float(f),
            ValueRef::Str(s) => Value::Str(s.to_string()),
            ValueRef::List(l) => Value::List(l.iter().map(ValueRef::to_value).collect()),
            ValueRef::Map(m) => {
                let mut out = Map::with_capacity(m.len());
                for (k, v) in m.iter() {
                    out.insert(k, v.to_value());
                }
                Value::Map(out)
            }
            ValueRef::Ref { kind, id } => Value::Ref(Ref::new(kind, id)),
            ValueRef::Tag { kind, id } => Value::Tag(Ref::new(kind, id)),
            ValueRef::Res { set, key } => Value::Res(Res { set: set.to_string(), key: key.to_string() }),
            ValueRef::Lang(k) => Value::Lang(k.to_string()),
            ValueRef::Text(s) => Value::Text(s.to_string()),
            ValueRef::Image(s) => Value::Image(s.to_string()),
            ValueRef::Color(c) => Value::Color(c),
        }
    }

    /// The reference of a `Ref`, owned.
    pub fn to_ref(&self) -> Option<Ref> {
        match *self {
            ValueRef::Ref { kind, id } => Some(Ref::new(kind, id)),
            _ => None,
        }
    }
}

impl Value {
    /// This value, borrowed (lists and maps in `arena`) — what schemas read.
    pub fn borrow<'a>(&'a self, arena: &'a Bump) -> ValueRef<'a> {
        match self {
            Value::Null => ValueRef::Null,
            Value::Bool(b) => ValueRef::Bool(*b),
            Value::Int(n) => ValueRef::Int(*n),
            Value::Float(f) => ValueRef::Float(*f),
            Value::Str(s) => ValueRef::Str(s),
            Value::List(l) => ValueRef::List(arena.alloc_slice_fill_iter(l.iter().map(|v| v.borrow(arena)))),
            Value::Map(m) => ValueRef::Map(MapRef(arena.alloc_slice_fill_iter(m.iter().map(|(k, v)| (k, v.borrow(arena)))))),
            Value::Ref(r) => ValueRef::Ref { kind: &r.kind, id: &r.id },
            Value::Tag(r) => ValueRef::Tag { kind: &r.kind, id: &r.id },
            Value::Res(r) => ValueRef::Res { set: &r.set, key: &r.key },
            Value::Lang(k) => ValueRef::Lang(k),
            Value::Text(s) => ValueRef::Text(s),
            Value::Image(s) => ValueRef::Image(s),
            Value::Color(c) => ValueRef::Color(*c),
        }
    }
}
