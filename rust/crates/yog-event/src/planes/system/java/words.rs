//! The words of the enums of names the shared records hold, as Java
//! constants (`EntityKind.LIVING`): a record keeps such a field as its
//! word, and the host compares it with these instead of a string of its own.

use std::fmt::Write;

use yog_data::Shape;
use yog_data::schema::{EnumShape, StructShape};

/// Every enum of names `records`' fields hold (through lists, maps and optionals), once each.
fn enums(records: &[StructShape]) -> Vec<EnumShape> {
    let mut out: Vec<EnumShape> = Vec::new();
    for r in records {
        for f in &r.fields {
            collect(&(f.shape)(), &mut out);
        }
    }
    out
}

fn collect(shape: &Shape, out: &mut Vec<EnumShape>) {
    match shape {
        Shape::List(f) | Shape::Map(f) | Shape::Optional(f) => collect(&f(), out),
        Shape::Enum(en) if en.tag.is_none() && !out.iter().any(|e| e.name == en.name) => {
            out.push(en.clone());
        }
        _ => {}
    }
}

/// `multiply_base` → `MULTIPLY_BASE`.
fn constant(word: &str) -> String {
    word.to_uppercase()
}

/// A class of constants for each enum of names `records` hold, indented by `pad`.
pub(super) fn classes(records: &[StructShape], pad: &str) -> String {
    let mut out = String::new();
    for en in enums(records) {
        if !en.doc.is_empty() {
            let _ = write!(out, "\n{pad}/** {} */", en.doc);
        }
        let name = en.name;
        let _ = write!(
            out,
            "\n{pad}public static final class {name} {{\n{pad}    private {name}() {{}}\n"
        );
        for v in &en.variants {
            let _ = writeln!(
                out,
                "{pad}    public static final String {} = {:?};",
                constant(v.name),
                v.name
            );
        }
        let _ = writeln!(out, "{pad}}}");
    }
    out
}
