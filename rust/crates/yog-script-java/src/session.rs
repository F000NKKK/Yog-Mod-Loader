//! What a hot reload carries of a live object (docs/design/storage.md S8):
//! the object is made live in the session under its owner's key
//! (`Session.live`: mod|class|owner), and the runtime carries every field
//! of its old generation's object into it, whatever its type. An
//! `extend`'s class also says how to make its instance of an owner
//! (`Session.maker`), so a reload makes the live ones at once.

use yog_ids::storage::SESSION_KEY_SEP as KEY_SEP;
use yog_script_ids::jvm::session::{CLASS, LIVE, MAKER};

/// Whether `body` (a class's) has anything a hot reload carries: an instance field.
pub(crate) fn keeps(body: &[yog_script_analyze::ast::Decl]) -> bool {
    !crate::classes::fields(body).is_empty()
}

/// `made` live under its owner's key: the mod's, `class`'s, `owner`'s.
pub(crate) fn live(made: &str, (module, class, owner): (&str, &str, &str)) -> String {
    format!(
        "{CLASS}.{LIVE}({made}, {} + {owner})",
        prefix(module, class)
    )
}

/// `class`'s instances of an owner made by `make` (a function of the owner's key).
pub(crate) fn maker(module: &str, class: &str, make: &str) -> String {
    format!("{CLASS}.{MAKER}({}, {make});", prefix(module, class))
}

/// The start of `class`'s keys: `module + "|class|"`.
fn prefix(module: &str, class: &str) -> String {
    format!("{module} + \"{KEY_SEP}{class}{KEY_SEP}\"")
}
