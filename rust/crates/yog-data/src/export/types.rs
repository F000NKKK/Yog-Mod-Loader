//! Yog Data types as TypeScript, from their schemas: a struct's interface,
//! an enum's names (and, tagged, its variants' union) — so a record the
//! tools write (a [`crate::Diagnostic`]) is described once, in Rust.

use crate::schema::{EnumShape, Field, Shape, StructShape};
use yog_codegen::Literals;
use yog_codegen_typescript::TypeScript;

/// `shape`'s TypeScript type.
fn ts_type(shape: &Shape) -> String {
    match shape {
        Shape::Bool => "boolean".into(),
        Shape::Int { .. } | Shape::Float { .. } => "number".into(),
        Shape::List(of) => format!("{}[]", ts_type(&of())),
        Shape::Map(of) => format!("Record<string, {}>", ts_type(&of())),
        Shape::Optional(of) => ts_type(&of()),
        Shape::Struct(s) => s.name.into(),
        Shape::Enum(e) => e.name.into(),
        Shape::Any => "unknown".into(),
        _ => "string".into(),
    }
}

/// A field, `?` when it may be left out.
fn field(f: &Field) -> String {
    let shape = (f.shape)();
    let optional = !f.required || matches!(shape, Shape::Optional(_));
    let doc = doc_line(f.doc, "  ");
    let mark = if optional { "?" } else { "" };
    format!("{doc}  {}{mark}: {};\n", f.name, ts_type(&shape))
}

fn doc_line(doc: &str, indent: &str) -> String {
    let doc = doc.trim();
    if doc.is_empty() {
        String::new()
    } else {
        format!("{indent}/** {} */\n", doc.replace('\n', " "))
    }
}

/// A struct's interface.
fn interface(s: &StructShape) -> String {
    let fields: String = s.fields.iter().map(field).collect();
    format!(
        "{}export interface {} {{\n{fields}}}\n",
        doc_line(s.doc, ""),
        s.name
    )
}

/// An enum: its names as constants (`severity.ERROR`), and its type — the
/// names, or for a tagged one each variant's object.
fn enumeration(e: &EnumShape) -> String {
    let mut out = doc_line(e.doc, "");
    let consts = lower_first(e.name);
    out.push_str(&format!("export const {consts} = {{\n"));
    for v in &e.variants {
        out.push_str(&format!(
            "  {}: {},\n",
            v.name.to_uppercase(),
            TypeScript::string(v.name)
        ));
    }
    out.push_str("} as const;\n");
    let variants: Vec<String> = e
        .variants
        .iter()
        .map(|v| match e.tag {
            None => TypeScript::string(v.name),
            Some(tag) => {
                let fields: String = v
                    .fields
                    .iter()
                    .map(|f| format!(" {}", field(f).trim()))
                    .collect();
                format!("{{ {tag}: {};{fields} }}", TypeScript::string(v.name))
            }
        })
        .collect();
    out.push_str(&format!(
        "export type {} = {};\n",
        e.name,
        variants.join(" | ")
    ));
    out
}

fn lower_first(name: &str) -> String {
    let mut c = name.chars();
    c.next()
        .map(|f| f.to_lowercase().chain(c).collect())
        .unwrap_or_default()
}

/// `shapes`' TypeScript, each a declaration.
pub(super) fn declarations(shapes: &[Shape]) -> String {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Struct(s) => Some(interface(s)),
            Shape::Enum(e) => Some(enumeration(e)),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
