//! A set's generated Yog Script: a class nested in `Res` with a member per
//! entry, as its Rust ([`super::accessors`]) has an accessor — the mod's
//! `.ys` reaches its resources as `Res.<Set>.<Entry>` (`res.g.ys`).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use yog_codegen::{Blocks, Comments, Header, Regenerate};
use yog_codegen_ys::Ys;
use yog_script_analyze::words::{
    bool as bool_word, f64 as f64_word, i64 as i64_word, u32 as u32_word,
};

use super::{Entry, Kind, Value};

/// The generated file's header.
const HEADER: Header = Header {
    tool: "`yog build`",
    source: "resources/**/*.yres",
    regenerate: Regenerate::EveryBuild,
};

/// What every set is nested in: `Res.Strings.Title`.
const ROOT: &str = "Res";

/// An entry's member's name: its accessor's in PascalCase (`screen_title`:
/// `ScreenTitle`).
fn member(acc: &str) -> String {
    yog_codegen::pascal(acc)
}

/// A text's member: its value its translation key.
pub(super) fn text(acc: &str, key: &str) -> String {
    Ys::text_getter(&member(acc), key)
}

/// An image's, sound's, file's or video's member: its id.
pub(super) fn medium(acc: &str, id: &str) -> String {
    Ys::const_string(&member(acc), id)
}

/// A color's, number's or flag's member: a constant.
pub(super) fn value(e: &Entry, acc: &str) -> Option<String> {
    let (ty, v) = match (e.kind, &e.value) {
        (Kind::Color, _) => (
            u32_word::WORD.text,
            format!("0x{:08X}", super::color_of(e)?),
        ),
        (Kind::Number, Value::Num(lit)) => match super::accessors::number(lit) {
            (true, v) => (i64_word::WORD.text, v),
            (false, v) => (f64_word::WORD.text, v),
        },
        (Kind::Bool, v) => (
            bool_word::WORD.text,
            matches!(v, Value::Bool(true)).to_string(),
        ),
        _ => return None,
    };
    Some(Ys::const_value(ty, &member(acc), &v))
}

/// The file of every set: `Res` with a class per set (`sets`: name →
/// (where it's read from, its members)); none without sets.
pub(super) fn source(sets: &BTreeMap<String, (String, Vec<String>)>) -> Option<String> {
    if sets.is_empty() {
        return None;
    }
    let indent = Ys::INDENT;
    let mut code = HEADER.comment::<Ys>();
    code.push_str(&Ys::open(&Ys::static_class(ROOT)));
    code.push('\n');
    for (set, (shown, members)) in sets {
        let _ = writeln!(
            code,
            "{indent}{}\n{indent}{}",
            Ys::doc(&format!("The resources of `{shown}`.")),
            Ys::open(&Ys::static_class(set))
        );
        for m in members {
            let _ = writeln!(code, "{indent}{indent}{m}");
        }
        let _ = writeln!(code, "{indent}{}", Ys::close(""));
    }
    code.push_str(&Ys::close(""));
    code.push('\n');
    Some(code)
}
