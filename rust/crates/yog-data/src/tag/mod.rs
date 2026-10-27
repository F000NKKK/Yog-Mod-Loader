//! Tags of content: `@tag:item:minecraft:planks` — a reference of kind
//! `tag` whose id is the content's kind, then the tag's id.

use std::fmt;
use std::marker::PhantomData;

use substrate_data::{Id, Kind, Reader, Ref, Schema, Shape, Value, ValueRef};

use crate::kinds;

/// A tag's id (`item:minecraft:planks`) split: its content's kind and its own id.
fn split(spelled: &str) -> Option<(&str, &str)> {
    spelled.split_once(yog_ids::sep::NAMESPACE)
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
        write!(f, "@{}:{}:{}", kinds::Tag::NAME, K::NAME, self.id)
    }
}

impl<K: Kind> Schema for TagId<K> {
    fn shape() -> Shape {
        substrate_data::schema::ref_shape::<kinds::Tag>()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader<'_>) -> Option<Self> {
        let tag = Id::<kinds::Tag>::read(v, cx)?;
        match split(&tag.id) {
            Some((kind, id)) if kind == K::NAME => Some(TagId::new(id)),
            _ => {
                cx.problem(format!(
                    "`@{}:{}`: this takes a tag of {} — `@{}:{}:<id>`",
                    kinds::Tag::NAME,
                    tag.id,
                    K::NAME,
                    kinds::Tag::NAME,
                    K::NAME
                ));
                None
            }
        }
    }
    fn write(&self) -> Value {
        let spelled = format!("{}{}{}", K::NAME, yog_ids::sep::NAMESPACE, self.id);
        Id::<kinds::Tag>::new(spelled).write()
    }
}

/// Whether `v` is written as a tag.
fn is_tag(v: &ValueRef<'_>) -> bool {
    let tag = kinds::Tag::NAME;
    match v {
        ValueRef::Ref { kind, .. } => kind.as_str() == tag,
        ValueRef::Str(s) => s
            .strip_prefix(yog_ids::sep::TYPED)
            .and_then(|rest| rest.strip_prefix(tag))
            .is_some_and(|rest| rest.starts_with(yog_ids::sep::NAMESPACE)),
        _ => false,
    }
}

/// Content of kind `K` or a tag of it — a recipe ingredient.
#[derive(Debug, Clone, PartialEq)]
pub enum OneOrTag<K: Kind<Id = String>> {
    One(Id<K>),
    Tag(TagId<K>),
}

/// An item or an item tag: `"@item:minecraft:stick"`, `"@tag:item:minecraft:planks"`.
pub type ItemIdOrTag = OneOrTag<kinds::Item>;

impl<K: Kind<Id = String>> Schema for OneOrTag<K> {
    fn shape() -> Shape {
        Shape::Ref {
            kinds: &[K::NAME, kinds::Tag::NAME],
            id: String::shape,
        }
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader<'_>) -> Option<Self> {
        if let ValueRef::Str(s) = v
            && let Some(tag) = s.strip_prefix(yog_ids::sep::TAG)
        {
            cx.problem(format!("`{s}`: write a tag as `@{}:{}:{tag}`", kinds::Tag::NAME, K::NAME));
            return None;
        }
        if is_tag(v) {
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

/// Content of any kind or a tag of it: `@entity:minecraft:pig`,
/// `@tag:entity:c:bosses` — what a tag names is its first part.
#[derive(Debug, Clone, PartialEq)]
pub enum AnyRefOrTag {
    One(Ref),
    /// The tag's content kind and its id.
    Tag(String, String),
}

impl AnyRefOrTag {
    /// The kind of content it names (`entity`).
    pub fn kind(&self) -> &str {
        match self {
            AnyRefOrTag::One(r) => r.kind.as_str(),
            AnyRefOrTag::Tag(kind, _) => kind,
        }
    }
}

impl fmt::Display for AnyRefOrTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyRefOrTag::One(r) => r.fmt(f),
            AnyRefOrTag::Tag(kind, id) => write!(f, "@{}:{kind}:{id}", kinds::Tag::NAME),
        }
    }
}

impl Schema for AnyRefOrTag {
    fn shape() -> Shape {
        Shape::Ref {
            kinds: &[],
            id: String::shape,
        }
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader<'_>) -> Option<Self> {
        let r = substrate_data::schema::read_ref(v, cx, &[])?;
        if r.kind.as_str() != kinds::Tag::NAME {
            return Some(AnyRefOrTag::One(r));
        }
        let Some((kind, id)) = split(r.id.as_str()) else {
            cx.problem(format!("`{r}`: a tag is `@tag:<kind>:<id>`"));
            return None;
        };
        Some(AnyRefOrTag::Tag(kind.to_owned(), id.to_owned()))
    }
    fn write(&self) -> Value {
        match self {
            AnyRefOrTag::One(r) => Value::Ref(r.clone()),
            AnyRefOrTag::Tag(kind, id) => {
                Id::<kinds::Tag>::new(format!("{kind}{}{id}", yog_ids::sep::NAMESPACE)).write()
            }
        }
    }
}
