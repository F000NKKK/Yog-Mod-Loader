//! A payload field's Java type, and the expressions turning it into Yog
//! Data's values (`toData`) and back (`fromData`).

use yog_data::Shape;
use yog_data::schema::StructShape;

/// Bits of a 32-bit float: a Java `float`.
pub(super) const F32_BITS: u32 = 32;

/// The class holding the payloads' records every message shares (`BlockPos`…).
pub(super) const TYPES: &str = "YogTypes";

/// A list of numbers' Java type: a primitive array (`int[]`, `float[]`…).
pub(super) fn number_array(item: &Shape) -> Option<&'static str> {
    match item {
        Shape::Int { min, max } if fits_int(*min, *max) => Some("int[]"),
        Shape::Int { .. } => Some("long[]"),
        Shape::Float { bits } if *bits == F32_BITS => Some("float[]"),
        Shape::Float { .. } => Some("double[]"),
        _ => None,
    }
}

/// The Java type of `shape`; the records it needs go to `records`.
pub(super) fn java_type(shape: &Shape, records: &mut Vec<StructShape>) -> String {
    if let Shape::List(f) = shape
        && let Some(array) = number_array(&f())
    {
        return array.into();
    }
    match shape {
        Shape::Bool => "boolean".into(),
        Shape::Int { min, max } if fits_int(*min, *max) => "int".into(),
        Shape::Int { .. } => "long".into(),
        Shape::Float { bits } if *bits == F32_BITS => "float".into(),
        Shape::Float { .. } => "double".into(),
        Shape::Bytes => "byte[]".into(),
        Shape::Any => "Object".into(),
        Shape::List(f) => format!("java.util.List<{}>", boxed(&java_type(&f(), records))),
        Shape::Map(f) => format!(
            "java.util.Map<String, {}>",
            boxed(&java_type(&f(), records))
        ),
        Shape::Optional(f) => boxed(&java_type(&f(), records)),
        Shape::Struct(s) => {
            if !records.iter().any(|r| same(r, s)) {
                records.push(s.clone());
            }
            format!("{TYPES}.{}", s.name)
        }
        _ => "String".into(),
    }
}

pub(super) fn fits_int(min: i64, max: i64) -> bool {
    min >= i64::from(i32::MIN) && max <= i64::from(i32::MAX)
}

/// Whether `a` and `b` are one payload struct: its name and fields.
pub(super) fn same(a: &StructShape, b: &StructShape) -> bool {
    a.name == b.name
        && a.fields
            .iter()
            .map(|f| f.name)
            .eq(b.fields.iter().map(|f| f.name))
}

/// The object type standing for a primitive (in a list, or nullable).
fn boxed(java: &str) -> String {
    match java {
        "boolean" => "Boolean",
        "int" => "Integer",
        "long" => "Long",
        "double" => "Double",
        "float" => "Float",
        other => other,
    }
    .into()
}

/// Whether a value of `shape` needs converting to be a Yog Data value.
fn converts(shape: &Shape) -> bool {
    match shape {
        Shape::Struct(_) => true,
        Shape::List(f) if number_array(&f()).is_some() => true,
        Shape::List(f) | Shape::Map(f) | Shape::Optional(f) => converts(&f()),
        _ => false,
    }
}

/// `value` (of `shape`) as a Yog Data value; `depth` names nested lambdas' parameters apart.
pub(super) fn to_data(shape: &Shape, value: &str, depth: usize) -> String {
    if !converts(shape) {
        return value.into();
    }
    let e = format!("e{depth}");
    match shape {
        Shape::Struct(_) => format!("{value}.toData()"),
        Shape::List(f) if number_array(&f()).is_some() => format!("YogData.listOf({value})"),
        Shape::List(f) => format!(
            "{value}.stream().map({e} -> {}).toList()",
            to_data(&f(), &e, depth + 1)
        ),
        Shape::Map(f) => format!(
            "{value}.entrySet().stream().collect(java.util.stream.Collectors.toMap(java.util.Map.Entry::getKey, {e} -> {}))",
            to_data(&f(), &format!("{e}.getValue()"), depth + 1)
        ),
        Shape::Optional(f) => format!("({value} == null ? null : {})", to_data(&f(), value, depth)),
        _ => value.into(),
    }
}

/// `value` (a Yog Data value) read as `shape`'s Java type.
pub(super) fn from_data(shape: &Shape, value: &str, depth: usize) -> String {
    let e = format!("e{depth}");
    match shape {
        Shape::Bool => format!("YogData.bool({value}, false)"),
        Shape::Int { min, max } if fits_int(*min, *max) => {
            format!("(int) YogData.integer({value}, 0)")
        }
        Shape::Int { .. } => format!("YogData.integer({value}, 0)"),
        Shape::Float { bits } if *bits == F32_BITS => format!("(float) YogData.number({value}, 0)"),
        Shape::Float { .. } => format!("YogData.number({value}, 0)"),
        Shape::Bytes => format!("({value} instanceof byte[] {e} ? {e} : new byte[0])"),
        Shape::Any => value.into(),
        Shape::List(f) if let Some(array) = number_array(&f()) => {
            format!("YogData.{}({value})", array_reader(array))
        }
        Shape::List(f) => format!(
            "YogData.list({value}).stream().map({e} -> {}).toList()",
            from_data(&f(), &e, depth + 1)
        ),
        Shape::Map(f) => format!(
            "YogData.map({value}).entrySet().stream().collect(java.util.stream.Collectors.toMap(java.util.Map.Entry::getKey, {e} -> {}))",
            from_data(&f(), &format!("{e}.getValue()"), depth + 1)
        ),
        Shape::Optional(f) => format!(
            "({value} == null ? null : {})",
            from_data(&f(), value, depth)
        ),
        Shape::Struct(s) => format!("{TYPES}.{}.fromData({value})", s.name),
        _ => format!("YogData.str({value}, \"\")"),
    }
}

/// `YogData`'s reader of a number array type.
fn array_reader(array: &str) -> &'static str {
    match array {
        "int[]" => "ints",
        "long[]" => "longs",
        "float[]" => "floats",
        _ => "doubles",
    }
}

/// What a field of `shape` holds when it isn't null, if it may be (it's left out of the payload then).
pub(super) fn nullable(shape: &Shape) -> Option<Shape> {
    match shape {
        Shape::Optional(f) => Some(f()),
        _ => None,
    }
}
