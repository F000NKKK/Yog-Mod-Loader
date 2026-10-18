//! `yog serve` — the CLI as a service for tools (Yog-IDLE): one JSON
//! request per line on stdin, one reply per line on stdout. Every format is
//! read, written and converted here, so a tool needn't be rebuilt when a
//! format changes — only this binary.
//!
//! ```text
//! → {"id": 1, "method": "yres.parse", "params": {"text": "{…}"}}
//! ← {"id": 1, "result": {"entries": […]}}
//! ← {"id": 2, "error": "…"}
//! ```
//!
//! Methods: see [`dispatch`] and the designers' (`design::service`).

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{Map, Value, json};
use yog_formats::yres::{Entry, Kind};

/// Serves requests until stdin ends.
pub fn run() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(req) => {
                let id = req.get("id").cloned().unwrap_or(Value::Null);
                let method = req.get("method").and_then(Value::as_str).unwrap_or_default();
                let params = req.get("params").cloned().unwrap_or(Value::Null);
                match dispatch(method, &params) {
                    Ok(result) => json!({ "id": id, "result": result }),
                    Err(error) => json!({ "id": id, "error": error }),
                }
            }
            Err(e) => json!({ "id": null, "error": format!("not a request: {e}") }),
        };
        writeln!(out, "{reply}").map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A request's string parameter.
fn str_param<'a>(params: &'a Value, name: &str) -> Result<&'a str, Value> {
    params.get(name).and_then(Value::as_str).ok_or_else(|| json!(format!("missing string `{name}`")))
}

/// A request's path parameter.
fn path_param(params: &Value, name: &str) -> Result<PathBuf, Value> {
    str_param(params, name).map(PathBuf::from)
}

/// A request's value parameter, given as JSON text (so its maps keep their order).
fn data_param(params: &Value, name: &str) -> Result<yog_data::Value, Value> {
    yog_data::text::parse_value(str_param(params, name)?).map_err(|e| json!(format!("`{name}`: {e}")))
}

