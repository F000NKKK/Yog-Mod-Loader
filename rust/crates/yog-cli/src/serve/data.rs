//! The service's own methods over Yog Data: its version, Yog Data text
//! read, patched and formatted, typed strings classified, the formats'
//! shapes.

use yog_data::{Schema, Value};
use yog_ids::serve_method as M;
use yog_ids::serve_param as P;

use super::Reply;

/// Text that doesn't parse: what's wrong, and where.
#[derive(Schema)]
struct ParseError {
    message: String,
    line: u32,
    col: u32,
}

/// A string as a typed value, the way a tool takes it: `{t, …}`.
#[derive(Schema)]
#[data(tag = "t")]
enum Typed {
    Ref { kind: String, id: String },
    Tag { kind: String, id: String },
    Res { set: String, key: String },
    Image { source: String },
    Lang { key: String },
    Bytes,
    Str { v: String },
}

/// A string that isn't a typed value though it's written as one.
#[derive(Schema)]
struct NotTyped {
    error: String,
}

/// `method` answered, if it's one of these.
pub(super) fn method(method: &str, params: &Value) -> Option<Reply> {
    Some(match method {
        // The service's version: a tool restarts it when the binary changes.
        M::VERSION => Ok(env!("CARGO_PKG_VERSION").into()),
        // Yog Data text → its value (maps in file order).
        M::DATA_PARSE => text(params).and_then(|t| {
            yog_data::text::parse_value(t).map_err(|e| {
                ParseError {
                    message: e.to_string(),
                    line: e.at.line,
                    col: e.at.col,
                }
                .write()
            })
        }),
        // `text` changed to hold `next`, comments and layout of the rest kept.
        M::DATA_PATCH => {
            text(params).and_then(|t| Ok(yog_data::patch::patch(t, param(params, P::NEXT)?).into()))
        }
        // A value as Yog's editors write it, at indent `level`.
        M::DATA_FORMAT => {
            let level = params.get(P::LEVEL).and_then(Value::as_i64).unwrap_or(0);
            param(params, P::VALUE)
                .map(|v| yog_data::patch::format(v, usize::try_from(level).unwrap_or(0)).into())
        }
        // Strings as typed values (`@item:x`, `@tag:item:x`, `@Set.key`, `@image:…`,
        // `@lang:…`, `@@` escaped, else plain text): each `{t, …}` or `{error}`.
        M::TYPED_CLASSIFY => param(params, P::VALUES).and_then(|values| {
            let values = values.as_list().ok_or("`values` is a list")?;
            Ok(Value::List(values.iter().map(classified).collect()))
        }),
        // A format's shape (`yog_data::Shape::describe`): `yblock`, `.ytexture`, `yog.toml`…
        M::FORMAT_SHAPE => param(params, P::FORMAT).and_then(|name| {
            let name = name.as_str().ok_or("`format` is a name")?;
            yog_formats::shape_of(name)
                .map(|shape| shape.describe())
                .ok_or_else(|| format!("no format `{name}`").into())
        }),
        _ => return None,
    })
}

/// The parameter `name`.
fn param<'a>(params: &'a Value, name: &str) -> Result<&'a Value, Value> {
    params
        .get(name)
        .ok_or_else(|| format!("missing `{name}`").into())
}

/// The `text` parameter.
fn text(params: &Value) -> Result<&str, Value> {
    param(params, P::TEXT)?
        .as_str()
        .ok_or_else(|| format!("`{}` is text", P::TEXT).into())
}

/// A string as a typed value, the way a tool takes it.
fn classified(v: &Value) -> Value {
    let typed = match yog_data::typed::classify(v.as_str().unwrap_or_default()) {
        Ok(Value::Ref(r)) => Typed::Ref {
            kind: r.kind,
            id: r.id,
        },
        Ok(Value::Tag(r)) => Typed::Tag {
            kind: r.kind,
            id: r.id,
        },
        Ok(Value::Res(r)) => Typed::Res {
            set: r.set,
            key: r.key,
        },
        Ok(Value::Image(source)) => Typed::Image { source },
        Ok(Value::Lang(key)) => Typed::Lang { key },
        Ok(Value::Bytes(_)) => Typed::Bytes,
        Ok(Value::Str(v)) => Typed::Str { v },
        Ok(other) => Typed::Str {
            v: yog_data::typed::typed_string(&other).unwrap_or_default(),
        },
        Err(error) => return NotTyped { error }.write(),
    };
    typed.write()
}
