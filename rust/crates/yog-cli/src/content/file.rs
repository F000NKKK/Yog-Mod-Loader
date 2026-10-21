//! What every content file says the same way: its id and its format's version.

use std::path::Path;

use yog_data::Path as At;

use crate::content::ctx::{Ctx, Loc};
use crate::content::{valid_namespace, valid_path};
use yog_ids::Format;
use yog_ids::sep::{EXT, NAMESPACE, PATH};

/// What joins an id's parts in a generated file's name: `ns__path__more`.
const NAME_JOIN: &str = "__";

/// Where `id`'s data is kept under `.yog-build/generated/`, in folder
/// `dir`: `<dir>/<ns>__<path>.<ext>`.
pub(super) fn generated(dir: &str, id: &str, format: Format) -> String {
    let name = id.replace(NAMESPACE, NAME_JOIN).replace(PATH, NAME_JOIN);
    format!("{dir}{PATH}{name}{EXT}{}", format.ext())
}

/// The one format version of the content files.
pub(super) const FORMAT: u32 = 1;

/// Whether `format` (when the file gives one, at `at`) is [`FORMAT`].
pub(super) fn check_format(cx: &mut Ctx, loc: Loc, at: &At, format: Option<u32>) -> bool {
    let ok = format.is_none_or(|f| f == FORMAT);
    if !ok {
        cx.err_at(
            loc,
            at,
            format!("must be {FORMAT} (this yog understands format {FORMAT})"),
        );
    }
    ok
}

/// A file's id: as written, else the file's name; the mod's namespace by default.
pub(super) fn file_id(
    cx: &mut Ctx,
    line: usize,
    id: Option<&str>,
    path: &Path,
    mod_ns: Option<&str>,
) -> Option<(String, String)> {
    let raw = id.map(str::to_string).unwrap_or_else(|| {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    let (ns, p) = match raw.split_once(yog_ids::sep::NAMESPACE) {
        Some((ns, p)) => (ns.to_string(), p.to_string()),
        None => match mod_ns {
            Some(ns) => (ns.to_string(), raw.clone()),
            None => {
                cx.err(line, "the mod id isn't a valid Minecraft namespace (a-z, 0-9, `_`, `-`, `.`), so give the namespace: id: \"mynamespace:name\"");
                return None;
            }
        },
    };
    if !valid_namespace(&ns) || !valid_path(&p) {
        cx.err(line, format!("id `{ns}:{p}` isn't a valid Minecraft id — lower case a-z, 0-9, `_`, `-`, `.` (and `/` in the path)"));
        return None;
    }
    Some((ns, p))
}
