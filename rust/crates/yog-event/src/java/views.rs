//! A record's view in Java (`Name.View`): the record read where its frame
//! is — the arena, in place — each field decoded only when it's asked for.
//! Made, it has stepped over the fields once (lists and records by their
//! lengths) and kept where each starts; a nested record is a view too, a
//! list of numbers is read item by item (`x(i)`, `xLength()`) without being
//! unpacked, a list of records is walked (`forEachX`). Everything else is
//! decoded as the record's own codec does, from where it starts.
//!
//! A view shares its reader with the views it hands out, so it's read on
//! the thread that made it, before that thread's next bus call.

use std::fmt::Write;

use yog_data::Shape;
use yog_data::schema::StructShape;

use super::field;
use super::frames::{missing, unpack};
use super::types::{F32_BITS, TYPES, fits_int, java_type};

/// How a field is kept by the view.
enum Kept<'a> {
    /// Numbers bit-packed: read one by one.
    Ints { signed: bool, wide: bool },
    /// Floats: read one by one (`f32`: a list of them, never widened).
    Floats { f32: bool },
    /// A record: viewed.
    Record(&'a StructShape),
    /// A list of records: walked.
    Records(&'a StructShape),
    /// Anything else: decoded when asked.
    Decoded,
}

fn kept<'a>(shape: &'a Shape, item: &'a Option<Shape>) -> Kept<'a> {
    match (shape, item) {
        (Shape::Struct(s), _) => Kept::Record(s),
        (Shape::List(_), Some(Shape::Int { min, max })) => Kept::Ints { signed: *min < 0, wide: !fits_int(*min, *max) },
        (Shape::List(_), Some(Shape::Float { bits })) => Kept::Floats { f32: *bits == F32_BITS },
        (Shape::List(_), Some(Shape::Struct(s))) => Kept::Records(s),
        _ => Kept::Decoded,
    }
}

/// `Name.View` for record `s`, indented by `pad`.
pub(super) fn view(name: &str, s: &StructShape, pad: &str) -> String {
    let fields: Vec<(String, Shape, Option<Shape>)> = s
        .fields
        .iter()
        .map(|f| {
            let shape = (f.shape)();
            let item = if let Shape::List(i) = &shape { Some(i()) } else { None };
            (field(f.name), shape, item)
        })
        .collect();
    let mut out = String::new();
    let _ = write!(
        out,
        "\n\n{pad}    /** {{@code {name}}} read where its frame is, each field when it's asked for ({{@code yog-event}}'s {{@code java/views.rs}}). */\n{pad}    public static final class View {{\n{pad}        private final YogFrame.Unpacker u;"
    );
    for (k, (_, shape, item)) in fields.iter().enumerate() {
        let _ = write!(out, "\n{pad}        private final int o{k};");
        if matches!(kept(shape, item), Kept::Ints { .. } | Kept::Floats { .. } | Kept::Records(_)) {
            let _ = write!(out, "\n{pad}        private final int h{k}, n{k}, b{k};");
        }
    }
    let _ = write!(out, "\n\n{pad}        public View(YogFrame.Unpacker u) {{\n{pad}            this.u = u;\n{pad}            int end = u.begin();");
    for (k, (_, shape, item)) in fields.iter().enumerate() {
        let _ = write!(out, "\n{pad}            {}", step(k, shape, item));
    }
    let _ = write!(out, "\n{pad}            u.leave(end);\n{pad}        }}");
    for (k, (f, shape, item)) in fields.iter().enumerate() {
        out.push_str(&accessors(k, f, shape, item, pad));
    }
    let _ = write!(out, "\n{pad}    }}");
    out
}

/// The constructor's statements for field `k`: where it starts, stepped over.
fn step(k: usize, shape: &Shape, item: &Option<Shape>) -> String {
    let skip = match kept(shape, item) {
        Kept::Ints { .. } => {
            return format!(
                "if (u.more(end)) {{ h{k} = u.position(); n{k} = (int) u.getU(); b{k} = u.getByte(); o{k} = u.position(); u.skip(YogFrame.packedBytes(n{k}, b{k})); }} else {{ h{k} = n{k} = b{k} = 0; o{k} = -1; }}"
            );
        }
        Kept::Floats { f32 } => {
            let narrow = if f32 { "1".to_owned() } else { "u.getBool() ? 1 : 0".to_owned() };
            return format!(
                "if (u.more(end)) {{ h{k} = u.position(); n{k} = u.count(); b{k} = {narrow}; o{k} = u.position(); u.skip(n{k} * (b{k} != 0 ? 4 : 8)); }} else {{ h{k} = n{k} = b{k} = 0; o{k} = -1; }}"
            );
        }
        Kept::Records(_) => {
            return format!(
                "if (u.more(end)) {{ h{k} = u.position(); n{k} = u.count(); int e{k} = u.listEnd(); b{k} = 0; o{k} = u.position(); u.skipTo(e{k}); }} else {{ h{k} = n{k} = b{k} = 0; o{k} = -1; }}"
            );
        }
        Kept::Record(_) | Kept::Decoded => skip(shape, 0),
    };
    format!("if (u.more(end)) {{ o{k} = u.position(); {skip} }} else {{ o{k} = -1; }}")
}

/// Statements stepping over a value of `shape`; `depth` names locals apart.
fn skip(shape: &Shape, depth: usize) -> String {
    match shape {
        Shape::Bool => "u.skip(1);".into(),
        Shape::Int { .. } | Shape::Str | Shape::Atom => "u.getU();".into(),
        Shape::Float { bits } if *bits == F32_BITS => "u.skip(4);".into(),
        Shape::Float { .. } => "u.skip(8);".into(),
        Shape::Enum(en) if en.tag.is_none() => "u.getU();".into(),
        Shape::Optional(inner) => format!("if (u.getBool()) {{ {} }}", skip(&inner(), depth + 1)),
        Shape::Struct(_) => format!("{{ int r{depth} = u.begin(); u.leave(r{depth}); }}"),
        Shape::List(item) => match item() {
            Shape::Int { .. } => format!("{{ int c{depth} = (int) u.getU(); int w{depth} = u.getByte(); u.skip(YogFrame.packedBytes(c{depth}, w{depth})); }}"),
            Shape::Float { bits } if bits == F32_BITS => "u.skip(u.count() * 4);".into(),
            Shape::Float { .. } => format!("{{ int c{depth} = u.count(); u.skip(c{depth} * (u.getBool() ? 4 : 8)); }}"),
            _ => "{ u.count(); u.skipTo(u.listEnd()); }".into(),
        },
        // Bytes, and what goes as its binary form.
        _ => "u.skipBytes();".into(),
    }
}

/// Field `k`'s methods.
fn accessors(k: usize, f: &str, shape: &Shape, item: &Option<Shape>, pad: &str) -> String {
    let java = java_type(shape, &mut Vec::new());
    let big = format!("{}{}", f[..1].to_uppercase(), &f[1..]);
    let decoded = |out: &mut String, from: &str| {
        let _ = write!(
            out,
            "\n\n{pad}        public {java} {f}() {{\n{pad}            if (o{k} < 0) return {};\n{pad}            u.seek({from});\n{pad}            return {};\n{pad}        }}",
            missing(shape),
            unpack(shape, 0)
        );
    };
    let mut out = String::new();
    match kept(shape, item) {
        Kept::Record(s) => {
            let _ = write!(
                out,
                "\n\n{pad}        public {TYPES}.{n}.View {f}() {{\n{pad}            if (o{k} < 0) return null;\n{pad}            u.seek(o{k});\n{pad}            return new {TYPES}.{n}.View(u);\n{pad}        }}",
                n = s.name
            );
        }
        Kept::Ints { signed, wide } => {
            decoded(&mut out, &format!("h{k}"));
            let (t, raw) = (if wide { "long" } else { "int" }, if signed { "YogFrame.unzigzag(v)" } else { "v" });
            let _ = write!(
                out,
                "\n\n{pad}        public int {f}Length() {{\n{pad}            return n{k};\n{pad}        }}\n\n{pad}        /** Item {{@code i}}, read in place. */\n{pad}        public {t} {f}(int i) {{\n{pad}            long v = u.packedAt(o{k}, n{k}, b{k}, i);\n{pad}            return ({t}) {raw};\n{pad}        }}"
            );
        }
        Kept::Floats { f32 } => {
            decoded(&mut out, &format!("h{k}"));
            let (t, read) = if f32 {
                ("float", format!("u.f32At(o{k} + 4 * i)"))
            } else {
                ("double", format!("b{k} != 0 ? (double) u.f32At(o{k} + 4 * i) : u.f64At(o{k} + 8 * i)"))
            };
            let _ = write!(
                out,
                "\n\n{pad}        public int {f}Length() {{\n{pad}            return n{k};\n{pad}        }}\n\n{pad}        /** Item {{@code i}}, read in place. */\n{pad}        public {t} {f}(int i) {{\n{pad}            if (i < 0 || i >= n{k}) throw new IndexOutOfBoundsException(i);\n{pad}            return {read};\n{pad}        }}"
            );
        }
        Kept::Records(s) => {
            decoded(&mut out, &format!("h{k}"));
            let _ = write!(
                out,
                "\n\n{pad}        public int {f}Length() {{\n{pad}            return n{k};\n{pad}        }}\n\n{pad}        /** Each item, viewed in turn. */\n{pad}        public void forEach{big}(java.util.function.Consumer<{TYPES}.{n}.View> each) {{\n{pad}            int at = o{k};\n{pad}            for (int j = 0; j < n{k}; j++) {{\n{pad}                u.seek(at);\n{pad}                {TYPES}.{n}.View v = new {TYPES}.{n}.View(u);\n{pad}                at = u.position();\n{pad}                each.accept(v);\n{pad}            }}\n{pad}        }}",
                n = s.name
            );
        }
        Kept::Decoded => decoded(&mut out, &format!("o{k}")),
    }
    out
}
