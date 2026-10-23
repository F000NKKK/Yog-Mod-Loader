//! Text with its comments kept as data — what a file becomes in the binary
//! form, which has no comments: a comment above an object (or above the
//! entry holding it) becomes its `comment`; one above any other entry
//! becomes the map's `$<key>`; one above a list's item that isn't an
//! object joins the list's nearest map's `comment`.

use super::{Spans, SyntaxError, parse};
use crate::model::value::{Map, Value};
use crate::schema::{COMMENT_KEY, META_PREFIX};

/// `text`'s value, its comments in it (see the module docs).
pub fn with_comments(text: &str) -> Result<Value, SyntaxError> {
    let doc = parse(text)?;
    let mut value = doc.value;
    let lead = comments(&text[..doc.spans.span.start as usize]);
    let mut stray = Vec::new();
    attach(&mut value, &doc.spans, text, &mut stray);
    let mut notes: Vec<String> = lead.into_iter().collect();
    notes.extend(stray);
    if let (Value::Map(m), false) = (&mut value, notes.is_empty()) {
        note(m, COMMENT_KEY, &notes.join("\n"));
    }
    Ok(value)
}

/// Moves the comments inside `v` (spanned by `s`) into it; the ones that
/// have no object to go to are added to `stray`.
fn attach(v: &mut Value, s: &Spans, text: &str, stray: &mut Vec<String>) {
    let inner = s.span.start as usize + 1;
    match v {
        Value::Map(m) => {
            let mut before = inner;
            let mut entries: Vec<(String, Value)> = std::mem::take(m).into_iter().collect();
            let mut out = Map::with_capacity(entries.len());
            let mut own = Vec::new();
            for ((key, child), cs) in entries.iter_mut().zip(&s.children) {
                let start = cs.key.unwrap_or(cs.span).start as usize;
                let said = comments(&text[before..start]);
                before = cs.span.end as usize;
                attach(child, cs, text, &mut own);
                match (said, &mut *child) {
                    (Some(c), Value::Map(cm)) => note(cm, COMMENT_KEY, &c),
                    (Some(c), _) => {
                        out.insert(format!("{META_PREFIX}{key}"), Value::Str(c));
                    }
                    (None, _) => {}
                }
                out.insert(key.clone(), std::mem::take(child));
            }
            own.extend(comments(
                &text[before..(s.span.end as usize).saturating_sub(1).max(before)],
            ));
            *m = out;
            if !own.is_empty() {
                note(m, COMMENT_KEY, &own.join("\n"));
            }
        }
        Value::List(items) => {
            let mut before = inner;
            for (item, cs) in items.iter_mut().zip(&s.children) {
                let said = comments(&text[before..cs.span.start as usize]);
                before = cs.span.end as usize;
                attach(item, cs, text, stray);
                match (said, &mut *item) {
                    (Some(c), Value::Map(im)) => note(im, COMMENT_KEY, &c),
                    (Some(c), _) => stray.push(c),
                    (None, _) => {}
                }
            }
        }
        _ => {}
    }
}

/// Adds `text` to `m`'s note `key` (first, before what it said already).
fn note(m: &mut Map, key: &str, text: &str) {
    let joined = match m.get(key).and_then(Value::as_str) {
        Some(had) => format!("{text}\n{had}"),
        None => text.to_string(),
    };
    let rest: Vec<(String, Value)> = std::mem::take(m)
        .into_iter()
        .filter(|(k, _)| k != key)
        .collect();
    m.insert(key, Value::Str(joined));
    for (k, v) in rest {
        m.insert(k, v);
    }
}

/// The comments in `gap` (text between values), their markers and
/// indentation gone, joined by lines; `None` if there are none.
fn comments(gap: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut rest = gap;
    while let Some(at) = rest.find('/') {
        let after = &rest[at + 1..];
        if let Some(line) = after.strip_prefix('/') {
            let end = line.find('\n').unwrap_or(line.len());
            lines.push(line[..end].trim().to_string());
            rest = &line[end..];
        } else if let Some(block) = after.strip_prefix('*') {
            let end = block.find("*/").unwrap_or(block.len());
            lines.extend(
                block[..end]
                    .lines()
                    .map(|l| l.trim().trim_start_matches('*').trim().to_string())
                    .filter(|l| !l.is_empty()),
            );
            rest = block.get(end + 2..).unwrap_or_default();
        } else {
            rest = after;
        }
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

#[cfg(test)]
mod tests;
