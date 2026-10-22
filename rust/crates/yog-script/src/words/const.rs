//! `const` — a value known when the mod is built.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "const",
    doc: "A value known when the mod is built: `const PREFIX = \"g\"`. Numbers and texts players see belong in the mod's `.yres`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a constant")?;
    let ty = if p.eat(Punct::Colon) { Some(p.ty()?) } else { None };
    p.need(Punct::Eq)?;
    Ok(DeclKind::Const { name, ty, value: p.expr()? })
}
