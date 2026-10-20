//! A record's frame codec in Java (`yog_data::frame`): `pack` writes its
//! fields in order, `unpack` reads them back — the rule
//! `yog_data::frame::unpack_shaped` reads by: what packs itself is packed,
//! everything else goes as its binary form (`putTagged`).

use std::fmt::Write;

use yog_data::Shape;
use yog_data::schema::{EnumShape, StructShape};

use super::field;
use super::types::{F32_BITS, TYPES, fits_int, from_data, java_type, to_data};


/// `pack`, `unpack`, `toFrame` and `fromFrame` for record `name` of `s`, indented by `pad`.
pub(super) fn codec(name: &str, s: &StructShape, pad: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "\n\n{pad}    public void pack(YogFrame.Packer p) {{\n{pad}        int at = p.begin();");
    for f in &s.fields {
        let _ = writeln!(out, "{pad}        {}", pack(&(f.shape)(), &format!("this.{}", field(f.name)), 0));
    }
    let _ = writeln!(out, "{pad}        p.end(at);\n{pad}    }}\n");
    let _ = writeln!(out, "{pad}    public static {name} unpack(YogFrame.Unpacker u) {{\n{pad}        int end = u.begin();");
    let mut args = Vec::new();
    for f in &s.fields {
        let shape = (f.shape)();
        let local = format!("f_{}", field(f.name));
        // The field's own type: an empty list or a null takes it from there.
        let java = java_type(&shape, &mut Vec::new());
        let _ = writeln!(out, "{pad}        {java} {local} = u.more(end) ? {} : {};", unpack(&shape, 0), missing(&shape));
        args.push(local);
    }
    let _ = writeln!(out, "{pad}        u.leave(end);\n{pad}        return new {name}({});\n{pad}    }}\n", args.join(", "));
    let _ = writeln!(
        out,
        "{pad}    public byte[] toFrame() {{\n{pad}        YogFrame.Packer p = new YogFrame.Packer();\n{pad}        pack(p);\n{pad}        return p.finish();\n{pad}    }}\n"
    );
    let _ = write!(
        out,
        "{pad}    public static {name} fromFrame(byte[] bytes) {{\n{pad}        YogFrame.Unpacker u = new YogFrame.Unpacker(bytes);\n{pad}        {name} v = unpack(u);\n{pad}        u.finish();\n{pad}        return v;\n{pad}    }}"
    );
    out
}

/// The statements packing `value` (of `shape`); `depth` names loop variables apart.
pub(super) fn pack(shape: &Shape, value: &str, depth: usize) -> String {
    let e = format!("e{depth}");
    match shape {
        Shape::Bool => format!("p.putBool({value});"),
        Shape::Int { .. } => format!("p.putI({value});"),
        Shape::Float { bits } if *bits == F32_BITS => format!("p.putF32({value});"),
        Shape::Float { .. } => format!("p.putF64({value});"),
        Shape::Str => format!("p.putStr({value});"),
        Shape::Bytes => format!("p.putBytes({value});"),
        Shape::Optional(inner) => format!("p.putBool({value} != null); if ({value} != null) {{ {} }}", pack(&inner(), value, depth)),
        Shape::List(item) => match item() {
            Shape::Int { min, .. } => format!("p.putUints({value}, {});", min < 0),
            Shape::Float { bits } if bits == F32_BITS => format!("p.putF32s({value});"),
            Shape::Float { .. } => format!("p.putF64s({value});"),
            item => format!("p.putU({value}.size()); for (var {e} : {value}) {{ {} }}", pack(&item, &e, depth + 1)),
        },
        Shape::Struct(_) => format!("{value}.pack(p);"),
        Shape::Enum(en) if en.tag.is_none() => format!("p.putU(switch ({value}) {{ {} default -> 0; }});", name_cases(en)),
        other => format!("p.putTagged({});", to_data(other, value, depth)),
    }
}

