//! A mod's resources as the service hands them: `.yres` text and the
//! media in it, its pictures, its id.

use std::path::{Path, PathBuf};

use yog_data::{Schema, Value};
use yog_formats::yres::{Entry, Kind};
use yog_ids::serve_method as M;
use yog_ids::serve_param as P;

use super::Reply;

/// A file taken into a `.yres` entry: its value (`@bytes:…`) and the type its bytes are.
#[derive(Schema)]
struct Imported {
    value: String,
    #[data(rename = "type")]
    kind: String,
}

/// One of the mod's pictures (no bytes).
#[derive(Schema)]
struct PictureInfo {
    set: String,
    name: String,
    path: String,
    file: PathBuf,
}

/// `.yres` text that doesn't read: why, and on which line.
#[derive(Schema)]
struct ParseError {
    message: String,
    line: Option<u64>,
}

/// A `.yres` file's entries.
#[derive(Schema)]
struct Parsed {
    entries: Vec<EntryInfo>,
}

/// An entry as a tool takes it: with its accessor, what's wrong with its
/// name or value, and (a text of a known set and mod) its translation key.
#[derive(Schema)]
#[data(rename_all = "camelCase")]
struct EntryInfo {
    name: String,
    #[data(rename = "type")]
    kind: String,
    value: Value,
    line: u64,
    accessor: String,
    name_problem: Option<String>,
    value_problem: Option<String>,
    argb: Option<u32>,
    translation_key: Option<String>,
    comment: Option<String>,
    key: Option<String>,
    event: Option<String>,
    subtitle: Option<String>,
}

/// Every kind of `.yres` entry.
const KINDS: [Kind; 7] = [
    Kind::String,
    Kind::Image,
    Kind::File,
    Kind::Sound,
    Kind::Color,
    Kind::Number,
    Kind::Bool,
];

/// `method` answered, if it's one of these.
pub(super) fn method(method: &str, params: &Value) -> Option<Reply> {
    Some(match method {
        M::MEDIA_IMPORT => path(params, P::FILE).and_then(|file| {
            let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let kind = yog_formats::yres::kind_of(&yog_formats::yres::Value::Bytes(bytes.clone()));
            let value = yog_data::typed::typed_string(&Value::Bytes(bytes)).unwrap_or_default();
            Ok(Imported {
                value,
                kind: kind.name().into(),
            }
            .write())
        }),
        // `.yres` text → its entries; media values as their text form (`@bytes:…`). With the
        // file's `set` and the `mod` id, a text's translation key too.
        M::YRES_PARSE => text(params, P::TEXT).and_then(|t| {
            let set_and_mod = params
                .get(P::SET)
                .and_then(Value::as_str)
                .zip(params.get(P::MOD_ID).and_then(Value::as_str));
            parse(t, set_and_mod)
        }),
        // Entries (as `yres.parse` gives them) → `.yres` text.
        M::YRES_WRITE => params
            .get(P::ENTRIES)
            .ok_or_else(|| Value::from("missing `entries`"))
            .and_then(write),
        // The mod's pictures: set, name, path, file.
        M::MEDIA_PICTURES => path(params, P::PATH).and_then(|p| {
            let root = mod_root(&p)?;
            let list = crate::media::pictures(&root)
                .into_iter()
                .map(|p| {
                    PictureInfo {
                        path: p.path(),
                        set: p.set,
                        name: p.name,
                        file: p.file,
                    }
                    .write()
                })
                .collect();
            Ok(Value::List(list))
        }),
        // The id of the mod a file is in.
        M::MOD_ID => path(params, P::PATH)
            .and_then(|p| mod_root(&p))
            .map(|root| crate::media::mod_id(&root).write()),
        _ => return None,
    })
}

/// A request's text parameter.
fn text<'a>(params: &'a Value, name: &str) -> Result<&'a str, Value> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string `{name}`").into())
}

/// A request's path parameter.
fn path(params: &Value, name: &str) -> Result<PathBuf, Value> {
    text(params, name).map(PathBuf::from)
}

