//! The typed values' string forms — one grammar for every format:
//!
//! | Written | Value |
//! |---|---|
//! | `@item:minecraft:stick`, `@block:minecraft:furnace[facing=north]` | [`Value::Ref`] |
//! | `@tag:item:minecraft:planks` | [`Value::Tag`] |
//! | `@Strings.screen.title` (set names start upper case) | [`Value::Res`] |
//! | `@image:sprite:button`, `@image:mymod:gui/logo` | [`Value::Image`] |
//! | `@lang:gui.done` (a key as in the lang files) | [`Value::Lang`] |
//! | `#RRGGBB`, `#AARRGGBB` | [`Value::Color`] |
//! | `@@text` | the plain string `@text` |
//!
//! A string that starts with `@` is always one of these (or an error), in
//! any field — so a typo in a reference is never silently plain text.

use crate::model::value::{Ref, Res, Value};

use base64::Engine as _;
use yog_ids::typed;

/// `s` as a typed value if it's written as one (`@…`), the plain string
/// otherwise; `@@` escapes a leading `@`.
pub fn classify(s: &str) -> Result<Value, String> {
    let Some(rest) = s.strip_prefix('@') else {
        return Ok(Value::Str(s.to_string()));
    };
    if let Some(literal) = rest.strip_prefix('@') {
        return Ok(Value::Str(format!("@{literal}")));
    }
    let head_end = rest.find([':', '.']).ok_or_else(|| format!("`{s}`: a reference is `@kind:id` (`@item:minecraft:stick`) or `@Set.key`; write `@@` for a literal `@`"))?;
    let head = &rest[..head_end];
    let tail = &rest[head_end + 1..];
    if rest.as_bytes()[head_end] == b'.' {
        if !is_ident(head) || !head.starts_with(|c: char| c.is_ascii_uppercase()) {
            return Err(format!(
                "`{s}`: a resource is `@Set.key` with an upper-case set name (`@Strings.title`)"
            ));
        }
        if !is_res_key(tail) {
            return Err(format!(
                "`{s}`: `{tail}` isn't a resource key (letters, digits, `_`, dotted)"
            ));
        }
        return Ok(Value::Res(Res {
            set: head.to_string(),
            key: tail.to_string(),
        }));
    }
    if !is_kind(head) {
        return Err(format!(
            "`{s}`: `{head}` isn't a kind of content (lower case: `item`, `block`, `tag`, ...)"
        ));
    }
    match head {
        typed::TAG => {
            let (kind, id) = tail.split_once(':').ok_or_else(|| {
                format!("`{s}`: a tag is `@tag:kind:id` (`@tag:item:minecraft:planks`)")
            })?;
            if !is_kind(kind) {
                return Err(format!("`{s}`: `{kind}` isn't a kind of content"));
            }
            Ok(Value::Tag(Ref {
                kind: kind.to_string(),
                id: resource_id(s, id)?,
            }))
        }
        typed::LANG => {
            if tail.is_empty()
                || !tail.bytes().all(|b| {
                    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b':' | b'/')
                })
            {
                return Err(format!(
                    "`{s}`: `@lang:` takes a translation key as the lang files have it (`@lang:gui.done`)"
                ));
            }
            Ok(Value::Lang(tail.to_string()))
        }
        typed::BYTES => base64::engine::general_purpose::STANDARD
            .decode(tail)
            .map(Value::Bytes)
            .map_err(|e| format!("`@bytes:`: not base64 ({e})")),
        typed::IMAGE => {
            if tail.is_empty() {
                return Err(format!("`{s}`: no image source after `@image:`"));
            }
            Ok(Value::Image(tail.to_string()))
        }
        _ => Ok(Value::Ref(Ref {
            kind: head.to_string(),
            id: resource_id(s, tail)?,
        })),
    }
}

/// The string form of a typed value (`None` for the untyped ones).
pub fn typed_string(v: &Value) -> Option<String> {
    Some(match v {
        Value::Ref(r) => format!("@{}:{}", r.kind, r.id),
        Value::Tag(r) => format!("@tag:{}:{}", r.kind, r.id),
        Value::Res(r) => r.to_string(),
        Value::Image(src) => format!("@image:{src}"),
        Value::Lang(key) => format!("@lang:{key}"),
        Value::Color(c) => format_color(*c),
        Value::Bytes(b) => format!(
            "@{}:{}",
            typed::BYTES,
            base64::engine::general_purpose::STANDARD.encode(b)
        ),
        Value::Text(t) => escape_at(t),
        Value::Str(t) => escape_at(t),
        _ => return None,
    })
}

/// `v` with its typed values as their strings (`@item:x`): plain values
/// only, for a tool that knows no typed ones (the IDE, through `yog serve`).
pub fn flatten(v: &Value) -> Value {
    match v {
        Value::List(items) => Value::List(items.iter().map(flatten).collect()),
        Value::Map(m) => Value::Map(m.iter().map(|(k, x)| (k.to_string(), flatten(x))).collect()),
        Value::Str(_) => v.clone(),
        typed => typed_string(typed).map_or_else(|| typed.clone(), Value::Str),
    }
}

