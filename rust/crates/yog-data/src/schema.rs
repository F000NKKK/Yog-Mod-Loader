//! Schemas: a format is a Rust type implementing [`Schema`] (usually
//! `#[derive(Schema)]`). One schema reads and writes the format, checks it
//! with the path of every problem, collects its references for the build to
//! check, and describes itself ([`Shape`]) for Yog-IDLE's editors and the
//! docs.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::marker::PhantomData;

use crate::text::{self, Doc, LineCol};
use crate::typed;
use crate::value::{Map, Path, Ref, Res, Seg, Value};

/// A Yog Data format.
pub trait Schema: Sized {
    /// What the format looks like.
    fn shape() -> Shape;

    /// Reads `v`. Problems go to `cx` at its current path; `None` when the
    /// value couldn't be read (the problem is in `cx`).
    fn read(v: &Value, cx: &mut Reader) -> Option<Self>;

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
    Float,
    /// Plain text.
    Str,
    /// Yog text, a `@Set.key` resource, or `@kind:id` (that content's name).
    Text,
    /// An image source or a `@Set.key` image resource.
    Image,
    /// `#RRGGBB`/`#AARRGGBB` or a `@Set.key` color resource.
    Color,
    /// A mod resource, `@Set.key`.
    Res,
    /// Content of a kind (`""`: any kind, written `@kind:id`).
    Ref(&'static str),
    /// A tag of content of a kind.
    Tag(&'static str),
    /// Content of a kind or a tag of it.
    RefOrTag(&'static str),
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
            Shape::Float => kind("float"),
            Shape::Str => kind("string"),
            Shape::Text => kind("text"),
            Shape::Image => kind("image"),
            Shape::Color => kind("color"),
            Shape::Res => kind("res"),
            Shape::Ref(k) | Shape::Tag(k) | Shape::RefOrTag(k) => {
                let mut m = kind(match self {
                    Shape::Ref(_) => "ref",
                    Shape::Tag(_) => "tag",
                    _ => "ref_or_tag",
                });
                m.insert("of", Value::from(*k));
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

// ── Reading ─────────────────────────────────────────────────────────────────

/// A problem found reading a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: Path,
    pub message: String,
    /// About the key at `path` rather than its value (an unknown field).
    pub at_key: bool,
}

/// Where a reference was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found<T> {
    pub value: T,
    pub path: Path,
}

/// The state of reading a value: where it is, the problems so far, and the
/// references met (for the build to check against the content there is).
#[derive(Debug, Default)]
pub struct Reader {
    path: Path,
    pub problems: Vec<Problem>,
    pub refs: Vec<Found<Ref>>,
    pub tags: Vec<Found<Ref>>,
    pub resources: Vec<Found<Res>>,
    /// Keys a struct being read doesn't report as unknown (an enum's tag).
    allowed: Vec<&'static str>,
}

/// Source-only keys every object may have: `$…`, `_…`, `comment`.
pub fn is_metadata_key(key: &str) -> bool {
    key.starts_with('$') || key.starts_with('_') || key == "comment"
}

impl Reader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reports a problem with the value being read.
    pub fn problem(&mut self, message: impl Into<String>) {
        self.problems.push(Problem {
            path: self.path.clone(),
            message: message.into(),
            at_key: false,
        });
    }

    /// "expected X, found Y".
    pub fn expected(&mut self, what: &str, found: &Value) {
        self.problem(format!("expected {what}, found {}", found_text(found)));
    }

    /// Runs `f` one step deeper.
    pub fn at<T>(&mut self, seg: Seg, f: impl FnOnce(&mut Self) -> T) -> T {
        self.path.push(seg);
        let r = f(self);
        self.path.pop();
        r
    }

    pub fn note_ref(&mut self, r: Ref) {
        self.refs.push(Found {
            value: r,
            path: self.path.clone(),
        });
    }

    pub fn note_tag(&mut self, r: Ref) {
        self.tags.push(Found {
            value: r,
            path: self.path.clone(),
        });
    }

    pub fn note_res(&mut self, r: Res) {
        self.resources.push(Found {
            value: r,
            path: self.path.clone(),
        });
    }

    /// `v` as an object, or a problem.
    pub fn map<'v>(&mut self, v: &'v Value) -> Option<&'v Map> {
        match v {
            Value::Map(m) => Some(m),
            other => {
                self.expected("an object", other);
                None
            }
        }
    }

