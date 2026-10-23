//! `on` — what runs when something happens.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "on",
    doc: "What runs when something happens: `on Player.Join(Player player) { … }` in a file, \
          `on Use(Player player) { … }` or `server on Tick { … }` on content and components. \
          A failure in it stops that handler, never the game.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let event = p.path("an event")?;
    let params = if p.at(Punct::LParen) { p.params()? } else { Vec::new() };
    Ok(DeclKind::On { event, params, body: p.block()? })
}
