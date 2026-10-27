//! [`Text`]: what a player reads, and [`Lang`]: a translation key.

use substrate_data::binary::{Decoder, Writer};
use substrate_data::frame::{Packer, Unpacker};
use substrate_data::schema::{Expansion, NamedType};
use substrate_data::typed::{Spelled, classify};
use substrate_data::{Id, Kind, Reader, Ref, Schema, Shape, Value, ValueRef};

use crate::{Res, kinds};

/// A translation key of the game's or a mod's lang files: `@lang:gui.done`.
pub type Lang = Id<kinds::Lang>;

/// A text a player reads: Yog text as written (`{b|bold}`), a `.yres`
/// text (`@res:Strings.title`), the name of some content
/// (`@item:minecraft:stick` shows "Stick"), or a translation key
/// (`@lang:gui.done`).
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

impl Text {
    /// The reference it is, if it's one (`@res:…`, `@lang:…`, a name's).
    pub fn to_ref(&self) -> Option<Ref> {
        match self {
            Text::Literal(_) => None,
            Text::Res(r) => Some(Id::<kinds::Res>::new(r.name()).to_ref()),
            Text::NameOf(r) => Some(r.clone()),
            Text::Lang(k) => Some(Lang::new(k.clone()).to_ref()),
        }
    }

    /// What the tool reading it makes of it: its words as written, else
    /// what the resolver expands its reference to (a `.yres` text's words
    /// or translation key, an item's name), else its reference as written.
    pub fn resolve(self, cx: &mut Reader<'_>) -> Option<String> {
        let Some(r) = self.to_ref() else {
            return match self {
                Text::Literal(s) => Some(s),
                _ => None,
            };
        };
        match cx.expand_ref(&r) {
            Expansion::Kept => Some(r.to_string()),
            Expansion::Value(Value::Str(s)) => Some(s),
            Expansion::Value(other) => {
                cx.problem(format!("`{r}` expands to {}, not a text", other.kind()));
                None
            }
            Expansion::Failed => None,
        }
    }

    /// What reference `r` is as a text (a resolver's `expand` gets the
    /// reference a text is).
    pub fn of_ref(r: Ref) -> Option<Text> {
        let kind = r.kind.as_str();
        if kind == kinds::Res::NAME {
            return Res::parse(r.id.as_str()).map(Text::Res);
        }
        if kind == kinds::Lang::NAME {
            return Some(Text::Lang(r.id.as_str().to_owned()));
        }
        Some(Text::NameOf(r))
    }
}

impl NamedType for Text {
    const NAME: &'static str = yog_ids::typed::TEXT;
    type Stored = Value;
    fn to_stored(&self) -> Value {
        match self {
            Text::Literal(s) => Value::Str(substrate_data::typed::escape_at(s)),
            Text::Res(r) => substrate_data::Schema::write(r),
            Text::NameOf(r) => Value::Ref(r.clone()),
            Text::Lang(k) => substrate_data::Schema::write(&Lang::new(k.clone())),
        }
    }
    fn from_stored(stored: Value) -> Option<Self> {
        match stored {
            Value::Ref(r) => Text::of_ref(r),
            Value::Str(s) => match classify(&s).ok()? {
                Spelled::Plain(p) => Some(Text::Literal(p)),
                Spelled::Ref(r) => Text::of_ref(r),
                Spelled::Bytes(_) => None,
            },
            _ => None,
        }
    }
}

/// Read as its reference when it's one, so a tool sees every reference a
/// text makes; every other form is its stored form's.
impl Schema for Text {
    fn shape() -> Shape {
        Shape::named::<Text>()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader<'_>) -> Option<Self> {
        let is_ref = match v {
            ValueRef::Ref { .. } => true,
            ValueRef::Str(s) => matches!(classify(s), Ok(Spelled::Ref(_))),
            _ => false,
        };
        if !is_ref {
            return String::read(v, cx).map(Text::Literal);
        }
        let r = substrate_data::schema::read_ref(v, cx, &[])?;
        Text::of_ref(r.clone()).or_else(|| {
            cx.problem(format!("`{r}` isn't a text"));
            None
        })
    }
    fn write(&self) -> Value {
        self.to_stored()
    }
    fn encode(&self, w: &mut Writer) {
        self.to_stored().encode(w);
    }
    fn decode(d: &mut Decoder<'_>) -> Option<Self> {
        Value::decode(d).and_then(Text::from_stored)
    }
    fn pack(&self, p: &mut Packer) {
        self.to_stored().pack(p);
    }
    fn unpack(u: &mut Unpacker<'_>) -> Option<Self> {
        Value::unpack(u).and_then(Text::from_stored)
    }
}

/// A field that is a text, kept resolved ([`Text::resolve`]):
/// `#[data(with = "yog_data::TextOf")]`.
#[derive(Debug)]
pub struct TextOf;

impl substrate_data::with::With<String> for TextOf {
    fn shape() -> Shape {
        Text::shape()
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader<'_>) -> Option<String> {
        Text::read(v, cx)?.resolve(cx)
    }
    fn write(f: &String) -> Value {
        Text::Literal(f.clone()).write()
    }
}
