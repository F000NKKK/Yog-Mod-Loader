//! `type` — another name for a type.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "type",
    doc: "Another name for a type: `type Grid = Map<Pos, Int>`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a type's name")?;
    let generics = p.generics()?;
    p.need(Punct::Eq)?;
    Ok(DeclKind::Type { name, generics, ty: p.ty()? })
}
