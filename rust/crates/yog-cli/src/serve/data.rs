//! The service's own methods over Yog Data: its version, Yog Data text
//! read, patched and formatted, typed strings classified, the formats'
//! shapes.

use yog_data::{Schema, Value};
use yog_ids::serve_method as M;
use yog_ids::serve_param as P;

use super::{Reply, ServiceError};

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
        M::DATA_PARSE => text(params, P::TEXT)
            .and_then(|t| yog_data::text::parse_value(t).map_err(|e| ServiceError::syntax(&e))),
        // `text` changed to hold `next`, comments and layout of the rest kept.
        M::DATA_PATCH => text(params, P::TEXT)
            .and_then(|t| Ok(yog_data::patch::patch(t, param(params, P::NEXT)?).into())),
        // A value as Yog's editors write it, at indent `level`.
        M::DATA_FORMAT => {
            let level = params.get(P::LEVEL).and_then(Value::as_i64).unwrap_or(0);
            param(params, P::VALUE)
                .map(|v| yog_data::patch::format(v, usize::try_from(level).unwrap_or(0)).into())
        }
        // Strings as typed values (`@item:x`, `@tag:item:x`, `@res:Set.key`, `@image:…`,
        // `@lang:…`, `@@` escaped, else plain text): each `{t, …}` or `{error}`.
        M::TYPED_CLASSIFY => param(params, P::VALUES).and_then(|values| {
            let values = values.as_list().ok_or("`values` is a list")?;
            Ok(Value::List(values.iter().map(classified).collect()))
        }),
        // A Yog format's file as text (binary on disk: written out as the editor shows it).
        M::SOURCE_READ => text(params, P::PATH).and_then(|p| {
            crate::files::read_source(p)
                .map(Value::from)
                .map_err(|e| format!("{p}: {e}").into())
        }),
        // Text saved as a Yog format's file: binary, its comments kept as notes.
        M::SOURCE_WRITE => text(params, P::PATH).and_then(|p| {
            let bytes = yog_data::source::binary(text(params, P::TEXT)?)
                .map_err(|e| ServiceError::syntax(&e))?;
            std::fs::write(p, bytes).map_err(|e| ServiceError::from(format!("{p}: {e}")))?;
            Ok(Value::Null)
        }),
        // A source as edited (unsaved `text` of the file at `path`), checked:
        // its syntax, then its format's problems, each at its line and column.
        M::SOURCE_CHECK => text(params, P::PATH)
            .and_then(|p| Ok(super::check::check(p, text(params, P::TEXT)?).write())),
        // A file's data of a format, every field it leaves out at its default
        // (what a designer's preview takes, as the game does).
        M::FORMAT_COMPLETE => param(params, P::FORMAT).and_then(|name| {
            let name = name.as_str().ok_or("`format` is a name")?;
            let format = yog_ids::Format::of(name).ok_or_else(|| format!("no format `{name}`"))?;
            yog_formats::complete(format, param(params, P::VALUE)?)
                .map_err(|e| ServiceError::from(crate::diagnostics::listed(&e)))
        }),
        // A format's shape (`yog_data::Shape::describe`): `yblock`, `.ytexture`, `yog.toml`…
        M::FORMAT_SHAPE => param(params, P::FORMAT).and_then(|name| {
            let name = name.as_str().ok_or("`format` is a name")?;
            yog_formats::shape_of(name)
                .map(|shape| shape.describe().write())
                .ok_or_else(|| format!("no format `{name}`").into())
        }),
        _ => return None,
    })
}

/// The parameter `name`.
fn param<'a>(params: &'a Value, name: &str) -> Result<&'a Value, ServiceError> {
    params
        .get(name)
        .ok_or_else(|| format!("missing `{name}`").into())
}

/// The text parameter `name`.
fn text<'a>(params: &'a Value, name: &str) -> Result<&'a str, ServiceError> {
    param(params, name)?
        .as_str()
        .ok_or_else(|| format!("`{name}` is text").into())
}

/// A string as a typed value, the way a tool takes it.
fn classified(v: &Value) -> Value {
    use yog_data::typed::Spelled;
    let typed = match yog_data::typed::classify(v.as_str().unwrap_or_default()) {
        Ok(Spelled::Ref(r)) => match yog_data::Meaning::of(r.clone()) {
            Some(yog_data::Meaning::Tag(t)) => Typed::Tag {
                kind: t.kind,
                id: t.id,
            },
            Some(yog_data::Meaning::Res(r)) => Typed::Res {
                set: r.set,
                key: r.key,
            },
            Some(yog_data::Meaning::Image(source)) => Typed::Image { source },
            Some(yog_data::Meaning::Lang(key)) => Typed::Lang { key },
            Some(yog_data::Meaning::Content(c)) => Typed::Ref {
                kind: c.kind.as_str().to_owned(),
                id: c.id.as_str().to_owned(),
            },
            None => {
                return NotTyped {
                    error: format!(
                        "`{r}`: `{}` isn't an id (lower case `namespace:path`)",
                        r.id
                    ),
                }
                .write();
            }
        },
        Ok(Spelled::Bytes(_)) => Typed::Bytes,
        Ok(Spelled::Plain(v)) => Typed::Str { v },
        Err(e) => {
            return NotTyped {
                error: e.to_string(),
            }
            .write();
        }
    };
    typed.write()
}
