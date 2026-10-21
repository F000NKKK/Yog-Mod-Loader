//! `yog serve` — the CLI as a service for tools (Yog-IDLE): frames of
//! binary Yog Data (`yog_data::io`) on stdin and stdout, each request a
//! [`Request`] and each answer a [`Reply`]. Every format is read, written
//! and converted here, so a tool needn't be rebuilt when a format changes —
//! only this binary. Answers hold plain values: a typed one (`@item:x`) is
//! sent as its string.
//!
//! Methods: see [`answer`], [`dispatch`] and the designers' (`design::service`).

mod data;

use std::path::{Path, PathBuf};
use yog_ids::serve_method as M;
use yog_ids::serve_param as P;

use serde_json::{Map, Value, json};
use yog_data::Schema;
use yog_formats::yres::{Entry, Kind};

/// What a tool asks: a method, and its parameters.
#[derive(Debug, Clone, Schema)]
pub struct Request {
    pub method: String,
    #[data(default)]
    pub params: yog_data::Value,
}

/// The answer: the method's result, or what went wrong — a message, or
/// `{message, line}` for a file that doesn't parse.
pub type Reply = Result<yog_data::Value, yog_data::Value>;

/// Serves requests until stdin ends.
pub fn run() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut out = std::io::stdout().lock();
    loop {
        let reply = match yog_data::io::read_frame_bytes(&mut input) {
            Ok(None) => return Ok(()),
            Ok(Some(bytes)) => match yog_data::read_bytes::<Request>(&bytes) {
                Ok(request) => answer(&request),
                Err(e) => Err(format!("not a request: {e:?}").into()),
            },
            Err(e) => return Err(e.to_string()),
        };
        yog_data::io::write_frame(&mut out, &reply).map_err(|e| e.to_string())?;
    }
}

/// `request` answered, in plain values.
pub fn answer(request: &Request) -> Reply {
    let params = &request.params;
    let reply = match data::method(&request.method, params) {
        Some(reply) => reply,
        // The methods still written over JSON values.
        None => {
            let params = yog_data::json::to_json(params);
            dispatch(&request.method, &params)
                .map(|v| yog_data::json::from_json(&v))
                .map_err(|e| yog_data::json::from_json(&e))
        }
    };
    reply
        .map(|v| yog_data::typed::flatten(&v))
        .map_err(|e| yog_data::typed::flatten(&e))
}

/// A request's string parameter.
fn str_param<'a>(params: &'a Value, name: &str) -> Result<&'a str, Value> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| json!(format!("missing string `{name}`")))
}

/// A request's path parameter.
fn path_param(params: &Value, name: &str) -> Result<PathBuf, Value> {
    str_param(params, name).map(PathBuf::from)
}

/// What a method does with its params; an error is a string, or
/// `{message, line}` for a file that doesn't parse.
pub fn dispatch(method: &str, params: &Value) -> Result<Value, Value> {
    if let Some(reply) = media_method(method, params)? {
        return Ok(reply);
    }
    // The designers: drawing, pickers' choices, edits of a mod's files.
    crate::design::service::dispatch(method, params)
        .unwrap_or_else(|| Err(json!(format!("no method `{method}`"))))
}

/// A mod's resources: `.yres` text and the media in it, its pictures, its
/// id; `None`: not one of these.
fn media_method(method: &str, params: &Value) -> Result<Option<Value>, Value> {
    Ok(Some(match method {
        // A file taken into a `.yres` entry: its value (`@bytes:…`) and the type its bytes are.
        M::MEDIA_IMPORT => {
            let file = path_param(params, P::FILE)?;
            let bytes =
                std::fs::read(&file).map_err(|e| json!(format!("{}: {e}", file.display())))?;
            let kind = yog_formats::yres::kind_of(&yog_formats::yres::Value::Bytes(bytes.clone()));
            json!({ "value": yog_data::typed::typed_string(&yog_data::Value::Bytes(bytes)), "type": kind.name() })
        }
        // `.yres` text → its entries; media values as their text form (`@bytes:…`). With the
        // file's `set` and the `mod` id, a text's translation key too.
        M::YRES_PARSE => {
            let set_and_mod = params
                .get(P::SET)
                .and_then(Value::as_str)
                .zip(params.get(P::MOD_ID).and_then(Value::as_str));
            yres_parse(str_param(params, P::TEXT)?, set_and_mod)?
        }
        // Entries (as `yres.parse` gives them) → `.yres` text.
        M::YRES_WRITE => yres_write(
            params
                .get(P::ENTRIES)
                .ok_or_else(|| json!("missing `entries`"))?,
        )?,
        // The mod's pictures: set, name, path, file (no bytes).
        M::MEDIA_PICTURES => {
            let root = mod_root(&path_param(params, P::PATH)?)?;
            let list = crate::media::pictures(&root)
                .into_iter()
                .map(|p| json!({ "set": p.set, "name": p.name, "path": p.path(), "file": p.file }))
                .collect();
            Value::Array(list)
        }
        // The id of the mod a file is in.
        M::MOD_ID => json!(crate::media::mod_id(&mod_root(&path_param(
            params,
            P::PATH
        )?)?)),
        _ => return Ok(None),
    }))
}

