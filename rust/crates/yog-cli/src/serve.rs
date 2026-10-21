//! `yog serve` — the CLI as a service for tools (Yog-IDLE): frames of
//! binary Yog Data (`yog_data::io`) on stdin and stdout, each request a
//! [`Request`] and each answer a [`Reply`]. Every format is read, written
//! and converted here, so a tool needn't be rebuilt when a format changes —
//! only this binary. Answers hold plain values: a typed one (`@item:x`) is
//! sent as its string.
//!
//! Methods: the service's own (`data`), a mod's resources (`media`) and
//! the designers' (`design::service`).

mod data;
mod media;

use yog_data::Schema;

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
    let (method, params) = (request.method.as_str(), &request.params);
    let reply = data::method(method, params)
        .or_else(|| media::method(method, params))
        // The designers: drawing, pickers' choices, edits of a mod's files.
        .or_else(|| crate::design::service::dispatch(method, params))
        .unwrap_or_else(|| Err(format!("no method `{method}`").into()));
    reply
        .map(|v| yog_data::typed::flatten(&v))
        .map_err(|e| yog_data::typed::flatten(&e))
}

#[cfg(test)]
mod tests;
