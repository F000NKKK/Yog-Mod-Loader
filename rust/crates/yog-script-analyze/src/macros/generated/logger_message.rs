//! `[LoggerMessage(level, "template")] static partial void Moved(ILogger
//! logger, i32 count);` (.NET's `LoggerMessage` source generator): its
//! body checks the level first — nothing made when it's off — then logs a
//! state of its own: the values typed, unboxed until a provider asks for
//! them by name, and the message formatted by code the compiler wrote,
//! not a template read at each entry.

use yog_script_ids::system::{ERROR, EVENT_ID_SUFFIX, LOG_LEVEL, LOG_VALUES_SUFFIX, LOGGER};

use super::signature::{Args, Param, Signature};
use super::template;
use crate::lexer::string::QUOTE;
use crate::parser::pattern::WILDCARD;
use crate::source::token::Punct;
use crate::words::null;

/// The code written, as a template: `$Name$`s filled.
const CODE: &str = include_str!("LoggerMessage.ys");

/// What a method without an error parameter logs as its error.
const NO_ERROR: &str = "null";
/// The event id of a `[LoggerMessage]` naming none.
const NO_EVENT_ID: &str = "0";

/// The method's code, or why there's none.
pub(super) fn generate(
    sig: &Signature,
    args: &Args,
    instance_logger: Option<&str>,
) -> Result<String, String> {
    if !sig.partial || !sig.void {
        return Err(format!(
            "a `[LoggerMessage]` method is `partial` and gives `void`: `static partial void {}(…);`",
            sig.name
        ));
    }
    let typed = |name: &str| sig.params.iter().find(|p| bare(&p.ty) == name);
    let logger = typed(LOGGER)
        .map(|p| p.name.as_str())
        .or(instance_logger)
        .ok_or_else(|| {
            format!(
                "`{}` has no `{LOGGER}` to log to: a parameter, or a field of its class",
                sig.name
            )
        })?;
    let level = match (&args.level, typed(LOG_LEVEL)) {
        (Some(l), _) => l.clone(),
        (None, Some(p)) => p.name.clone(),
        (None, None) => {
            return Err(format!(
                "`{}` has no level: the attribute's, or a `{LOG_LEVEL}` parameter",
                sig.name
            ));
        }
    };
    let message = args.message.as_deref().unwrap_or_default();
    let pieces =
        template::pieces(message).ok_or_else(|| format!("`{message}`: a hole isn't closed"))?;
    let values: Vec<&Param> = sig
        .params
        .iter()
        .filter(|p| ![LOGGER, LOG_LEVEL, ERROR].contains(&bare(&p.ty)))
        .collect();
    let field = |hole: &str| {
        values
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(hole))
            .map(|p| p.name.clone())
    };
    let names = template::names(&pieces);
    if let Some(missing) = names.iter().find(|n| field(n).is_none()) {
        return Err(format!(
            "`{{{missing}}}` names no parameter of `{}`",
            sig.name
        ));
    }
    let names_at = arms(
        names
            .iter()
            .map(|n| format!("{QUOTE}{}{QUOTE}", template::escaped(n))),
        format!("{QUOTE}{QUOTE}"),
    );
    let values_at = arms(
        names.iter().map(|n| field(n).unwrap_or_default()),
        null::WORD.text.to_string(),
    );
    let list =
        |f: &dyn Fn(&Param) -> String| values.iter().map(|p| f(p)).collect::<Vec<_>>().join(", ");
    let error = typed(ERROR).map_or(NO_ERROR, |p| p.name.as_str());
    let state = format!("{}{LOG_VALUES_SUFFIX}", sig.name);
    let event_field = format!("_{}{EVENT_ID_SUFFIX}", camel(&sig.name));
    let params = sig
        .params
        .iter()
        .map(|p| format!("{} {}", p.ty, p.name))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(fill(
        CODE,
        &[
            ("Modifiers", &sig.modifiers.join(" ")),
            ("Method", &sig.name),
            ("Params", &params),
            ("Logger", logger),
            ("Level", &level),
            ("EventField", &event_field),
            ("EventId", args.event_id.as_deref().unwrap_or(NO_EVENT_ID)),
            ("State", &state),
            ("Values", &list(&|p| p.name.clone())),
            ("Fields", &list(&|p| format!("{} {}", p.ty, p.name))),
            ("Error", error),
            ("Count", &names.len().to_string()),
            ("Names", &names_at),
            ("Arms", &values_at),
            ("Template", &template::escaped(message)),
            (
                "Message",
                &template::message(&pieces, |h| field(h).unwrap_or_default()),
            ),
        ],
    ))
}

/// What marks a name to fill in a template: `$Name$`.
const MARK: char = '$';

/// `code` with each `$name$` of `with` replaced by its value (once:
/// what's filled in is never read for marks).
fn fill(code: &str, with: &[(&str, &str)]) -> String {
    let mut out = String::new();
    let mut rest = code;
    while let Some(start) = rest.find(MARK) {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let name = after.find(MARK).map(|end| &after[..end]);
        match name.and_then(|n| with.iter().find(|(k, _)| *k == n)) {
            Some((n, value)) => {
                out.push_str(value);
                rest = &after[n.len() + 1..];
            }
            None => {
                out.push(MARK);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A `switch`'s arms: each of `values` by its index, `otherwise` for the rest.
fn arms(values: impl Iterator<Item = String>, otherwise: String) -> String {
    values
        .enumerate()
        .map(|(i, v)| format!("{i} {} {v}", Punct::FatArrow.text()))
        .chain(std::iter::once(format!(
            "{WILDCARD} {} {otherwise}",
            Punct::FatArrow.text()
        )))
        .collect::<Vec<_>>()
        .join(", ")
}

/// What makes a type nullable (`ILogger?`).
const NULLABLE: char = '?';

/// Type `ty` as written without its `?`.
fn bare(ty: &str) -> &str {
    ty.trim_end_matches(NULLABLE)
}

/// `Name` as `name`.
fn camel(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(|c| c.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}