    /// Reports keys of `map` that aren't `known` (nor metadata, nor allowed by an enclosing enum).
    pub fn unknown_keys(&mut self, map: &Map, known: &[&str]) {
        for key in map.keys() {
            if known.contains(&key) || is_metadata_key(key) || self.allowed.contains(&key) {
                continue;
            }
            let hint = closest(key, known)
                .map(|k| format!(" — did you mean `{k}`?"))
                .unwrap_or_else(|| {
                    if known.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " (expected {})",
                            known
                                .iter()
                                .map(|k| format!("`{k}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                });
            self.problems.push(Problem {
                path: self.path.key(key),
                message: format!("unknown field `{key}`{hint}"),
                at_key: true,
            });
        }
    }

    /// Reports a name that isn't one of `known` (an enum's variant).
    pub fn unknown_name(&mut self, name: &str, known: &[&str]) {
        let hint = match closest(name, known) {
            Some(k) => format!(" — did you mean `{k}`?"),
            None => format!(
                " (expected {})",
                known
                    .iter()
                    .map(|k| format!("`{k}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        self.problem(format!("unknown `{name}`{hint}"));
    }

    /// Field `key` of `map`: read, or its absent value, or a problem.
    pub fn field<T: Schema>(&mut self, map: &Map, key: &'static str) -> Option<T> {
        match map.get(key) {
            Some(v) => self.at(Seg::Key(key.to_string()), |cx| T::read(v, cx)),
            None => T::absent().or_else(|| {
                self.problem(format!("missing `{key}`"));
                None
            }),
        }
    }

    /// Field `key` of `map`, `default()` when it isn't there.
    pub fn field_or<T: Schema>(
        &mut self,
        map: &Map,
        key: &'static str,
        default: impl FnOnce() -> T,
    ) -> Option<T> {
        match map.get(key) {
            Some(v) => self.at(Seg::Key(key.to_string()), |cx| T::read(v, cx)),
            None => Some(default()),
        }
    }

    /// Reads `v` as `T` with `key` allowed in it — the tag of the enum variant `T` is.
    pub fn with_allowed<T>(&mut self, key: &'static str, f: impl FnOnce(&mut Self) -> T) -> T {
        self.allowed.push(key);
        let r = f(self);
        self.allowed.pop();
        r
    }

    /// Reads a struct's own keys with no allowance inherited from outside.
    pub fn own_keys<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::take(&mut self.allowed);
        let r = f(self);
        self.allowed = saved;
        r
    }
}

fn found_text(v: &Value) -> String {
    match v {
        Value::Str(s) if s.chars().count() <= 40 => format!("{s:?}"),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.kind_name().to_string(),
    }
}

/// The `known` name closest to `key`, if it's close enough to be a typo.
fn closest<'k>(key: &str, known: &[&'k str]) -> Option<&'k str> {
    let lower = key.to_ascii_lowercase();
    known
        .iter()
        .map(|k| (edit_distance(&lower, k), *k))
        .filter(|(d, k)| *d <= (k.len() / 3).max(1))
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != *cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Problems found reading a document, with where each is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Errors(pub Vec<Located>);

/// A problem and its place in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub at: Option<LineCol>,
    pub path: Path,
    pub message: String,
}

impl fmt::Display for Located {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(at) = self.at {
            write!(f, "{at}: ")?;
        }
        if !self.path.is_root() {
            write!(f, "{}: ", self.path)?;
        }
        f.write_str(&self.message)
    }
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Errors {}

/// What reading a document gives: the value and the references in it.
#[derive(Debug)]
pub struct Read<T> {
    pub value: T,
    pub refs: Vec<Found<Ref>>,
    pub tags: Vec<Found<Ref>>,
    pub resources: Vec<Found<Res>>,
}

/// Reads `v` as `T`: the value and its references, or every problem.
pub fn from_value<T: Schema>(v: &Value) -> Result<Read<T>, Vec<Problem>> {
    let mut cx = Reader::new();
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

/// Reads a parsed document as `T`; `text` is its source, for positions.
pub fn from_doc<T: Schema>(text: &str, doc: &Doc) -> Result<Read<T>, Errors> {
    from_value(&doc.value).map_err(|problems| {
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

// ── Plain types ─────────────────────────────────────────────────────────────

impl Schema for Value {
    fn shape() -> Shape {
        Shape::Any
    }
    fn read(v: &Value, _: &mut Reader) -> Option<Self> {
        Some(v.clone())
    }
    fn write(&self) -> Value {
        self.clone()
    }
}

impl Schema for bool {
    fn shape() -> Shape {
        Shape::Bool
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        v.as_bool().or_else(|| {
            cx.expected("true or false", v);
            None
        })
    }
    fn write(&self) -> Value {
        Value::Bool(*self)
    }
}

macro_rules! int_schema {
    ($($t:ty),*) => {$(
        impl Schema for $t {
            fn shape() -> Shape {
                Shape::Int { min: <$t>::MIN as i64, max: (<$t>::MAX as u64).min(i64::MAX as u64) as i64 }
            }
            fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
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
        }
    )*};
}
int_schema!(i8, i16, i32, i64, u8, u16, u32, u64, usize);

macro_rules! float_schema {
    ($($t:ty),*) => {$(
        impl Schema for $t {
            fn shape() -> Shape {
                Shape::Float
            }
            fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
                v.as_f64().map(|f| f as $t).or_else(|| {
                    cx.expected("a number", v);
                    None
                })
            }
            fn write(&self) -> Value {
                Value::Float(*self as f64)
            }
        }
    )*};
}
float_schema!(f32, f64);

impl Schema for String {
    fn shape() -> Shape {
        Shape::Str
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        match v {
            Value::Str(s) => match typed::classify(s) {
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
}

impl<T: Schema> Schema for Option<T> {
    fn shape() -> Shape {
        Shape::Optional(T::shape)
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        if v.is_null() {
            Some(None)
        } else {
            T::read(v, cx).map(Some)
        }
    }
    fn write(&self) -> Value {
        self.as_ref().map_or(Value::Null, T::write)
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
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        T::read(v, cx).map(Box::new)
    }
    fn write(&self) -> Value {
        (**self).write()
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
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        let Value::List(items) = v else {
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
}

impl<T: Schema, const N: usize> Schema for [T; N] {
    fn shape() -> Shape {
        Shape::List(T::shape)
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
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
}

fn read_map<T: Schema>(v: &Value, cx: &mut Reader, mut put: impl FnMut(String, T)) -> Option<()> {
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
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
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
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
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
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        let mut out = Vec::new();
        read_map(v, cx, |k, x| out.push((k, x)))?;
        Some(Ordered(out))
    }
    fn write(&self) -> Value {
        Value::Map(self.0.iter().map(|(k, v)| (k.clone(), v.write())).collect())
    }
}

// ── Typed values ────────────────────────────────────────────────────────────

/// A kind of content: `item`, `block`, ... — a mod declares its own with
/// [`kind!`](crate::kind).
pub trait Kind {
    const NAME: &'static str;
}

/// Declares kinds of content: `yog_data::kind!(Pattern = "pattern");`.
#[macro_export]
macro_rules! kind {
    ($($name:ident = $kind:literal),* $(,)?) => {$(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
        pub struct $name;
        impl $crate::Kind for $name {
            const NAME: &'static str = $kind;
        }
    )*};
}

/// The kinds every Yog format knows.
pub mod kinds {
    crate::kind! {
        Item = "item",
        Block = "block",
        Entity = "entity",
        Recipe = "recipe",
        Entry = "entry",
        Category = "category",
        Fluid = "fluid",
        Biome = "biome",
        Dimension = "dimension",
    }
}

/// The id of content of kind `K`: written `@item:minecraft:stick` or, in a
/// field that can only be an item, just `minecraft:stick` (`stick`).
pub struct Id<K> {
    pub id: String,
    kind: PhantomData<K>,
}

pub type ItemId = Id<kinds::Item>;
pub type BlockId = Id<kinds::Block>;
pub type EntityId = Id<kinds::Entity>;
pub type RecipeId = Id<kinds::Recipe>;

impl<K> Id<K> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: PhantomData,
        }
    }
}

impl<K> Clone for Id<K> {
    fn clone(&self) -> Self {
        Self::new(self.id.clone())
    }
}
impl<K> PartialEq for Id<K> {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}
impl<K> Eq for Id<K> {}
impl<K> std::hash::Hash for Id<K> {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.id.hash(h);
    }
}
impl<K: Kind> fmt::Debug for Id<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}:{}", K::NAME, self.id)
    }
}
impl<K> fmt::Display for Id<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

/// `v` as content of kind `kind`: a reference of that kind, or (`bare`) a bare id.
fn read_ref(v: &Value, cx: &mut Reader, kind: &str) -> Option<Ref> {
    let r = match v {
        Value::Ref(r) => r.clone(),
        Value::Str(s) if s.starts_with('@') => match typed::classify(s) {
            Ok(Value::Ref(r)) => r,
            Ok(other) => {
                cx.problem(format!(
                    "expected {kind} `@{kind}:id`, found {}",
                    other.kind_name()
                ));
                return None;
            }
            Err(e) => {
                cx.problem(e);
                return None;
            }
        },
        Value::Str(s) => match typed::resource_id(s, s) {
            Ok(id) => Ref {
                kind: kind.to_string(),
                id,
            },
            Err(e) => {
                cx.problem(e);
                return None;
            }
        },
        other => {
            cx.expected(&format!("an id of {kind}"), other);
            return None;
        }
    };
    if !kind.is_empty() && r.kind != kind {
        cx.problem(format!("expected {kind}, found {} `{}`", r.kind, r.id));
        return None;
    }
    cx.note_ref(r.clone());
    Some(r)
}

fn read_tag(v: &Value, cx: &mut Reader, kind: &str) -> Option<Ref> {
    let r = match v {
        Value::Tag(r) => r.clone(),
        Value::Str(s) => match typed::classify(s) {
            Ok(Value::Tag(r)) => r,
            Ok(_) => {
                cx.problem(format!("expected a tag `@tag:{kind}:id`, found {s:?}"));
                return None;
            }
            Err(e) => {
                cx.problem(e);
                return None;
            }
        },
        other => {
            cx.expected("a tag", other);
            return None;
        }
    };
    if !kind.is_empty() && r.kind != kind {
        cx.problem(format!(
            "expected a tag of {kind}, found a tag of {}",
            r.kind
        ));
        return None;
    }
    cx.note_tag(r.clone());
    Some(r)
}

impl<K: Kind> Schema for Id<K> {
    fn shape() -> Shape {
        Shape::Ref(K::NAME)
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        read_ref(v, cx, K::NAME).map(|r| Id::new(r.id))
    }
    fn write(&self) -> Value {
        Value::Ref(Ref::new(K::NAME, self.id.clone()))
    }
}

impl Schema for Ref {
    fn shape() -> Shape {
        Shape::Ref("")
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        if let Value::Str(s) = v
            && !s.starts_with('@')
        {
            cx.problem(format!("expected a reference `@kind:id`, found {s:?}"));
            return None;
        }
        read_ref(v, cx, "")
    }
    fn write(&self) -> Value {
        Value::Ref(self.clone())
    }
}

/// A tag of content of kind `K`: `@tag:item:minecraft:planks`.
pub struct TagId<K> {
    pub id: String,
    kind: PhantomData<K>,
}

impl<K> TagId<K> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: PhantomData,
        }
    }
}
impl<K> Clone for TagId<K> {
    fn clone(&self) -> Self {
        Self::new(self.id.clone())
    }
}
impl<K> PartialEq for TagId<K> {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}
impl<K: Kind> fmt::Debug for TagId<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@tag:{}:{}", K::NAME, self.id)
    }
}

