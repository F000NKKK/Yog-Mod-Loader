//! `was` — what an old save called a field.

use crate::ast::{Name, Type};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "was",
    doc: "What an old save called a field (and its old type): `saved var energy: Energy = 0 was power: Int`. \
          Old worlds load; the value is converted where the types differ.",
    reserved: true,
    roles: &[Role::Part],
};

/// `was name (: Type)?`, or nothing.
pub(crate) fn migration(p: &mut Parser) -> PResult<Option<(Name, Option<Type>)>> {
    if !p.eat_word(&WORD) {
        return Ok(None);
    }
    let name = p.name("the old name")?;
    let ty = if p.eat(Punct::Colon) { Some(p.ty()?) } else { None };
    Ok(Some((name, ty)))
}
