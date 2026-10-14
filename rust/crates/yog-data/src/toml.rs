//! TOML read into the Yog Data model (feature `toml`) — for `yog.toml`,
//! which keeps the syntax people know from Cargo but gets the same
//! schemas, checks and references as every other format.
//!
//! Tables are maps in file order, arrays lists; a datetime becomes its
//! string. Every value keeps its span, so schema problems point at their
//! line just as in the JSON-shaped formats.

use serde_spanned::Spanned;
use toml::de::{DeTable, DeValue};

use crate::text::{Doc, LineCol, Span, Spans, SyntaxError};
use crate::value::{Map, Value};

/// Parses a TOML document.
pub fn parse(text: &str) -> Result<Doc, SyntaxError> {
    let table = DeTable::parse(text).map_err(|e| {
        let offset = e.span().map_or(0, |s| s.start);
        SyntaxError {
            message: e.message().trim().to_string(),
            at: LineCol::of(text, offset),
            offset,
        }
    })?;
    let span = to_span(table.span());
    let (value, children) = table_value(table.into_inner());
    Ok(Doc {
        value,
        spans: Spans {
            span,
            key: None,
            children,
        },
    })
}

fn to_span(r: std::ops::Range<usize>) -> Span {
    Span {
        start: r.start as u32,
        end: r.end as u32,
    }
}

fn table_value(table: DeTable<'_>) -> (Value, Vec<Spans>) {
    let mut map = Map::with_capacity(table.len());
    let mut spans = Vec::with_capacity(table.len());
    for (k, v) in table {
        let key = to_span(k.span());
        let (value, mut s) = convert(v);
        s.key = Some(key);
        map.push_unchecked(k.into_inner().as_ref().into(), value);
        spans.push(s);
    }
    (Value::Map(map), spans)
}

fn convert(v: Spanned<DeValue<'_>>) -> (Value, Spans) {
    let span = to_span(v.span());
    let (value, children) = match v.into_inner() {
        DeValue::String(s) => (Value::Str(s.into_owned()), Vec::new()),
        DeValue::Integer(n) => {
            let value = i64::from_str_radix(n.as_str(), n.radix())
                .map(Value::Int)
                .unwrap_or_else(|_| Value::Float(n.as_str().parse().unwrap_or(f64::NAN)));
            (value, Vec::new())
        }
        DeValue::Float(f) => (Value::Float(parse_float(f.as_str())), Vec::new()),
        DeValue::Boolean(b) => (Value::Bool(b), Vec::new()),
        DeValue::Datetime(d) => (Value::Str(d.to_string()), Vec::new()),
        DeValue::Array(items) => {
            let mut list = Vec::with_capacity(items.len());
            let mut spans = Vec::with_capacity(items.len());
            for item in items {
                let (v, s) = convert(item);
                list.push(v);
                spans.push(s);
            }
            (Value::List(list), spans)
        }
        DeValue::Table(t) => table_value(t),
    };
    (
        value,
        Spans {
            span,
            key: None,
            children,
        },
    )
}

fn parse_float(s: &str) -> f64 {
    match s.trim_start_matches('+') {
        "inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        "nan" | "-nan" => f64::NAN,
        other => other.replace('_', "").parse().unwrap_or(f64::NAN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Path;

    const MANIFEST: &str = r#"[mod]
id = "gm"
version = "0.1.0"
depends = ["yog-api", "hexmod"]

[run."fabric-1.21.1"]
memory = 0x800
scale = 1.5
released = 1979-05-27

[external.create]
items = ["create:cogwheel"]
"#;

    #[test]
    fn reads_tables_in_order_with_positions() {
        let doc = parse(MANIFEST).unwrap();
        let v = &doc.value;
        assert_eq!(
            v.as_map().unwrap().keys().collect::<Vec<_>>(),
            ["mod", "run", "external"]
        );
        assert_eq!(
            v.get("mod")
                .unwrap()
                .get("depends")
                .unwrap()
                .as_list()
                .unwrap()
                .len(),
            2
        );
        let run = v.get("run").unwrap().get("fabric-1.21.1").unwrap();
        assert_eq!(run.get("memory"), Some(&Value::Int(2048)));
        assert_eq!(run.get("scale"), Some(&Value::Float(1.5)));
        assert_eq!(run.get("released"), Some(&Value::Str("1979-05-27".into())));
        let at = doc.locate(
            MANIFEST,
            &Path::root().key("run").key("fabric-1.21.1").key("scale"),
        );
        assert_eq!(at, LineCol { line: 8, col: 9 });
        let at = doc.locate(
            MANIFEST,
            &Path::root()
                .key("external")
                .key("create")
                .key("items")
                .index(0),
        );
        assert_eq!(at, LineCol { line: 12, col: 10 });
    }

    #[test]
    fn syntax_errors_say_where() {
        let e = parse("[mod]\nid = \"gm\"\nid = \"x\"\n").unwrap_err();
        assert_eq!(e.at.line, 3, "{e}");
        let e = parse("[mod\nid = 1\n").unwrap_err();
        assert_eq!(e.at.line, 1, "{e}");
    }
}
