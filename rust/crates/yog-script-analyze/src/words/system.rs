//! `system` — what runs over every object that has some components.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, every};

pub static WORD: Word = Word {
    text: "system",
    doc: "Runs over every object having what its parameters ask for: \
          `server system Solar(Charge charge, Block block) every 1t { … }`. Compiled to the game's \
          indexed lookups and ticking of exactly the matching objects, not a scan.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a system's name")?;
    let params = p.params()?;
    let every = if p.eat_word(&every::WORD) {
        Some(p.no_brace(Parser::expr)?)
    } else {
        None
    };
    let body = p.block()?;
    Ok(DeclKind::System { name, params, every, body })
}