/// `s` with a leading `@` doubled, so it reads back as plain text.
pub fn escape_at(s: &str) -> String {
    if s.starts_with('@') {
        format!("@{s}")
    } else {
        s.to_string()
    }
}

/// A content id: `namespace:path` (namespace defaults to `minecraft`),
/// lower case, with an optional `[state]` / `{nbt}` suffix kept as written.
pub fn resource_id(whole: &str, id: &str) -> Result<String, String> {
    let base_end = id.find(['[', '{']).unwrap_or(id.len());
    let (base, suffix) = id.split_at(base_end);
    let (ns, path) = yog_ids::split(base);
    let ok_ns = !ns.is_empty()
        && ns
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'));
    let ok_path = !path.is_empty()
        && path
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'/'));
    if !ok_ns || !ok_path {
        return Err(format!(
            "`{whole}`: `{base}` isn't an id (lower case `namespace:path`)"
        ));
    }
    if !suffix.is_empty() && !(suffix.ends_with(']') || suffix.ends_with('}')) {
        return Err(format!("`{whole}`: unclosed `{}`", &suffix[..1]));
    }
    Ok(format!("{ns}:{path}{suffix}"))
}

/// A colour's digits: `RRGGBB` and `AARRGGBB`, hexadecimal; full alpha.
const RGB_DIGITS: usize = 6;
const ARGB_DIGITS: usize = 8;
const HEX_RADIX: u32 = 16;
const OPAQUE: u32 = 0xFF00_0000;

/// `#RRGGBB` (opaque) or `#AARRGGBB` → `0xAARRGGBB`.
pub fn parse_color(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        RGB_DIGITS => u32::from_str_radix(hex, HEX_RADIX).ok().map(|c| OPAQUE | c),
        ARGB_DIGITS => u32::from_str_radix(hex, HEX_RADIX).ok(),
        _ => None,
    }
}

/// `0xAARRGGBB` → `#RRGGBB` when opaque, else `#AARRGGBB`.
pub fn format_color(c: u32) -> String {
    if c >> 24 == 0xFF {
        format!("#{:06X}", c & 0xFF_FFFF)
    } else {
        format!("#{c:08X}")
    }
}

/// A resource key: identifiers joined by `.` (`screen.title`); after the
/// first, a part may start with a digit (`item.robes.2_boots`).
pub fn is_res_key(key: &str) -> bool {
    let mut parts = key.split('.');
    parts.next().is_some_and(is_ident)
        && parts.all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_kind(s: &str) -> bool {
    let mut b = s.bytes();
    b.next().is_some_and(|c| c.is_ascii_lowercase())
        && b.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_form() {
        let r = |k: &str, id: &str| Ref {
            kind: k.into(),
            id: id.into(),
        };
        assert_eq!(classify("plain").unwrap(), Value::Str("plain".into()));
        assert_eq!(classify("@@me").unwrap(), Value::Str("@me".into()));
        assert_eq!(
            classify("@item:minecraft:stick").unwrap(),
            Value::Ref(r("item", "minecraft:stick"))
        );
        assert_eq!(
            classify("@item:stick").unwrap(),
            Value::Ref(r("item", "minecraft:stick"))
        );
        assert_eq!(
            classify("@block:furnace[facing=north]").unwrap(),
            Value::Ref(r("block", "minecraft:furnace[facing=north]"))
        );
        assert_eq!(
            classify("@tag:item:minecraft:planks").unwrap(),
            Value::Tag(r("item", "minecraft:planks"))
        );
        // A mod's own kind of content.
        assert_eq!(
            classify("@pattern:hexmod:get_caster").unwrap(),
            Value::Ref(r("pattern", "hexmod:get_caster"))
        );
    }

    #[test]
    fn resources_and_images() {
        assert_eq!(
            classify("@Strings.screen.title").unwrap(),
            Value::Res(Res {
                set: "Strings".into(),
                key: "screen.title".into()
            })
        );
        assert_eq!(
            classify("@image:sprite:button").unwrap(),
            Value::Image("sprite:button".into())
        );
    }

    #[test]
    fn rejects_typos() {
        for bad in [
            "@",
            "@stick",
            "@Item:stick",
            "@item:Stick",
            "@item:",
            "@tag:item",
            "@strings.title",
            "@Strings.",
            "@block:furnace[facing=north",
            "@image:",
        ] {
            assert!(classify(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn round_trips() {
        for s in [
            "@item:minecraft:stick",
            "@tag:block:minecraft:logs",
            "@Strings.a.b",
            "@image:sprite:x",
            "@@at",
            "plain",
        ] {
            assert_eq!(typed_string(&classify(s).unwrap()).unwrap(), s);
        }
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#8d6acc"), Some(0xFF8D6ACC));
        assert_eq!(parse_color("#808d6acc"), Some(0x808D6ACC));
        assert_eq!(parse_color("#fff"), None);
        assert_eq!(format_color(0xFF8D6ACC), "#8D6ACC");
        assert_eq!(format_color(0x808D6ACC), "#808D6ACC");
    }
}