impl<K: Kind> Schema for TagId<K> {
    fn shape() -> Shape {
        Shape::Tag(K::NAME)
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        read_tag(v, cx, K::NAME).map(|r| TagId::new(r.id))
    }
    fn write(&self) -> Value {
        Value::Tag(Ref::new(K::NAME, self.id.clone()))
    }
}

/// Content of kind `K` or a tag of it — a recipe ingredient.
#[derive(Debug, Clone, PartialEq)]
pub enum OneOrTag<K: Kind> {
    One(Id<K>),
    Tag(TagId<K>),
}

/// An item or an item tag: `"minecraft:stick"`, `"@tag:item:minecraft:planks"`.
pub type ItemIdOrTag = OneOrTag<kinds::Item>;

impl<K: Kind> Schema for OneOrTag<K> {
    fn shape() -> Shape {
        Shape::RefOrTag(K::NAME)
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        let is_tag = match v {
            Value::Tag(_) => true,
            Value::Str(s) => s.starts_with("@tag:"),
            _ => false,
        };
        if is_tag {
            TagId::read(v, cx).map(OneOrTag::Tag)
        } else {
            Id::read(v, cx).map(OneOrTag::One)
        }
    }
    fn write(&self) -> Value {
        match self {
            OneOrTag::One(i) => i.write(),
            OneOrTag::Tag(t) => t.write(),
        }
    }
}

