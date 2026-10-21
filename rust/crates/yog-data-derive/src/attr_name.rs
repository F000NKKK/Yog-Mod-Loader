//! The words of `#[data(...)]`: the attribute itself and each of its options.

pub const CRATE: &str = "crate";
pub const DATA: &str = "data";
pub const DEFAULT: &str = "default";
pub const DOC: &str = "doc";
pub const OTHER: &str = "other";
pub const RANGE: &str = "range";
pub const RENAME: &str = "rename";
pub const RENAME_ALL: &str = "rename_all";
/// `rename_all`'s one case: fields (a struct's) or variants (an enum's) in lower camel case.
pub const CAMEL_CASE: &str = "camelCase";
pub const SKIP: &str = "skip";
pub const TAG: &str = "tag";
pub const VIEW: &str = "view";
pub const WITH: &str = "with";
