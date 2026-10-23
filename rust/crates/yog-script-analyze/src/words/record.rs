//! `record` — a type made of named values.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "record",
    doc: "A type made of named values, compared by value: `record Pos(x: Int, y: Int, z: Int)`, members in `{ … }`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a record's name")?;
    let generics = p.generics()?;
    let fields = p.params()?;
    let body = if p.at(Punct::LBrace) { p.body()? } else { Vec::new() };
    Ok(DeclKind::Record { name, generics, fields, body })
}