/// The expression unpacking a value of `shape`.
pub(super) fn unpack(shape: &Shape, depth: usize) -> String {
    match shape {
        Shape::Bool => "u.getBool()".into(),
        Shape::Int { min, max } if fits_int(*min, *max) => "(int) u.getI()".into(),
        Shape::Int { .. } => "u.getI()".into(),
        Shape::Float { bits } if *bits == F32_BITS => "u.getF32()".into(),
        Shape::Float { .. } => "u.getF64()".into(),
        Shape::Str => "u.getStr()".into(),
        Shape::Bytes => "u.getBytes()".into(),
        Shape::Optional(inner) => format!("(u.getBool() ? {} : null)", boxed(&inner(), depth)),
        Shape::List(item) => match item() {
            Shape::Int { min, max } if fits_int(min, max) => format!("u.getUints({}).stream().map(Long::intValue).toList()", min < 0),
            Shape::Int { min, .. } => format!("u.getUints({})", min < 0),
            Shape::Float { bits } if bits == F32_BITS => "u.getF32s()".into(),
            Shape::Float { .. } => "u.getF64s()".into(),
            item => format!("YogFrame.list(u, () -> {})", boxed(&item, depth + 1)),
        },
        Shape::Struct(s) => format!("{TYPES}.{}.unpack(u)", s.name),
        Shape::Enum(en) if en.tag.is_none() => format!("switch ((int) u.getU()) {{ {} default -> \"\"; }}", index_cases(en)),
        other => from_data(other, "u.getTagged()", depth),
    }
}

/// [`unpack`] as an object (a list's item, an optional's value): primitives boxed.
fn boxed(shape: &Shape, depth: usize) -> String {
    match shape {
        Shape::Bool => format!("Boolean.valueOf({})", unpack(shape, depth)),
        Shape::Int { min, max } if fits_int(*min, *max) => format!("Integer.valueOf({})", unpack(shape, depth)),
        Shape::Int { .. } => format!("Long.valueOf({})", unpack(shape, depth)),
        Shape::Float { bits } if *bits == F32_BITS => format!("Float.valueOf({})", unpack(shape, depth)),
        Shape::Float { .. } => format!("Double.valueOf({})", unpack(shape, depth)),
        _ => unpack(shape, depth),
    }
}

/// A host method's result `out` (an `Object`, its "none" already null) as
/// `shape`'s Java type: numbers to the width the reply has.
pub(super) fn from_host(shape: &Shape, out: &str) -> String {
    match shape {
        Shape::Bool => format!("(Boolean) {out}"),
        Shape::Int { min, max } if fits_int(*min, *max) => format!("((Number) {out}).intValue()"),
        Shape::Int { .. } => format!("((Number) {out}).longValue()"),
        Shape::Float { bits } if *bits == F32_BITS => format!("((Number) {out}).floatValue()"),
        Shape::Float { .. } => format!("((Number) {out}).doubleValue()"),
        Shape::Optional(inner) => format!("({out} == null ? null : {})", boxed_host(&inner(), out)),
        other => format!("({}) {out}", java_type(other, &mut Vec::new())),
    }
}

/// [`from_host`] as an object.
fn boxed_host(shape: &Shape, out: &str) -> String {
    match shape {
        Shape::Int { min, max } if fits_int(*min, *max) => format!("Integer.valueOf({})", from_host(shape, out)),
        Shape::Int { .. } => format!("Long.valueOf({})", from_host(shape, out)),
        Shape::Float { bits } if *bits == F32_BITS => format!("Float.valueOf({})", from_host(shape, out)),
        Shape::Float { .. } => format!("Double.valueOf({})", from_host(shape, out)),
        _ => from_host(shape, out),
    }
}

/// What a field an older writer didn't have holds.
pub(super) fn missing(shape: &Shape) -> String {
    match shape {
        Shape::Bool => "false".into(),
        Shape::Int { min, max } if fits_int(*min, *max) => "0".into(),
        Shape::Int { .. } => "0L".into(),
        Shape::Float { bits } if *bits == F32_BITS => "0.0f".into(),
        Shape::Float { .. } => "0.0".into(),
        Shape::Bytes => "new byte[0]".into(),
        Shape::List(_) => "java.util.List.of()".into(),
        Shape::Map(_) => "java.util.Map.of()".into(),
        Shape::Enum(en) if en.tag.is_none() => format!("{:?}", en.variants.first().map_or("", |v| v.name)),
        Shape::Str | Shape::Text | Shape::Image | Shape::Color | Shape::Res => "\"\"".into(),
        _ => "null".into(),
    }
}

/// `case "name" -> index;` for each variant.
fn name_cases(en: &EnumShape) -> String {
    en.variants.iter().enumerate().map(|(i, v)| format!("case {:?} -> {i};", v.name)).collect::<Vec<_>>().join(" ")
}

/// `case index -> "name";` for each variant.
fn index_cases(en: &EnumShape) -> String {
    en.variants.iter().enumerate().map(|(i, v)| format!("case {i} -> {:?};", v.name)).collect::<Vec<_>>().join(" ")
}
