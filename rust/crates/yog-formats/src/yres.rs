//! `.yres` — a mod's texts, images, sounds, videos, files, colors, numbers, flags.

use yog_data::Schema;
use yog_ids::res_kind;

/// An entry's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
pub enum Kind {
    String,
    Image,
    File,
    Sound,
    Color,
    Number,
    Bool,
    Video,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::String => res_kind::STRING,
            Kind::Image => res_kind::IMAGE,
            Kind::File => res_kind::FILE,
            Kind::Sound => res_kind::SOUND,
            Kind::Color => res_kind::COLOR,
            Kind::Number => res_kind::NUMBER,
            Kind::Bool => res_kind::BOOL,
            Kind::Video => res_kind::VIDEO,
        }
    }

    /// Whether its value is bytes (`@bytes:…`): a picture, a file, a sound, a video.
    pub fn holds_bytes(self) -> bool {
        matches!(self, Kind::Image | Kind::File | Kind::Sound | Kind::Video)
    }
}

/// An entry's object form: `{ "type": ..., "value": ..., "comment": ..., ... }`.
#[derive(Schema)]
pub struct EntryObject {
    /// What the entry is; by default what its value is (string, number, bool).
    #[data(rename = "type")]
    pub kind: Option<Kind>,
    /// The text, path, color, number or flag.
    pub value: yog_data::Value,
    /// A note for whoever edits the file — never shipped.
    pub comment: Option<String>,
    /// Texts: the translation key instead of the default one.
    pub key: Option<String>,
}

/// `resources/.../Strings.ru_ru.yres` → `("Strings", Some("ru_ru"))`.
pub fn split_file_name(file_name: &str) -> Option<(&str, Option<&str>)> {
    let stem = file_name
        .strip_suffix(yog_ids::Format::Res.ext())?
        .strip_suffix('.')?;
    Some(match stem.split_once('.') {
        Some((set, locale)) => (set, Some(locale)),
        None => (stem, None),
    })
}

/// Where an image entry's picture goes, under `assets/<mod id>/textures/`:
/// its file's path under `resources/` (`rel`) without `.png`. `None` for a
/// file that isn't a `.png`. (Textures the game's atlases are made of are
/// `.ytexture` files, not `.yres` images.)
#[must_use]
pub fn image_texture(rel: &str) -> Option<String> {
    rel.strip_suffix(yog_ids::res::PNG).map(str::to_string)
}

// ── Entries ───────────────────────────────────────────────────────────────────

use std::collections::HashMap;

use yog_data::{Reader, Seg, Shape, ValueRef};

/// The type of an entry that doesn't say, by its value — bytes by what they start with.
pub fn kind_of(value: &Value) -> Kind {
    match value {
        Value::Str(_) => Kind::String,
        Value::Num(_) => Kind::Number,
        Value::Bool(_) => Kind::Bool,
        Value::Bytes(b) if b.starts_with(OGG_MAGIC) => Kind::Sound,
        Value::Bytes(b) if PICTURE_MAGIC.iter().any(|m| b.starts_with(m)) => Kind::Image,
        Value::Bytes(_) => Kind::File,
    }
}

/// What an Ogg stream starts with — the game's sound format.
pub const OGG_MAGIC: &[u8] = b"OggS";
/// What the pictures an entry can hold start with: PNG, JPEG, GIF, WebP (RIFF), BMP.
const PICTURE_MAGIC: &[&[u8]] = &[b"\x89PNG", b"\xFF\xD8\xFF", b"GIF8", b"RIFF", b"BM"];

/// A scalar as written: a string, a number (its literal), a bool, or bytes
/// (a picture, a sound, a file — `@bytes:<base64>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Num(String),
    Bool(bool),
    Bytes(Vec<u8>),
}

/// One entry of a `.yres` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    pub value: Value,
    pub comment: Option<String>,
    pub key: Option<String>,
    /// 1-based line of the name.
    pub line: usize,
}

impl Entry {
    /// What the entry holds when it's text (empty otherwise).
    pub fn text(&self) -> &str {
        match &self.value {
            Value::Str(s) => s,
            _ => "",
        }
    }

    /// An image entry named `name` holding the picture `bytes`.
    pub fn picture(name: &str, bytes: Vec<u8>) -> Entry {
        Entry {
            name: name.to_string(),
            kind: Kind::Image,
            value: Value::Bytes(bytes),
            comment: None,
            key: None,
            line: 0,
        }
    }
}

/// What bytes are written as in text: `@bytes:<base64>`.
const BYTES_PREFIX: &str = yog_ids::concatcp!(
    yog_ids::sep::TYPED,
    yog_ids::typed::BYTES,
    yog_ids::sep::NAMESPACE
);

/// A whole `.yres` file: named entries in file order.
pub struct YresFile(Vec<Entry>);

/// `v` as an entry's scalar value.
fn scalar(v: &ValueRef, cx: &mut Reader) -> Option<Value> {
    Some(match v {
        ValueRef::Bytes(b) => Value::Bytes(b.to_vec()),
        ValueRef::Str(s) if s.starts_with(BYTES_PREFIX) => match yog_data::typed::classify(s) {
            Ok(yog_data::Value::Bytes(b)) => Value::Bytes(b),
            Ok(other) => {
                cx.problem(format!("`{s}`: bytes, not {other:?}"));
                return None;
            }
            Err(e) => {
                cx.problem(e);
                return None;
            }
        },
        ValueRef::Str(_) => Value::Str(String::read(v, cx)?),
        ValueRef::Int(n) => Value::Num(n.to_string()),
        ValueRef::Float(f) => Value::Num(format!("{f:?}")),
        ValueRef::Bool(b) => Value::Bool(*b),
        other => {
            cx.expected("a string, a number or true/false", other);
            return None;
        }
    })
}