/// Bytes as a reply carries them.
fn base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// What a method does with its params; an error is a string, or
/// `{message, line}` for a file that doesn't parse.
pub fn dispatch(method: &str, params: &Value) -> Result<Value, Value> {
    match method {
        // A file taken into a `.yres` entry: its value (`@bytes:…`) and the type its bytes are.
        "media.import" => {
            let file = path_param(params, "file")?;
            let bytes = std::fs::read(&file).map_err(|e| json!(format!("{}: {e}", file.display())))?;
            let kind = yog_formats::yres::kind_of(&yog_formats::yres::Value::Bytes(bytes.clone()));
            Ok(json!({ "value": yog_data::typed::typed_string(&yog_data::Value::Bytes(bytes)), "type": kind.name() }))
        }
        // The service's version: a tool restarts it when the binary changes.
        "version" => Ok(json!(env!("CARGO_PKG_VERSION"))),
        // `.yres` text → its entries; media values as their text form (`@bytes:…`). With the
        // file's `set` and the `mod` id, a text's translation key too.
        "yres.parse" => yres_parse(str_param(params, "text")?, params.get("set").and_then(Value::as_str).zip(params.get("mod").and_then(Value::as_str))),
        // Entries (as `yres.parse` gives them) → `.yres` text.
        "yres.write" => yres_write(params.get("entries").ok_or_else(|| json!("missing `entries`"))?),
        // Yog Data text → its value as JSON text (maps in file order, typed values as written).
        "data.parse" => {
            let value = yog_data::text::parse_value(str_param(params, "text")?)
                .map_err(|e| json!({ "message": e.to_string(), "line": e.at.line, "col": e.at.col }))?;
            Ok(json!(yog_data::patch::to_json(&value)))
        }
        // `text` changed to hold `next` (JSON text), comments and layout of the rest kept.
        "data.patch" => Ok(json!(yog_data::patch::patch(str_param(params, "text")?, &data_param(params, "next")?))),
        // A value (JSON text) as Yog's editors write it, at indent `level`.
        "data.format" => {
            let level = params.get("level").and_then(Value::as_u64).unwrap_or(0) as usize;
            Ok(json!(yog_data::patch::format(&data_param(params, "value")?, level)))
        }
        // A format's shape (`yog_data::Shape::describe`): `yblock`, `.ytexture`, `yog.toml`…
        "format.shape" => {
            let name = str_param(params, "format")?;
            let shape = yog_formats::shape_of(name).ok_or_else(|| json!(format!("no format `{name}`")))?;
            Ok(yog_data::json::to_json(&shape.describe()))
        }
        // The id of the mod a file is in.
        "mod.id" => Ok(json!(crate::media::mod_id(&mod_root(&path_param(params, "path")?)?))),
        // The file the build makes at `assets/<ns>/<path>` (base64), or null.
        "media.built_asset" => {
            let root = mod_root(&path_param(params, "path")?)?;
            Ok(json!(crate::media::built_asset(&root, str_param(params, "ns")?, str_param(params, "asset")?).map(|b| base64(&b))))
        }
        // The mod's textures: id, `.ytexture`, picture, atlas.
        "media.textures" => {
            let root = mod_root(&path_param(params, "path")?)?;
            let list: Vec<Value> = crate::media::textures(&root)
                .into_iter()
                .map(|t| json!({ "id": t.id, "file": t.file, "image": format!("{}{}.{}", yog_ids::sep::TYPED, t.texture.image.set, t.texture.image.key), "atlas": t.texture.atlas }))
                .collect();
            Ok(Value::Array(list))
        }
        // Whether texture `id` is the mod's own (it can be painted in place, or added).
        "media.is_own_texture" => {
            let root = mod_root(&path_param(params, "path")?)?;
            Ok(json!(crate::media::is_own_texture(&root, str_param(params, "id")?)))
        }
        // The mod's pictures: set, name, path, file (no bytes).
        "media.pictures" => {
            let root = mod_root(&path_param(params, "path")?)?;
            let list: Vec<Value> = crate::media::pictures(&root)
                .into_iter()
                .map(|p| json!({ "set": p.set, "name": p.name, "path": p.path(), "file": p.file }))
                .collect();
            Ok(Value::Array(list))
        }
        // Paints texture `id`: `png` (base64) as its picture; the file written.
        "media.save_texture_picture" => {
            let root = mod_root(&path_param(params, "path")?)?;
            let png = base64::engine::general_purpose::STANDARD.decode(str_param(params, "png")?).map_err(|e| json!(e.to_string()))?;
            crate::media::save_texture_picture(&root, str_param(params, "id")?, png).map(|f| json!(f)).map_err(Value::from)
        }
        // Sets texture `id`'s animation (the game's `.mcmeta` JSON, or null).
        "media.save_texture_animation" => {
            let root = mod_root(&path_param(params, "path")?)?;
            let animation = params.get("mcmeta").filter(|m| !m.is_null()).and_then(yog_formats::texture::Animation::from_mcmeta);
            crate::media::save_texture_animation(&root, str_param(params, "id")?, animation).map(|f| json!(f)).map_err(Value::from)
        }
        // The designers: drawing, pickers' choices, edits of a mod's files.
        other => crate::design::service::dispatch(other, params).unwrap_or_else(|| Err(json!(format!("no method `{other}`")))),
    }
}

/// The mod a file is in (the folder with its manifest, up from it).
fn mod_root(file: &Path) -> Result<PathBuf, Value> {
    crate::design::project::mod_root(file).ok_or_else(|| json!(format!("{} isn't in a mod", file.display())))
}

