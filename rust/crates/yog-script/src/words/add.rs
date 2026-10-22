//! `add` — members added to a patched class.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, is};

pub static WORD: Word = Word {
    text: "add",
    doc: "Adds to the patched class: a field (`add saved var shield: Float = 0` — `saved` and `synced` \
          work here too), a function (`add fn …`), an interface (`add is Chargeable`).",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    if p.eat_word(&is::WORD) {
        return p.ty_plain().map(PatchItem::AddIs);
    }
    p.decl().map(|d| PatchItem::Add(Box::new(d)))
}
