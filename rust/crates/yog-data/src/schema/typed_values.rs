//! The schemas of typed values: references, tags, resources, texts, images, colors.
use super::{PhantomData, Reader, Ref, Res, Schema, Shape, Value, ValueRef, fmt, typed};

// ── Typed values ────────────────────────────────────────────────────────────

/// A kind of content: `item`, `block`, ... — a mod declares its own with
/// [`kind!`](crate::kind).
pub trait Kind {
    const NAME: &'static str;
}

/// One kind or several (`(Item, Block)`): what a reference field takes.
pub trait Kinds {
    const NAMES: &'static [&'static str];
}

impl<K: Kind> Kinds for K {
    const NAMES: &'static [&'static str] = &[K::NAME];
}

impl<A: Kind, B: Kind> Kinds for (A, B) {
    const NAMES: &'static [&'static str] = &[A::NAME, B::NAME];
}

impl<A: Kind, B: Kind, C: Kind> Kinds for (A, B, C) {
    const NAMES: &'static [&'static str] = &[A::NAME, B::NAME, C::NAME];
}

/// Declares kinds of content: `yog_data::kind!(Pattern = "pattern");`.
#[macro_export]
macro_rules! kind {
    ($($name:ident = $kind:expr),* $(,)?) => {$(
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
        Item = yog_ids::kind::ITEM,
        Block = yog_ids::kind::BLOCK,
        Entity = yog_ids::kind::ENTITY,
        Recipe = yog_ids::kind::RECIPE,
        Entry = yog_ids::kind::ENTRY,
        Category = yog_ids::kind::CATEGORY,
        Fluid = yog_ids::kind::FLUID,
        Biome = yog_ids::kind::BIOME,
        Dimension = yog_ids::kind::DIMENSION,
        Texture = yog_ids::kind::TEXTURE,
        Material = yog_ids::kind::MATERIAL,
        Advancement = yog_ids::kind::ADVANCEMENT,
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

/// `v` as a reference to content of one of `kinds` (any kind if empty),
/// written `@kind:id` — a bare id is an error that says how to write it.
pub fn read_ref(v: &ValueRef, cx: &mut Reader, kinds: &[&str]) -> Option<Ref> {
    let wanted = || {
        kinds
            .iter()
            .map(|k| format!("`@{k}:`"))
            .collect::<Vec<_>>()
            .join(" or ")
    };
    let r = match v {
        ValueRef::Ref { kind, id } => Ref::new(*kind, *id),
        ValueRef::Str(s) if s.starts_with('@') => match typed::classify(s) {
            Ok(Value::Ref(r)) => r,
            Ok(other) => {
                cx.problem(format!(
                    "expected a reference ({}), found {}",
                    if kinds.is_empty() {
                        "`@kind:id`".into()
                    } else {
                        wanted()
                    },
                    other.kind_name()
                ));
                return None;
            }
            Err(e) => {
                cx.problem(e);
                return None;
            }
        },
        ValueRef::Str(s) => {
            let kind = kinds.first().copied().unwrap_or(yog_ids::kind::ITEM);
            cx.problem(format!("`{s}`: name it by a reference — `@{kind}:{s}`"));
            return None;
        }
        other => {
            cx.expected("a reference `@kind:id`", other);
            return None;
        }
    };
    if !kinds.is_empty() && !kinds.contains(&r.kind.as_str()) {
        cx.problem(format!(
            "`@{}:{}` is a reference to {} — this takes {}",
            r.kind,
            r.id,
            r.kind,
            wanted()
        ));
        return None;
    }
    cx.note_ref(r.clone());
    Some(r)
}

fn read_tag(v: &ValueRef, cx: &mut Reader, kind: &str) -> Option<Ref> {
    let r = match v {
        ValueRef::Tag { kind, id } => Ref::new(*kind, *id),
        ValueRef::Str(s) => match typed::classify(s) {
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
        Shape::Ref(<K as Kinds>::NAMES)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        read_ref(v, cx, <K as Kinds>::NAMES).map(|r| Id::new(r.id))
    }
    fn write(&self) -> Value {
        Value::Ref(Ref::new(K::NAME, self.id.clone()))
    }
}

impl Schema for Ref {
    fn shape() -> Shape {
        Shape::Ref(&[])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        if let ValueRef::Str(s) = v
            && !s.starts_with('@')
        {
            cx.problem(format!("expected a reference `@kind:id`, found {s:?}"));
            return None;
        }
        read_ref(v, cx, &[])
    }
    fn write(&self) -> Value {
        Value::Ref(self.clone())
    }
}

/// Content of any kind or a tag of it: `@entity:minecraft:pig`,
/// `@tag:entity:c:bosses` — a tag's entries, whose kind the tag says.
#[derive(Debug, Clone, PartialEq)]
pub enum AnyRefOrTag {
    One(Ref),
    Tag(Ref),
}

impl AnyRefOrTag {
    /// The kind it names (`entity`).
    pub fn kind(&self) -> &str {
        match self {
            AnyRefOrTag::One(r) | AnyRefOrTag::Tag(r) => &r.kind,
        }
    }
}

impl fmt::Display for AnyRefOrTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyRefOrTag::One(r) => write!(f, "@{}:{}", r.kind, r.id),
            AnyRefOrTag::Tag(r) => write!(f, "@tag:{}:{}", r.kind, r.id),
        }
    }
}

impl Schema for AnyRefOrTag {
    fn shape() -> Shape {
        Shape::RefOrTag(&[])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let is_tag = match v {
            ValueRef::Tag { .. } => true,
            ValueRef::Str(s) => s.starts_with("@tag:"),
            _ => false,
        };
        if is_tag {
            let r = match v {
                ValueRef::Tag { kind, id } => Ref::new(*kind, *id),
                ValueRef::Str(s) => match typed::classify(s) {
                    Ok(Value::Tag(r)) => r,
                    Ok(_) => {
                        cx.problem(format!("expected a tag `@tag:kind:id`, found {s:?}"));
                        return None;
                    }
                    Err(e) => {
                        cx.problem(e);
                        return None;
                    }
                },
                _ => return None,
            };
            return Some(AnyRefOrTag::Tag(r));
        }
        if let ValueRef::Str(s) = v
            && !s.starts_with('@')
        {
            cx.problem(format!(
                "`{s}`: write `@<kind>:{s}` or a tag `@tag:<kind>:{s}`"
            ));
            return None;
        }
        read_ref(v, cx, &[]).map(AnyRefOrTag::One)
    }
    fn write(&self) -> Value {
        match self {
            AnyRefOrTag::One(r) => Value::Ref(r.clone()),
            AnyRefOrTag::Tag(r) => Value::Tag(r.clone()),
        }
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
        Shape::Tag(<K as Kinds>::NAMES)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
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
        Shape::RefOrTag(<K as Kinds>::NAMES)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        if let ValueRef::Str(s) = v
            && let Some(tag) = s.strip_prefix('#')
        {
            cx.problem(format!("`{s}`: write a tag as `@tag:{}:{tag}`", K::NAME));
            return None;
        }
        let is_tag = match v {
            ValueRef::Tag { .. } => true,
            ValueRef::Str(s) => s.starts_with("@tag:"),
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
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let r = match v {
            ValueRef::Res { set, key } => Res {
                set: set.to_string(),
                key: key.to_string(),
            },
            ValueRef::Str(s) => match typed::classify(s) {
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

/// A text a player reads: Yog text as written, a `.yres` text, the name
/// of some content (`@item:minecraft:stick` shows "Stick"), or any
/// translation key (`@lang:gui.done`).
#[derive(Debug, Clone, PartialEq)]
pub enum Text {
    Literal(String),
    Res(Res),
    NameOf(Ref),
    Lang(String),
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
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let typed = match v {
            // Words as they are, the usual case: copied once.
            ValueRef::Str(s) if !s.starts_with('@') => return Some(Text::Literal(s.to_string())),
            ValueRef::Text(t) => return Some(Text::Literal(t.to_string())),
            ValueRef::Lang(k) => return Some(Text::Lang(k.to_string())),
            ValueRef::Str(s) => typed::classify(s).map_err(|e| cx.problem(e)).ok()?,
            ValueRef::Res { .. } | ValueRef::Ref { .. } => v.to_value(),
            other => {
                cx.expected("a text", other);
                return None;
            }
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
            Value::Lang(k) => Some(Text::Lang(k)),
            other => {
                cx.problem(format!("expected a text, found {}", other.kind_name()));
                None
            }
        }
    }
    fn write(&self) -> Value {
        match self {
            Text::Literal(s) => Value::Text(s.clone()),
            Text::Res(r) => Value::Res(r.clone()),
            Text::NameOf(r) => Value::Ref(r.clone()),
            Text::Lang(k) => Value::Lang(k.clone()),
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
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        match v {
            ValueRef::Image(s) => Some(Image::Source(s.to_string())),
            ValueRef::Res { .. } => Res::read(v, cx).map(Image::Res),
            ValueRef::Str(s) if !s.starts_with('@') => Some(Image::Source(s.to_string())),
            ValueRef::Str(s) => match typed::classify(s) {
                Ok(Value::Image(src)) => Some(Image::Source(src)),
                Ok(Value::Res(r)) => {
                    cx.note_res(r.clone());
                    Some(Image::Res(r))
                }
                Ok(Value::Str(s)) => Some(Image::Source(s)),
                Ok(other) => {
                    cx.problem(format!("expected an image, found {}", other.kind_name()));
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
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        match v {
            ValueRef::Color(c) => Some(Color::Argb(*c)),
            ValueRef::Res { .. } => Res::read(v, cx).map(Color::Res),
            ValueRef::Str(s) if s.starts_with('#') => {
                typed::parse_color(s).map(Color::Argb).or_else(|| {
                    cx.problem(format!("`{s}` isn't a color (#RRGGBB or #AARRGGBB)"));
                    None
                })
            }
            ValueRef::Str(s) if s.starts_with('@') => Res::read(v, cx).map(Color::Res),
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
