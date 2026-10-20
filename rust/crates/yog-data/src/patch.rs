//! Editing Yog Data text in place: [`patch`] changes a document to hold a
//! new value and keeps everything that didn't change as it was — comments,
//! key order, number spelling, layout. What changed is written by
//! [`format`], the way Yog's editors write a value.

use crate::text::{Spans, write_string};
use crate::value::{Map, Value};

/// A line no longer than this is written on one line by [`format`].
const ONE_LINE: usize = 72;
/// One indent step.
const INDENT: &str = "  ";
/// Numbers are written to this many decimals.
const DECIMALS: f64 = 10_000.0;

/// `value` as Yog's editors write it at indent `level` (two spaces each):
/// short lists of numbers and short objects of scalars on one line
/// (`"uv": [0, 0, 16, 16]`), the rest one entry a line.
pub fn format(value: &Value, level: usize) -> String {
    if let Some(one) = inline(value) {
        return one;
    }
    let pad = INDENT.repeat(level + 1);
    let end = INDENT.repeat(level);
    match value {
        Value::List(items) => {
            let body: Vec<String> = items
                .iter()
                .map(|v| format!("{pad}{}", format(v, level + 1)))
                .collect();
            format!("[\n{}\n{end}]", body.join(",\n"))
        }
        Value::Map(map) => {
            let body: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{pad}{}: {}", quoted(k), format(v, level + 1)))
                .collect();
            format!("{{\n{}\n{end}}}", body.join(",\n"))
        }
        other => scalar(other).unwrap_or_default(),
    }
}

fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    write_string(s, &mut out);
    out
}

fn number(n: f64) -> String {
    let rounded = (n * DECIMALS).round() / DECIMALS;
    if rounded.fract() == 0.0 && rounded.abs() < 1e15 {
        format!("{}", rounded as i64)
    } else {
        rounded.to_string()
    }
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some("null".into()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Int(n) => Some(n.to_string()),
        Value::Float(f) if f.is_finite() => Some(number(*f)),
        Value::Float(_) => Some("null".into()),
        Value::List(_) | Value::Map(_) => None,
        // As the source has it: a typed value (`@item:…`) or an escaped `@@`, unread.
        Value::Str(s) => Some(quoted(s)),
        other => Some(crate::text::to_string(other)),
    }
}

fn inline(v: &Value) -> Option<String> {
    if let Some(s) = scalar(v) {
        return Some(s);
    }
    let out = match v {
        Value::List(items) if items.is_empty() => return Some("[]".into()),
        Value::List(items) => {
            if !items
                .iter()
                .all(|x| matches!(x, Value::Int(_) | Value::Float(_) | Value::Bool(_)))
            {
                return None;
            }
            format!(
                "[{}]",
                items
                    .iter()
                    .filter_map(scalar)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Value::Map(map) if map.is_empty() => return Some("{}".into()),
        Value::Map(map) => {
            let mut parts = Vec::with_capacity(map.len());
            for (k, x) in map.iter() {
                if matches!(x, Value::Map(_)) {
                    return None;
                }
                parts.push(format!("{}: {}", quoted(k), inline(x)?));
            }
            format!("{{ {} }}", parts.join(", "))
        }
        _ => return None,
    };
    (out.len() <= ONE_LINE).then_some(out)
}

/// `value` as compact JSON for a tool: maps in their order, strings as the
/// source has them (a typed value's text unread).
pub fn to_json(value: &Value) -> String {
    let mut out = String::new();
    json_into(value, &mut out);
    out
}

fn json_into(value: &Value, out: &mut String) {
    match value {
        Value::List(items) => {
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                json_into(v, out);
            }
            out.push(']');
        }
        Value::Map(map) => {
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                json_into(v, out);
            }
            out.push('}');
        }
        Value::Str(s) => write_string(s, out),
        Value::Float(f) if f.is_finite() => out.push_str(&f.to_string()),
        Value::Float(_) => out.push_str("null"),
        other => out.push_str(&crate::text::to_string(other)),
    }
}

/// Whether two values are the same, a number however it's spelled (`1` = `1.0`).
pub fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
            a.as_f64() == b.as_f64()
        }
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((ka, a), (kb, b))| ka == kb && same(a, b))
        }
        _ => a == b,
    }
}

