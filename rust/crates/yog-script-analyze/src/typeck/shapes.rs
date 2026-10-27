//! Yog's shapes (yog-data's description of a value) as Yog Script's types.

use yog_data::Shape;

use super::model::Type;
use yog_script_ids::system::{DICTIONARY, LIST};

impl Type {
    /// The type a value of `shape` has in a `.ys`.
    pub(crate) fn of_shape(shape: &Shape) -> Type {
        match shape {
            Shape::Bool => Type::Bool,
            Shape::Int { min, max } => {
                if *min >= i64::from(i32::MIN) && *max <= i64::from(i32::MAX) {
                    Type::I32
                } else {
                    Type::I64
                }
            }
            Shape::Float { bits } if *bits <= u32::BITS => Type::F32,
            Shape::Float { .. } => Type::F64,
            Shape::Str | Shape::Atom => Type::String,
            Shape::Text => Type::own(yog_script_ids::system::TEXT_TYPE.to_string()),
            Shape::Ref([kind]) => Type::Content((*kind).to_string()),
            Shape::Tag([kind]) => Type::Tag((*kind).to_string()),
            Shape::Tag(_) => Type::Tag(String::new()),
            Shape::List(inner) => Type::Own(LIST.into(), vec![Type::of_shape(&inner())]),
            Shape::Map(inner) => Type::Own(
                DICTIONARY.into(),
                vec![Type::String, Type::of_shape(&inner())],
            ),
            Shape::Optional(inner) => Type::of_shape(&inner()).nullable(),
            Shape::Struct(s) => Type::Yog(s.name.to_string()),
            Shape::Enum(e) => Type::Yog(e.name.to_string()),
            _ => Type::Unknown,
        }
    }
}