/// The mod a file is in (the folder with its manifest, up from it).
fn mod_root(file: &Path) -> Result<PathBuf, Value> {
    crate::design::project::mod_root(file)
        .ok_or_else(|| format!("{} isn't in a mod", file.display()).into())
}

fn parse(text: &str, set_and_mod: Option<(&str, &str)>) -> Reply {
    let entries = yog_formats::yres::parse(text).map_err(|e| {
        // `line:column: message` → {line, message}.
        let line = e
            .split(':')
            .next()
            .and_then(|l| l.trim().parse::<u64>().ok());
        ParseError { message: e, line }.write()
    })?;
    let entries = entries.iter().map(|e| info(e, set_and_mod)).collect();
    Ok(Parsed { entries }.write())
}

fn info(e: &Entry, set_and_mod: Option<(&str, &str)>) -> EntryInfo {
    let value = match &e.value {
        yog_formats::yres::Value::Str(s) => Value::Str(s.clone()),
        yog_formats::yres::Value::Num(n) => yog_data::text::parse_value(n).unwrap_or(Value::Null),
        yog_formats::yres::Value::Bool(b) => Value::Bool(*b),
        yog_formats::yres::Value::Bytes(b) => {
            Value::Str(yog_data::typed::typed_string(&Value::Bytes(b.clone())).unwrap_or_default())
        }
    };
    let translation_key = match (set_and_mod, e.kind) {
        (Some((set, mod_id)), Kind::String) => Some(
            e.key
                .clone()
                .unwrap_or_else(|| crate::yres::default_key(set, mod_id, &e.name)),
        ),
        _ => None,
    };
    EntryInfo {
        name: e.name.clone(),
        kind: e.kind.name().into(),
        value,
        line: e.line as u64,
        accessor: crate::yres::accessor_name(&e.name),
        name_problem: crate::yres::name_problem(&e.name),
        value_problem: crate::yres::value_problem(e),
        argb: crate::yres::color_of(e),
        translation_key,
        comment: e.comment.clone(),
        key: e.key.clone(),
        event: e.event.clone(),
        subtitle: e.subtitle.clone(),
    }
}

fn write(entries: &Value) -> Reply {
    let list = entries
        .as_list()
        .ok_or_else(|| Value::from("`entries` is a list"))?;
    let mut out = Vec::with_capacity(list.len());
    for e in list {
        out.push(entry(e)?);
    }
    Ok(yog_formats::yres::write(&out).into())
}

/// An entry as `yres.parse` gave it (and the editor changed it).
fn entry(e: &Value) -> Result<Entry, Value> {
    let field = |k: &str| e.get(k).and_then(Value::as_str).map(str::to_string);
    let name = field("name").ok_or_else(|| Value::from("an entry without `name`"))?;
    let kind = field("type").unwrap_or_else(|| Kind::String.name().into());
    let kind = KINDS
        .iter()
        .copied()
        .find(|k| k.name() == kind)
        .ok_or_else(|| Value::from(format!("{name}: no type `{kind}`")))?;
    let value = match e.get("value") {
        Some(Value::Str(s)) if s.starts_with(yog_ids::sep::TYPED) => {
            match yog_data::typed::classify(s) {
                Ok(Value::Bytes(b)) => yog_formats::yres::Value::Bytes(b),
                _ => yog_formats::yres::Value::Str(s.clone()),
            }
        }
        Some(Value::Str(s)) => yog_formats::yres::Value::Str(s.clone()),
        Some(Value::Bool(b)) => yog_formats::yres::Value::Bool(*b),
        Some(n @ (Value::Int(_) | Value::Float(_))) => {
            yog_formats::yres::Value::Num(yog_data::text::to_string(n))
        }
        _ => return Err(format!("{name}: no value").into()),
    };
    Ok(Entry {
        name,
        kind,
        value,
        comment: field("comment"),
        key: field("key"),
        event: field("event"),
        subtitle: field("subtitle"),
        line: 0,
    })
}