/// The mod a file is in (the folder with its manifest, up from it).
fn mod_root(file: &Path) -> Result<PathBuf, Value> {
    crate::design::project::mod_root(file)
        .ok_or_else(|| json!(format!("{} isn't in a mod", file.display())))
}

fn yres_parse(text: &str, set_and_mod: Option<(&str, &str)>) -> Result<Value, Value> {
    let entries = yog_formats::yres::parse(text).map_err(|e| {
        // `line:column: message` → {line, message}.
        let line = e
            .split(':')
            .next()
            .and_then(|l| l.trim().parse::<u64>().ok());
        json!({ "message": e, "line": line })
    })?;
    Ok(json!({ "entries": entries.iter().map(|e| entry_json(e, set_and_mod)).collect::<Vec<_>>() }))
}

/// An entry as a tool takes it: with its accessor, what's wrong with its
/// name, and (a text of a known set and mod) its translation key.
fn entry_json(e: &Entry, set_and_mod: Option<(&str, &str)>) -> Value {
    let mut m = Map::new();
    m.insert("name".into(), json!(e.name));
    m.insert("type".into(), json!(e.kind.name()));
    m.insert(
        "value".into(),
        match &e.value {
            yog_formats::yres::Value::Str(s) => json!(s),
            yog_formats::yres::Value::Num(n) => serde_json::from_str(n).unwrap_or(Value::Null),
            yog_formats::yres::Value::Bool(b) => json!(b),
            yog_formats::yres::Value::Bytes(b) => json!(yog_data::typed::typed_string(
                &yog_data::Value::Bytes(b.clone())
            )),
        },
    );
    m.insert("line".into(), json!(e.line));
    m.insert(
        "accessor".into(),
        json!(crate::yres::accessor_name(&e.name)),
    );
    if let Some(problem) = crate::yres::name_problem(&e.name) {
        m.insert("nameProblem".into(), json!(problem));
    }
    if let Some(problem) = crate::yres::value_problem(e) {
        m.insert("valueProblem".into(), json!(problem));
    }
    if let Some(argb) = crate::yres::color_of(e) {
        m.insert("argb".into(), json!(argb));
    }
    if let (Some((set, mod_id)), Kind::String) = (set_and_mod, e.kind) {
        let key = e
            .key
            .clone()
            .unwrap_or_else(|| crate::yres::default_key(set, mod_id, &e.name));
        m.insert("translationKey".into(), json!(key));
    }
    for (k, v) in [
        ("comment", &e.comment),
        ("key", &e.key),
        ("event", &e.event),
        ("subtitle", &e.subtitle),
    ] {
        if let Some(v) = v {
            m.insert(k.into(), json!(v));
        }
    }
    Value::Object(m)
}

fn yres_write(entries: &Value) -> Result<Value, Value> {
    let list = entries
        .as_array()
        .ok_or_else(|| json!("`entries` is a list"))?;
    let mut out = Vec::with_capacity(list.len());
    for e in list {
        let name = e
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| json!("an entry without `name`"))?;
        let kind = e
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or(Kind::String.name());
        let kind = KINDS
            .iter()
            .copied()
            .find(|k| k.name() == kind)
            .ok_or_else(|| json!(format!("{name}: no type `{kind}`")))?;
        let value = match e.get("value") {
            Some(Value::String(s)) if s.starts_with(yog_ids::sep::TYPED) => {
                match yog_data::typed::classify(s) {
                    Ok(yog_data::Value::Bytes(b)) => yog_formats::yres::Value::Bytes(b),
                    _ => yog_formats::yres::Value::Str(s.clone()),
                }
            }
            Some(Value::String(s)) => yog_formats::yres::Value::Str(s.clone()),
            Some(Value::Bool(b)) => yog_formats::yres::Value::Bool(*b),
            Some(Value::Number(n)) => yog_formats::yres::Value::Num(n.to_string()),
            _ => return Err(json!(format!("{name}: no value"))),
        };
        let field = |k: &str| e.get(k).and_then(Value::as_str).map(str::to_string);
        out.push(Entry {
            name: name.to_string(),
            kind,
            value,
            comment: field("comment"),
            key: field("key"),
            event: field("event"),
            subtitle: field("subtitle"),
            line: 0,
        });
    }
    Ok(json!(yog_formats::yres::write(&out)))
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

#[cfg(test)]
mod tests;
