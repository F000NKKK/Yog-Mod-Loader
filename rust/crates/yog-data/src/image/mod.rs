//! [`Image`]: a picture a screen shows.

use substrate_data::schema::NamedType;
use substrate_data::typed::{Spelled, classify};
use substrate_data::{Id, Kind, Ref, Value};

use crate::{Res, kinds};

/// An image: a source (`@image:sprite:button`, `@image:mymod:gui/logo`,
/// `@image:item:minecraft:stick`) or a `.yres` image (`@res:Images.logo`).
#[derive(Debug, Clone, PartialEq)]
pub enum Image {
    Source(String),
    Res(Res),
}

impl Image {
    /// What reference `r` is as an image.
    fn of_ref(r: &Ref) -> Option<Image> {
        let (kind, id) = (r.kind.as_str(), r.id.as_str());
        if kind == kinds::Res::NAME {
            return Res::parse(id).map(Image::Res);
        }
        (kind == kinds::Image::NAME).then(|| Image::Source(id.to_owned()))
    }
}

impl NamedType for Image {
    const NAME: &'static str = yog_ids::typed::IMAGE;
    type Stored = Value;
    fn to_stored(&self) -> Value {
        match self {
            Image::Source(s) => substrate_data::Schema::write(&Id::<kinds::Image>::new(s.clone())),
            Image::Res(r) => substrate_data::Schema::write(r),
        }
    }
    fn from_stored(stored: Value) -> Option<Self> {
        match stored {
            Value::Ref(r) => Image::of_ref(&r),
            Value::Str(s) => match classify(&s).ok()? {
                Spelled::Ref(r) => Image::of_ref(&r),
                Spelled::Plain(_) | Spelled::Bytes(_) => None,
            },
            _ => None,
        }
    }
}

substrate_data::named_schema!(Image);