impl Schema for Res {
    fn shape() -> Shape {
        Shape::Res
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        let r = match v {
            Value::Res(r) => r.clone(),
            Value::Str(s) => match typed::classify(s) {
                Ok(Value::Res(r)) => r,
                Ok(_) => {
                    cx.problem(format!("expected a resource `@Set.key`, found {s:?}"));
                    return None;
                }
                Err(e) => {
                    cx.problem(e);
                    return None;
                }
            },
            other => {
                cx.expected("a resource `@Set.key`", other);
                return None;
            }
        };
        cx.note_res(r.clone());
        Some(r)
    }
    fn write(&self) -> Value {
        Value::Res(self.clone())
    }
}

/// A text a player reads: Yog text as written, a `.yres` text, or the name
/// of some content (`@item:minecraft:stick` shows "Stick").
#[derive(Debug, Clone, PartialEq)]
pub enum Text {
    Literal(String),
    Res(Res),
    NameOf(Ref),
}

impl Default for Text {
    fn default() -> Self {
        Text::Literal(String::new())
    }
}

impl Schema for Text {
    fn shape() -> Shape {
        Shape::Text
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        let typed = match v {
            Value::Str(s) => typed::classify(s).map_err(|e| cx.problem(e)).ok()?,
            Value::Text(t) => return Some(Text::Literal(t.clone())),
            other => other.clone(),
        };
        match typed {
            Value::Str(s) => Some(Text::Literal(s)),
            Value::Res(r) => {
                cx.note_res(r.clone());
                Some(Text::Res(r))
            }
            Value::Ref(r) => {
                cx.note_ref(r.clone());
                Some(Text::NameOf(r))
            }
            other => {
                cx.expected("a text", &other);
                None
            }
        }
    }
    fn write(&self) -> Value {
        match self {
            Text::Literal(s) => Value::Text(s.clone()),
            Text::Res(r) => Value::Res(r.clone()),
            Text::NameOf(r) => Value::Ref(r.clone()),
        }
    }
}

