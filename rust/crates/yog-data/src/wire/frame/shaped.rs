//! A frame read by its [`Shape`] alone, as a [`Value`] — what a tool (the
//! IDE, `yog bus`) shows of a message it has no code for, and the rule the
//! generated Java codecs follow: what packs itself is read as packed,
//! everything else as its binary form.

use crate::model::value::{Map, Value};
use crate::schema::{EnumShape, Shape, StructShape};

use super::Unpacker;

/// Bits of a 32-bit float.
const F32_BITS: u32 = 32;

/// The frame `bytes` read as a value of `shape`.
pub fn read_shaped(bytes: &[u8], shape: &Shape) -> Option<Value> {
    let mut u = Unpacker::open(bytes)?;
    let v = unpack_shaped(&mut u, shape)?;
    u.at_end().then_some(v)
}

/// The next value, of `shape`.
pub fn unpack_shaped(u: &mut Unpacker<'_>, shape: &Shape) -> Option<Value> {
    Some(match shape {
        Shape::Bool => Value::Bool(u.get_bool()?),
        Shape::Int { .. } => Value::Int(u.get_i()?),
        Shape::Float { bits } if *bits == F32_BITS => Value::Float(f64::from(u.get_f32()?)),
        Shape::Float { .. } => Value::Float(u.get_f64()?),
        Shape::Str => Value::Str(u.get_str()?.to_owned()),
        Shape::Atom => Value::Str(
            crate::Atom::from_number(u32::try_from(u.get_u()?).ok()?)?
                .as_str()
                .to_owned(),
        ),
        Shape::Bytes => Value::Bytes(u.get_bytes()?.to_vec()),
        Shape::Optional(inner) => {
            if u.get_bool()? {
                unpack_shaped(u, &inner())?
            } else {
                Value::Null
            }
        }
        Shape::List(item) => list(u, &item())?,
        Shape::Struct(s) => record(u, s)?,
        Shape::Enum(e) if e.tag.is_none() => names(u, e)?,
        _ => u.get_tagged::<Value>()?,
    })
}

/// A list: numbers packed, anything else value by value.
fn list(u: &mut Unpacker<'_>, item: &Shape) -> Option<Value> {
    Some(Value::List(match item {
        Shape::Int { min, .. } if *min < 0 => {
            u.get_uints(|v| Some(Value::Int(crate::binary::unzigzag(v))))?
        }
        Shape::Int { .. } => u.get_uints(|v| i64::try_from(v).ok().map(Value::Int))?,
        Shape::Float { bits } if *bits == F32_BITS => u
            .get_f32s()?
            .into_iter()
            .map(|f| Value::Float(f64::from(f)))
            .collect(),
        Shape::Float { .. } => u.get_f64s()?.into_iter().map(Value::Float).collect(),
        _ => {
            let (n, end) = u.list()?;
            let items = (0..n)
                .map(|_| unpack_shaped(u, item))
                .collect::<Option<_>>()?;
            u.skip_to(end)?;
            items
        }
    }))
}

/// A record: its fields in order, as a map by their names; fields the shape
/// doesn't know skipped, missing ones left out.
fn record(u: &mut Unpacker<'_>, s: &StructShape) -> Option<Value> {
    let end = u.begin()?;
    let mut m = Map::new();
    for field in &s.fields {
        if !u.more(end) {
            break;
        }
        m.insert(field.name, unpack_shaped(u, &(field.shape)())?);
    }
    u.leave(end)?;
    Some(Value::Map(m))
}

/// An enum of names: its variant's name, by index.
fn names(u: &mut Unpacker<'_>, e: &EnumShape) -> Option<Value> {
    let i = usize::try_from(u.get_u()?).ok()?;
    Some(Value::Str(e.variants.get(i)?.name.to_owned()))
}
