//! `class` — a type of objects of one's own.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "class",
    doc: "A type of objects: `class Timer(Ticks every) { Ticks Left = every; }`. Its header is its \
          constructor (`new Timer(5s)`); `: IName` implements interfaces. No class inheritance: \
          share behaviour through components.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    type_decl(p, true)
}

/// `class`/`record`/`interface Name header { body }`; a record may end
/// with `;` instead of a body.
pub(crate) fn type_decl(p: &mut Parser, ctor: bool) -> PResult<DeclKind> {
    let word = p.any_name("a type's word")?;
    let name = p.name("a type's name")?;
    let header = p.header(ctor)?;
    let body = if p.eat(crate::source::token::Punct::Semi) {
        Vec::new()
    } else {
        p.body()?
    };
    Ok(DeclKind::Type { word, name, header, body })
}
