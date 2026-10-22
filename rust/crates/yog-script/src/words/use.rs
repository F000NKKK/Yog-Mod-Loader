//! `use` — names from elsewhere.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::token::Punct;
use crate::words::{Role, Word, r#as};

pub static WORD: Word = Word {
    text: "use",
    doc: "Brings names from a module into this file: `use yog.ui`, `use mc.world.{Level, Entity}`, `use a.b as c`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let path = p.path("a module")?;
    let mut items = Vec::new();
    if p.at(Punct::Dot) {
        p.bump();
        p.need(Punct::LBrace)?;
        items = p.list(Punct::RBrace, |p| p.any_name("a name"))?;
    }
    let alias = if p.eat_word(&r#as::WORD) { Some(p.name("a name")?) } else { None };
    Ok(DeclKind::Use { path, items, alias })
}