fn read_entry(name: &str, v: &ValueRef, cx: &mut Reader) -> Option<Entry> {
    let mut entry = Entry {
        name: name.to_string(),
        kind: Kind::String,
        value: Value::Str(String::new()),
        comment: None,
        key: None,
        line: 0,
    };
    if !matches!(v, ValueRef::Map(_)) {
        entry.value = scalar(v, cx)?;
        entry.kind = kind_of(&entry.value);
        return Some(entry);
    }
    let o = EntryObject::read(v, cx);
    // The value is checked even when another field is wrong: every problem at once.
    let value = v
        .get("value")
        .and_then(|val| cx.at(Seg::Key("value".into()), |cx| scalar(val, cx)));
    let (o, value) = (o?, value?);
    let kind = o.kind.unwrap_or(kind_of(&value));
    let fits = match (kind, &value) {
        (k, Value::Bytes(_)) => k.holds_bytes(),
        (Kind::Number, Value::Num(_))
        | (Kind::Bool, Value::Bool(_))
        | (Kind::String | Kind::Color, Value::Str(_)) => true,
        _ => false,
    };
    if !fits {
        let needs = match kind {
            Kind::Number => "number",
            Kind::Bool => "true/false",
            k if k.holds_bytes() => "bytes (`@bytes:…`)",
            _ => "string",
        };
        cx.at(Seg::Key("value".into()), |cx| {
            cx.problem(format!("a {} needs a {needs} value", kind.name()))
        });
        return None;
    }
    let misplaced = [("key", o.key.is_some(), Kind::String)];
    for (field, present, only) in misplaced {
        if present && kind != only {
            cx.at(Seg::Key(field.into()), |cx| {
                cx.problem(format!("only {} entries have a `{field}`", only.name()))
            });
            return None;
        }
    }
    entry.kind = kind;
    entry.value = value;
    (entry.comment, entry.key) = (o.comment, o.key);
    Some(entry)
}

impl Schema for YresFile {
    fn shape() -> Shape {
        Shape::Map(EntryObject::shape)
    }

    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let ValueRef::Map(map) = v else {
            cx.problem("expected `{` — a .yres file is one map of named resources");
            return None;
        };
        let mut entries = Vec::with_capacity(map.len());
        let mut ok = true;
        // `$…` keys are the file's metadata (`$schema`); every other key names a resource.
        for (name, v) in map.iter().filter(|(k, _)| !k.starts_with('$')) {
            match cx.at(Seg::Key(name.to_string()), |cx| read_entry(name, v, cx)) {
                Some(e) => entries.push(e),
                None => ok = false,
            }
        }
        ok.then_some(YresFile(entries))
    }

    fn write(&self) -> yog_data::Value {
        let mut map = yog_data::Map::new();
        for e in &self.0 {
            let value = match &e.value {
                Value::Str(s) => yog_data::Value::Str(s.clone()),
                Value::Num(n) => n
                    .parse::<i64>()
                    .map(yog_data::Value::Int)
                    .unwrap_or_else(|_| yog_data::Value::Float(n.parse().unwrap_or_default())),
                Value::Bool(b) => yog_data::Value::Bool(*b),
                Value::Bytes(b) => yog_data::Value::Bytes(b.clone()),
            };
            let plain = e.kind == kind_of(&e.value) && e.comment.is_none() && e.key.is_none();
            if plain {
                map.insert(e.name.clone(), value);
                continue;
            }
            let o = EntryObject {
                kind: (e.kind != kind_of(&e.value)).then_some(e.kind),
                value,
                comment: e.comment.clone(),
                key: e.key.clone(),
            };
            map.insert(e.name.clone(), o.write());
        }
        yog_data::Value::Map(map)
    }
}

/// Parses `.yres` text: its entries, or every problem, each with its code
/// and place; names aren't validated here (see [`valid_name`]).
pub fn parse(text: &str) -> Result<Vec<Entry>, yog_data::Errors> {
    let doc = yog_data::text::parse(text).map_err(yog_data::Errors::from)?;
    let mut entries = yog_data::schema::from_doc::<YresFile>(text, &doc)?.value.0;
    // Each name's line: the spans are in the same order as the names.
    let mut lines = HashMap::new();
    if let yog_data::Value::Map(map) = &doc.value {
        let mut line = 1;
        let mut at = 0;
        for ((name, _), spans) in map.iter().zip(&doc.spans.children) {
            let start = spans.key.unwrap_or(spans.span).start as usize;
            line += text.as_bytes()[at..start]
                .iter()
                .filter(|&&b| b == b'\n')
                .count();
            at = start;
            lines.insert(name, line);
        }
    }
    for e in &mut entries {
        e.line = lines.get(e.name.as_str()).copied().unwrap_or(0);
    }
    Ok(entries)
}

/// `.yres` text for `entries`, as the IDE writes it.
pub fn write(entries: &[Entry]) -> String {
    yog_data::text::to_string_pretty(&YresFile(entries.to_vec()).write())
}