fn yres_parse(text: &str, set_and_mod: Option<(&str, &str)>) -> Result<Value, Value> {
    let entries = yog_formats::yres::parse(text).map_err(|e| {
        // `line:column: message` → {line, message}.
        let line = e.split(':').next().and_then(|l| l.trim().parse::<u64>().ok());
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
    m.insert("value".into(), match &e.value {
        yog_formats::yres::Value::Str(s) => json!(s),
        yog_formats::yres::Value::Num(n) => serde_json::from_str(n).unwrap_or(Value::Null),
        yog_formats::yres::Value::Bool(b) => json!(b),
        yog_formats::yres::Value::Bytes(b) => json!(yog_data::typed::typed_string(&yog_data::Value::Bytes(b.clone()))),
    });
    m.insert("line".into(), json!(e.line));
    m.insert("accessor".into(), json!(crate::yres::accessor_name(&e.name)));
    if let Some(problem) = crate::yres::name_problem(&e.name) {
        m.insert("nameProblem".into(), json!(problem));
    }
    if let (Some((set, mod_id)), Kind::String) = (set_and_mod, e.kind) {
        let key = e.key.clone().unwrap_or_else(|| crate::yres::default_key(set, mod_id, &e.name));
        m.insert("translationKey".into(), json!(key));
    }
    for (k, v) in [("comment", &e.comment), ("key", &e.key), ("event", &e.event), ("subtitle", &e.subtitle)] {
        if let Some(v) = v {
            m.insert(k.into(), json!(v));
        }
    }
    Value::Object(m)
}

fn yres_write(entries: &Value) -> Result<Value, Value> {
    let list = entries.as_array().ok_or_else(|| json!("`entries` is a list"))?;
    let mut out = Vec::with_capacity(list.len());
    for e in list {
        let name = e.get("name").and_then(Value::as_str).ok_or_else(|| json!("an entry without `name`"))?;
        let kind = e.get("type").and_then(Value::as_str).unwrap_or(Kind::String.name());
        let kind = KINDS.iter().copied().find(|k| k.name() == kind).ok_or_else(|| json!(format!("{name}: no type `{kind}`")))?;
        let value = match e.get("value") {
            Some(Value::String(s)) if s.starts_with(yog_ids::sep::TYPED) => match yog_data::typed::classify(s) {
                Ok(yog_data::Value::Bytes(b)) => yog_formats::yres::Value::Bytes(b),
                _ => yog_formats::yres::Value::Str(s.clone()),
            },
            Some(Value::String(s)) => yog_formats::yres::Value::Str(s.clone()),
            Some(Value::Bool(b)) => yog_formats::yres::Value::Bool(*b),
            Some(Value::Number(n)) => yog_formats::yres::Value::Num(n.to_string()),
            _ => return Err(json!(format!("{name}: no value"))),
        };
        let field = |k: &str| e.get(k).and_then(Value::as_str).map(str::to_string);
        out.push(Entry { name: name.to_string(), kind, value, comment: field("comment"), key: field("key"), event: field("event"), subtitle: field("subtitle"), line: 0 });
    }
    Ok(json!(yog_formats::yres::write(&out)))
}

/// Every kind of `.yres` entry.
const KINDS: [Kind; 7] = [Kind::String, Kind::Image, Kind::File, Kind::Sound, Kind::Color, Kind::Number, Kind::Bool];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yres_round_trips_through_the_service() {
        let text = r##"{ "title": "Hi", "n": 3, "logo": "@bytes:iVBORw0KGgo=", "c": { "type": "color", "value": "#FF0000", "comment": "red" } }"##;
        let parsed = dispatch("yres.parse", &json!({ "text": text })).unwrap();
        let entries = &parsed["entries"];
        assert_eq!(entries[2]["type"], "image");
        assert_eq!(entries[2]["value"], "@bytes:iVBORw0KGgo=");
        let named = dispatch("yres.parse", &json!({ "text": "{ \"cad.0_x\": \"a\", \"1a\": \"b\" }", "set": "Items", "mod": "m" })).unwrap();
        assert_eq!(named["entries"][0]["accessor"], "cad_0_x");
        assert!(named["entries"][0].get("nameProblem").is_none());
        assert!(named["entries"][1]["nameProblem"].is_string());
        assert_eq!(named["entries"][0]["translationKey"], "item.m.cad.0_x");
        let written = dispatch("yres.write", &json!({ "entries": entries })).unwrap();
        let again = dispatch("yres.parse", &json!({ "text": written.as_str().unwrap() })).unwrap();
        assert_eq!(again["entries"][3]["comment"], "red");
        assert_eq!(dispatch("yres.parse", &json!({ "text": "{ \"a\": [] }" })).unwrap_err()["line"], 1);
        assert!(dispatch("nope", &Value::Null).is_err());
    }

    #[test]
    fn data_is_read_and_patched_through_the_service() {
        let text = "{\n  // c\n  a: 1,\n  b: \"@item:x\"\n}";
        assert_eq!(dispatch("data.parse", &json!({ "text": text })).unwrap(), json!(r#"{"a":1,"b":"@item:x"}"#));
        let patched = dispatch("data.patch", &json!({ "text": text, "next": r#"{"a":2,"b":"@item:x"}"# })).unwrap();
        assert_eq!(patched, json!(text.replace("a: 1", "a: 2")));
        assert_eq!(dispatch("data.parse", &json!({ "text": "{ a: }" })).unwrap_err()["line"], 1);
        assert_eq!(dispatch("data.format", &json!({ "value": "[1, 2]" })).unwrap(), json!("[1, 2]"));
    }
}
