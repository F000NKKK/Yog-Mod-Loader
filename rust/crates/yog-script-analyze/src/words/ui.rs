//! `ui` — the markup a screen shows.

use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "ui",
    doc: "The `.yui` markup a screen shows: `screen ChargerScreen(…) ui \"charger.yui\" { … }`.",
    reserved: false,
    roles: &[Role::Part],
};

/// `ui "file.yui"`.
pub(crate) fn file(p: &mut Parser) -> PResult<String> {
    p.need_word(&WORD)?;
    p.plain_string("the screen's `.yui` file")
}
