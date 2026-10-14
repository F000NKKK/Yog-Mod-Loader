//! The model: a tree of typed values, like NBT.

use std::fmt;

/// A Yog Data value.
///
/// The text form has no syntax of its own for the typed values (`Ref`,
/// `Tag`, `Res`, `Text`, `Image`, `Color`): they're written as strings
/// (`"@item:minecraft:stick"`), so every file stays JSON, and the schema of
/// a field says which type its string is. The binary form keeps the type.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Map(Map),
    /// `@item:minecraft:stick` — content of some kind, by id.
    Ref(Ref),
    /// `@tag:item:minecraft:planks` — a tag of content of some kind.
    Tag(Ref),
    /// `@Strings.title` — a mod resource (`.yres` entry).
    Res(Res),
    /// `@lang:gui.done` — a translation key of the game's or a mod's lang files.
    Lang(String),
    /// Yog text: `{b|bold} and {#8d6acc|color}`.
    Text(String),
    /// An image source: `sprite:button`, `mymod:gui/logo`, `item:minecraft:stick`.
    Image(String),
    /// `0xAARRGGBB`.
    Color(u32),
}

/// A reference to content: its kind (`item`, `block`, `entity`, `recipe`,
/// `entry`, `category`, or a mod's own) and id (`minecraft:stick`,
/// `minecraft:furnace[facing=north]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ref {
    pub kind: String,
    pub id: String,
}

impl Ref {
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }

    /// The namespace of the id (`minecraft` for an id without one).
    pub fn namespace(&self) -> &str {
        self.id.split_once(':').map_or("minecraft", |(ns, _)| ns)
    }
}

/// A mod resource: `@<set>.<key>` — `@Strings.screen.title` is set
/// `Strings`, key `screen.title`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Res {
    pub set: String,
    pub key: String,
}

/// A map: entries in file order, keys unique.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map(Vec<(String, Value)>);

impl Map {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn with_capacity(n: usize) -> Self {
        Self(Vec::with_capacity(n))
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value of the `i`-th entry.
    pub fn value_at(&self, i: usize) -> Option<&Value> {
        self.0.get(i).map(|(_, v)| v)
    }

    /// The position of `key` among the entries.
    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.0.iter().position(|(k, _)| k == key)
    }

    /// Sets `key`, in place if it's there, else at the end; returns the old value.
    pub fn insert(&mut self, key: impl Into<String>, value: Value) -> Option<Value> {
        let key = key.into();
        match self.get_mut(&key) {
            Some(slot) => Some(std::mem::replace(slot, value)),
            None => {
                self.0.push((key, value));
                None
            }
        }
    }

    /// Appends without checking for the key — for decoders that already did.
    pub(crate) fn push_unchecked(&mut self, key: String, value: Value) {
        self.0.push((key, value));
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self.index_of(key)?;
        Some(self.0.remove(i).1)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &Value)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn keys(&self) -> impl ExactSizeIterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&str, &Value) -> bool) {
        self.0.retain(|(k, v)| keep(k, v));
    }
}

impl IntoIterator for Map {
    type Item = (String, Value);
    type IntoIter = std::vec::IntoIter<(String, Value)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Into<String>> FromIterator<(K, Value)> for Map {
    fn from_iter<I: IntoIterator<Item = (K, Value)>>(iter: I) -> Self {
        let mut map = Map::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl Value {
    /// The type's name, for messages: `"a string"`, `"a list"`, ...
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "true/false",
            Value::Int(_) => "an integer",
            Value::Float(_) => "a number",
            Value::Str(_) => "a string",
            Value::List(_) => "a list",
            Value::Map(_) => "an object",
            Value::Ref(_) => "a reference",
            Value::Tag(_) => "a tag",
            Value::Res(_) => "a resource",
            Value::Lang(_) => "a translation key",
            Value::Text(_) => "a text",
            Value::Image(_) => "an image",
            Value::Color(_) => "a color",
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::Float(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(n) => Some(*n as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// The string of a `Str` (and of the string-like typed values: `Text`, `Image`).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Text(s) | Value::Image(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&Map> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    /// `self[key]` of a map, `None` for anything else.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_map()?.get(key)
    }

    /// The value at `path`.
    pub fn at(&self, path: &Path) -> Option<&Value> {
        let mut v = self;
        for seg in path.segments() {
            v = match (seg, v) {
                (Seg::Key(k), Value::Map(m)) => m.get(k)?,
                (Seg::Index(i), Value::List(l)) => l.get(*i)?,
                _ => return None,
            };
        }
        Some(v)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Int(n)
    }
}
impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}
impl From<Vec<Value>> for Value {
    fn from(l: Vec<Value>) -> Self {
        Value::List(l)
    }
}
impl From<Map> for Value {
    fn from(m: Map) -> Self {
        Value::Map(m)
    }
}

/// One step of a [`Path`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Seg {
    Key(String),
    Index(usize),
}

/// Where a value is in a document: `entries[2].pages[0].item`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Path(Vec<Seg>);

impl Path {
    pub fn root() -> Self {
        Self(Vec::new())
    }

    pub fn segments(&self) -> &[Seg] {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn push(&mut self, seg: Seg) {
        self.0.push(seg);
    }

    pub fn pop(&mut self) {
        self.0.pop();
    }

    pub fn key(&self, key: &str) -> Path {
        let mut p = self.clone();
        p.0.push(Seg::Key(key.to_string()));
        p
    }

    pub fn index(&self, i: usize) -> Path {
        let mut p = self.clone();
        p.0.push(Seg::Index(i));
        p
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("(root)");
        }
        for (i, seg) in self.0.iter().enumerate() {
            match seg {
                Seg::Key(k) if is_plain_key(k) => {
                    if i > 0 {
                        f.write_str(".")?;
                    }
                    f.write_str(k)?;
                }
                Seg::Key(k) => write!(f, "[{k:?}]")?,
                Seg::Index(n) => write!(f, "[{n}]")?,
            }
        }
        Ok(())
    }
}

fn is_plain_key(k: &str) -> bool {
    let mut chars = k.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}
