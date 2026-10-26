//! A set's generated code: a type with an accessor per entry, like a C#
//! project's `Resources.Designer.cs`.

use std::fmt::Write as _;

use super::placeholders::param_ident;
use super::{Entry, Kind, Value};

/// One line of a doc comment: no line breaks, no comment end.
fn doc_line(s: &str) -> String {
    let one: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    one.replace("*/", "* /")
}

/// What an entry holds, as its doc says.
fn what(e: &Entry) -> String {
    match e.kind {
        Kind::String => format!(
            "Looks up a localized string similar to \"{}\".",
            doc_line(e.text())
        ),
        Kind::Image => format!("The image `{}`.", e.name),
        Kind::File => format!("The file `{}`.", e.name),
        Kind::Sound => format!("The sound `{}`.", e.name),
        Kind::Video => format!("The video `{}`.", e.name),
        Kind::Color => format!("The color `{}`.", doc_line(e.text())),
        Kind::Number | Kind::Bool => match &e.value {
            Value::Num(n) => format!("The number `{n}`."),
            Value::Bool(b) => format!("The flag `{b}`."),
            Value::Str(v) => format!("The value `{}`.", doc_line(v)),
            Value::Bytes(b) => format!("{} bytes.", b.len()),
        },
    }
}

/// An entry's accessor's doc: what it holds, then its comment and where it is.
pub(super) fn doc(e: &Entry, shown: &str) -> String {
    let mut doc = format!("    /// {}\n    ///\n    /// `{}`", what(e), e.name);
    if let Some(comment) = &e.comment {
        let _ = write!(doc, " — {}", doc_line(comment));
    }
    let _ = writeln!(doc, " ({shown}:{})", e.line);
    doc
}

/// The opening of set `name`'s type, from the file `shown`.
pub(super) fn set_header(shown: &str, name: &str) -> String {
    format!(
        "/// The resources of `{shown}`.\n#[allow(dead_code)]\npub struct {name};\n\n#[allow(dead_code, non_snake_case, clippy::all)]\nimpl {name} {{\n"
    )
}

/// A text's accessor `acc`: its key's const, and a fn of its parameters
/// giving it in the player's language.
pub(super) fn text(acc: &str, key: &str, params: &[String], doc: &str) -> String {
    let upper = acc.to_ascii_uppercase();
    let mut code = format!(
        "    /// Translation key of [`Self::{acc}`]: `{key}`.\n    pub const {upper}_KEY: &'static str = {key:?};\n{doc}"
    );
    if params.is_empty() {
        let _ = writeln!(
            code,
            "    pub fn {acc}() -> ::std::string::String {{\n        ::yog_api::tr(Self::{upper}_KEY)\n    }}"
        );
        return code;
    }
    let idents: Vec<String> = params.iter().map(|p| param_ident(p)).collect();
    let sig = idents
        .iter()
        .map(|p| format!("{p}: impl ::std::fmt::Display"))
        .collect::<Vec<_>>()
        .join(", ");
    let args = idents
        .iter()
        .map(|p| format!("&{p}"))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        code,
        "    pub fn {acc}({sig}) -> ::std::string::String {{\n        ::yog_api::tr_args(Self::{upper}_KEY, &[{args}])\n    }}"
    );
    code
}

/// A medium's accessor: its id.
pub(super) fn medium(acc: &str, id: &str, doc: &str) -> String {
    format!("{doc}    pub const fn {acc}() -> &'static str {{\n        {id:?}\n    }}\n")
}

/// A value's const: a color, a number, a flag. None for an entry of
/// another kind, or not a value of its own.
pub(super) fn value(e: &Entry, upper: &str, doc: &str) -> Option<String> {
    let (ty, v) = match (e.kind, &e.value) {
        (Kind::Color, _) => ("u32", format!("0x{:08X}", super::color_of(e)?)),
        (Kind::Number, Value::Num(lit)) => match number(lit) {
            (true, v) => ("i64", v),
            (false, v) => ("f64", v),
        },
        (Kind::Bool, v) => ("bool", matches!(v, Value::Bool(true)).to_string()),
        _ => return None,
    };
    Some(format!("{doc}    pub const {upper}: {ty} = {v};\n"))
}

/// A number's literal, and whether it's whole (then 64-bit integer, else
/// a 64-bit float): the same in each language the sets are written in.
pub(super) fn number(lit: &str) -> (bool, String) {
    let whole = !lit.contains(['.', 'e', 'E']);
    match (whole, lit.parse::<i64>()) {
        (true, Ok(v)) => (true, v.to_string()),
        _ => (
            false,
            format!("{:?}", lit.parse::<f64>().unwrap_or_default()),
        ),
    }
}