/// An image: a source (`sprite:button`, `mymod:gui/logo`, `item:minecraft:stick`)
/// or a `.yres` image.
#[derive(Debug, Clone, PartialEq)]
pub enum Image {
    Source(String),
    Res(Res),
}

impl Schema for Image {
    fn shape() -> Shape {
        Shape::Image
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        match v {
            Value::Image(s) => Some(Image::Source(s.clone())),
            Value::Res(r) => {
                cx.note_res(r.clone());
                Some(Image::Res(r.clone()))
            }
            Value::Str(s) if !s.starts_with('@') => Some(Image::Source(s.clone())),
            Value::Str(s) => match typed::classify(s) {
                Ok(Value::Image(src)) => Some(Image::Source(src)),
                Ok(Value::Res(r)) => {
                    cx.note_res(r.clone());
                    Some(Image::Res(r))
                }
                Ok(Value::Str(s)) => Some(Image::Source(s)),
                Ok(other) => {
                    cx.expected("an image", &other);
                    None
                }
                Err(e) => {
                    cx.problem(e);
                    None
                }
            },
            other => {
                cx.expected("an image", other);
                None
            }
        }
    }
    fn write(&self) -> Value {
        match self {
            Image::Source(s) => Value::Image(s.clone()),
            Image::Res(r) => Value::Res(r.clone()),
        }
    }
}

/// A color: `#RRGGBB`, `#AARRGGBB`, or a `.yres` color.
#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Argb(u32),
    Res(Res),
}

impl Schema for Color {
    fn shape() -> Shape {
        Shape::Color
    }
    fn read(v: &Value, cx: &mut Reader) -> Option<Self> {
        match v {
            Value::Color(c) => Some(Color::Argb(*c)),
            Value::Res(r) => {
                cx.note_res(r.clone());
                Some(Color::Res(r.clone()))
            }
            Value::Str(s) if s.starts_with('#') => {
                typed::parse_color(s).map(Color::Argb).or_else(|| {
                    cx.problem(format!("`{s}` isn't a color (#RRGGBB or #AARRGGBB)"));
                    None
                })
            }
            Value::Str(s) if s.starts_with('@') => Res::read(v, cx).map(Color::Res),
            other => {
                cx.expected("a color (#RRGGBB or #AARRGGBB)", other);
                None
            }
        }
    }
    fn write(&self) -> Value {
        match self {
            Color::Argb(c) => Value::Color(*c),
            Color::Res(r) => Value::Res(r.clone()),
        }
    }
}
