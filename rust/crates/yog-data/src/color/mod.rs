//! [`Color`]: `#RRGGBB`, `#AARRGGBB`, or a `.yres` colour.

use substrate_data::schema::NamedType;
use substrate_data::typed::{Spelled, classify};
use substrate_data::{Kind, Value};

use crate::{Res, kinds};

/// Hex digits of a colour without alpha, and with it.
const RGB_DIGITS: usize = 6;
const ARGB_DIGITS: usize = 8;
/// What a colour written without alpha is: fully opaque.
const OPAQUE: u32 = 0xFF00_0000;

/// A colour: ARGB, or a `.yres` colour.
#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Argb(u32),
    Res(Res),
}

impl Color {
    /// `#RRGGBB` (opaque) or `#AARRGGBB`.
    pub fn parse(s: &str) -> Option<u32> {
        let hex = s.strip_prefix(yog_ids::sep::TAG)?;
        let value = u32::from_str_radix(hex, 16).ok()?;
        match hex.len() {
            RGB_DIGITS => Some(OPAQUE | value),
            ARGB_DIGITS => Some(value),
            _ => None,
        }
    }

    /// `#AARRGGBB`.
    pub fn format(argb: u32) -> String {
        format!("{}{argb:08X}", yog_ids::sep::TAG)
    }
}

impl NamedType for Color {
    const NAME: &'static str = yog_ids::typed::COLOR;
    type Stored = Value;
    fn to_stored(&self) -> Value {
        match self {
            Color::Argb(c) => Value::Int(i64::from(*c)),
            Color::Res(r) => substrate_data::Schema::write(r),
        }
    }
    fn from_stored(stored: Value) -> Option<Self> {
        match stored {
            Value::Int(c) => u32::try_from(c).ok().map(Color::Argb),
            Value::Str(s) if s.starts_with(yog_ids::sep::TAG) => Color::parse(&s).map(Color::Argb),
            Value::Str(s) => match classify(&s).ok()? {
                Spelled::Ref(r) if r.kind.as_str() == kinds::Res::NAME => Res::parse(r.id.as_str()).map(Color::Res),
                _ => None,
            },
            Value::Ref(r) if r.kind.as_str() == kinds::Res::NAME => Res::parse(r.id.as_str()).map(Color::Res),
            _ => None,
        }
    }
}

substrate_data::named_schema!(Color);