/// `text` changed to hold `next`, the rest as it was. Maps and lists are
/// edited entry by entry (a kept entry keeps the comments before it; a
/// removed one takes them along). Text that doesn't parse is written anew.
pub fn patch(text: &str, next: &Value) -> String {
    let Ok(doc) = crate::text::parse(text) else {
        return format!("{}\n", format(next, 0));
    };
    let (start, end) = range(&doc.spans);
    format!(
        "{}{}{}",
        &text[..start],
        rewrite(text, &doc.value, &doc.spans, next),
        &text[end..]
    )
}

fn range(s: &Spans) -> (usize, usize) {
    (s.span.start as usize, s.span.end as usize)
}

/// The indent of the line `at` is on.
fn line_indent(text: &str, at: usize) -> &str {
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let line = &text[start..at];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

fn level_of(indent: &str) -> usize {
    indent
        .chars()
        .map(|c| if c == '\t' { INDENT.len() } else { 1 })
        .sum::<usize>()
        .div_ceil(INDENT.len())
}

/// Past whitespace and comments from `i`.
fn skip_blank(text: &str, mut i: usize) -> usize {
    let b = text.as_bytes();
    loop {
        match (b.get(i), b.get(i + 1)) {
            (Some(b' ' | b'\t' | b'\n' | b'\r'), _) => i += 1,
            (Some(b'/'), Some(b'/')) => i = text[i..].find('\n').map_or(text.len(), |n| i + n),
            (Some(b'/'), Some(b'*')) => {
                i = text[i + 2..]
                    .find("*/")
                    .map_or(text.len(), |n| i + 2 + n + 2)
            }
            _ => return i,
        }
    }
}

/// A member as rewritten: its text, then what followed it (a comment) — the comma goes between.
struct Member {
    body: String,
    trail: String,
}

fn join(out: &[Member], trailing_comma: bool) -> String {
    let mut s = String::new();
    for (i, m) in out.iter().enumerate() {
        s.push_str(&m.body);
        if i + 1 < out.len() || trailing_comma {
            s.push(',');
        }
        s.push_str(&m.trail);
    }
    s
}

/// A container's members as text: each one's lead (the comments above it)
/// and trail (after it, up to its comma); whether the last has a comma;
/// what's left before the bracket.
struct Members {
    segments: Vec<(String, String)>,
    comma: bool,
    tail: String,
}

fn members(text: &str, node: (usize, usize), spans: &[(usize, usize)]) -> Members {
    let mut segments = Vec::with_capacity(spans.len());
    let mut at = node.0 + 1;
    let mut comma = false;
    for &(start, end) in spans {
        let lead = text[at..start].to_string();
        let after = skip_blank(text, end);
        comma = text.as_bytes().get(after) == Some(&b',');
        // A comment on the member's own line stays with it; the rest goes to the next one.
        let eol = text[end..].find('\n').map(|n| end + n);
        let stop = if comma {
            after
        } else {
            eol.map_or(after, |e| after.min(e))
        };
        let mut trail = text[end..stop].to_string();
        at = if comma { after + 1 } else { stop };
        if comma {
            let eol = text[at..].find('\n').map_or(text.len(), |n| at + n);
            let rest = &text[at..eol];
            let t = rest.trim();
            let comment = t.starts_with("//")
                || (t.starts_with("/*") && t.ends_with("*/") && t.matches("*/").count() == 1);
            if comment && eol < node.1 {
                trail.push_str(rest);
                at = eol;
            }
        }
        segments.push((lead, trail));
    }
    Members {
        segments,
        comma,
        tail: text[at..node.1 - 1].to_string(),
    }
}

/// The new text of the value `old` (at `spans`) holding `next`.
fn rewrite(text: &str, old: &Value, spans: &Spans, next: &Value) -> String {
    let node = range(spans);
    if same(old, next) {
        return text[node.0..node.1].to_string();
    }
    let level = level_of(line_indent(text, node.0));
    let fresh = || format(next, level);
    if !text[node.0..node.1].contains('\n') {
        return fresh();
    }
    match (old, next) {
        (Value::Map(old), Value::Map(next))
            if !old.is_empty() && spans.children.len() == old.len() =>
        {
            rewrite_map(text, node, old, spans, next)
        }
        (Value::List(old), Value::List(next))
            if !old.is_empty() && spans.children.len() == old.len() =>
        {
            rewrite_list(text, node, old, spans, next)
        }
        _ => fresh(),
    }
}

fn rewrite_map(text: &str, node: (usize, usize), old: &Map, spans: &Spans, next: &Map) -> String {
    let keys: Vec<&str> = old.keys().collect();
    let key_start = |c: &Spans| c.key.map_or(c.span.start, |k| k.start) as usize;
    let ranges: Vec<(usize, usize)> = spans
        .children
        .iter()
        .map(|c| (key_start(c), c.span.end as usize))
        .collect();
    let parts = members(text, node, &ranges);
    let pad = line_indent(text, ranges[0].0);
    let gone = |i: usize| i < keys.len() && next.get(keys[i]).is_none();
    let mut out = Vec::with_capacity(next.len());
    // The old entry last written: a new key right after it, in the place of one that's gone, is that one renamed.
    let mut last: Option<usize> = None;
    for (k, v) in next.iter() {
        let after = last.map_or(0, |l| l + 1);
        let found = keys.iter().position(|o| *o == k);
        let renamed = found.is_none() && gone(after);
        let Some(i) = found.or(renamed.then_some(after)) else {
            out.push(Member {
                body: format!("\n{pad}{}: {}", quoted(k), format(v, level_of(pad))),
                trail: String::new(),
            });
            continue;
        };
        last = Some(i);
        let child = &spans.children[i];
        let value_start = child.span.start as usize;
        let key = if renamed {
            let key_end = child.key.map_or(value_start, |s| s.end as usize);
            format!("{}{}", quoted(k), &text[key_end..value_start])
        } else {
            text[ranges[i].0..value_start].to_string()
        };
        let old_value = old.value_at(i).unwrap_or(&Value::Null);
        let (lead, trail) = &parts.segments[i];
        out.push(Member {
            body: format!("{lead}{key}{}", rewrite(text, old_value, child, v)),
            trail: trail.clone(),
        });
    }
    format!("{{{}{}}}", join(&out, parts.comma), parts.tail)
}

fn rewrite_list(
    text: &str,
    node: (usize, usize),
    old: &[Value],
    spans: &Spans,
    next: &[Value],
) -> String {
    let ranges: Vec<(usize, usize)> = spans.children.iter().map(range).collect();
    let parts = members(text, node, &ranges);
    let pad = line_indent(text, ranges[0].0);
    let item = |i: usize, v: &Value| {
        let (lead, trail) = &parts.segments[i];
        Member {
            body: format!("{lead}{}", rewrite(text, &old[i], &spans.children[i], v)),
            trail: trail.clone(),
        }
    };
    let out: Vec<Member> = if next.len() == old.len() {
        next.iter().enumerate().map(|(i, v)| item(i, v)).collect()
    } else {
        // Added or removed items: the others keep their text.
        let mut from = 0;
        next.iter()
            .map(|v| match (from..old.len()).find(|&k| same(&old[k], v)) {
                Some(i) => {
                    from = i + 1;
                    item(i, v)
                }
                None => Member {
                    body: format!("\n{pad}{}", format(v, level_of(pad))),
                    trail: String::new(),
                },
            })
            .collect()
    };
    format!("[{}{}]", join(&out, parts.comma), parts.tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::parse_value;

    fn edit(text: &str, change: impl FnOnce(&mut Map)) -> String {
        let Value::Map(mut v) = parse_value(text).unwrap() else {
            panic!("a map")
        };
        change(&mut v);
        let next = Value::Map(v);
        let out = patch(text, &next);
        assert!(same(&parse_value(&out).unwrap(), &next), "{out}");
        out
    }

    const FILE: &str = r#"// A block.
{
  "id": "lamp", // its id
  /* What it is. */
  "block": {
    "hardness": 4.0,
    "light": 15
  },
  "states": {
    "properties": {
      // On or off.
      "lit": ["false", "true"],
    },
  },
}
"#;

    fn map_at<'a>(m: &'a mut Map, key: &str) -> &'a mut Map {
        match m.get_mut(key) {
            Some(Value::Map(inner)) => inner,
            _ => panic!("no map `{key}`"),
        }
    }

    #[test]
    fn what_didnt_change_keeps_its_text() {
        assert_eq!(edit(FILE, |_| {}), FILE);
        let out = edit(FILE, |m| {
            map_at(m, "block").insert("light", Value::Int(7));
        });
        assert_eq!(out, FILE.replace("\"light\": 15", "\"light\": 7"));
    }

    #[test]
    fn a_new_key_goes_after_the_others() {
        let out = edit(FILE, |m| {
            map_at(m, "block").insert("sound", Value::Str("stone".into()));
        });
        assert!(
            out.contains("\"light\": 15,\n    \"sound\": \"stone\"\n  },"),
            "{out}"
        );
        assert!(out.contains("// its id"));
    }

    #[test]
    fn a_removed_key_takes_its_comments() {
        let out = edit(FILE, |m| {
            m.remove("block");
        });
        assert!(!out.contains("What it is."), "{out}");
        assert!(
            out.contains("\"id\": \"lamp\", // its id") && out.contains("// On or off."),
            "{out}"
        );
        assert_eq!(
            edit("{\n  \"a\": 1, // one\n  \"b\": 2\n}", |m| {
                m.remove("b");
            }),
            "{\n  \"a\": 1 // one\n}"
        );
        assert_eq!(
            edit("{\n  \"a\": 1 // one\n}", |m| {
                m.insert("b", Value::Int(2));
            }),
            "{\n  \"a\": 1, // one\n  \"b\": 2\n}"
        );
    }

    #[test]
    fn a_renamed_key_keeps_its_place_and_comments() {
        let out = edit(FILE, |m| {
            let props = map_at(map_at(m, "states"), "properties");
            let lit = props.remove("lit").unwrap_or(Value::Null);
            props.insert("on", lit);
        });
        assert!(
            out.contains("// On or off.\n      \"on\": [\"false\", \"true\"],"),
            "{out}"
        );
        let text = "{\n  a: 1,\n  b: 2,\n  c: 3\n}";
        let mut next = Map::new();
        for (k, v) in [("a", 1), ("x", 2), ("c", 3)] {
            next.insert(k, Value::Int(v));
        }
        assert_eq!(
            patch(text, &Value::Map(next)),
            "{\n  a: 1,\n  \"x\": 2,\n  c: 3\n}"
        );
    }

    #[test]
    fn list_items_added_and_removed_keep_the_others() {
        let text = "{\n  \"list\": [\n    // first\n    { \"a\": 1 },\n    { \"b\": 2 }\n  ]\n}";
        let list = |m: &mut Map| match m.get_mut("list") {
            Some(Value::List(l)) => std::mem::take(l),
            _ => Vec::new(),
        };
        let out = edit(text, |m| {
            let mut l = list(m);
            l.push(parse_value("{ c: 3 }").unwrap());
            m.insert("list", Value::List(l));
        });
        assert!(out.contains("// first"), "{out}");
        let out = edit(text, |m| {
            let mut l = list(m);
            l.remove(0);
            m.insert("list", Value::List(l));
        });
        assert!(
            !out.contains("first") && out.contains("{ \"b\": 2 }"),
            "{out}"
        );
    }

    #[test]
    fn number_keys_keep_their_order() {
        let text = "{\n  \"rotation\": {\n    \"0\": [0, 0, 0], // rest\n    \"0.5\": [45, 0, 0],\n    \"1\": [0, 0, 0]\n  }\n}";
        let out = edit(text, |m| {
            map_at(m, "rotation").insert("0.5", parse_value("[90, 0, 0]").unwrap());
        });
        assert_eq!(out, text.replace("[45, 0, 0]", "[90, 0, 0]"));
    }

    #[test]
    fn json_keeps_order_and_source_strings() {
        let v =
            parse_value(r#"{ "1": "@item:minecraft:stick", "0.5": [1.5, null, true] }"#).unwrap();
        assert_eq!(
            to_json(&v),
            r#"{"1":"@item:minecraft:stick","0.5":[1.5,null,true]}"#
        );
    }

    #[test]
    fn short_values_go_on_one_line() {
        let v =
            parse_value(r#"{ "uv": [0, 0, 16, 16.5], "faces": { "north": { "texture": "x" } } }"#)
                .unwrap();
        assert_eq!(
            format(&v, 0),
            "{\n  \"uv\": [0, 0, 16, 16.5],\n  \"faces\": {\n    \"north\": { \"texture\": \"x\" }\n  }\n}"
        );
        assert_eq!(patch("{ oops", &Value::Int(1)), "1\n");
    }
}
