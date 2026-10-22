//! `enum` — one of a fixed set of names.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, union};

pub static WORD: Word = Word {
    text: "enum",
    doc: "One of a fixed set of names: `enum Tier { Wood, Stone, Iron }`. Cases with data are a `union`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("an enum's name")?;
    let (cases, body) = union::cases(p, |p| p.name("a case"))?;
    Ok(DeclKind::Enum { name, cases, body })
}
