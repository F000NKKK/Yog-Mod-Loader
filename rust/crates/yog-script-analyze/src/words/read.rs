//! `read` — a field's every read.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "read",
    doc: "Every read of a field of the patched class: `read FoodData { return … ? FoodData.Full : proceed(); }` — `proceed()` is the read as it was.",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let field = p.any_name("a field")?;
    let body = p.block()?;
    Ok(PatchItem::Read { field, body })
}
