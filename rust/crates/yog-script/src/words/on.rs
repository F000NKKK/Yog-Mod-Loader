//! `on` — what runs when something happens.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "on",
    doc: "What runs when something happens: `on player.join(p) { … }` in a file, `on use(p) { … }` \
          or `server on tick { … }` on content. A failure in it stops that handler, never the game.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let mut event = vec![p.any_name("an event")?];
    while p.eat(Punct::Dot) {
        event.push(p.any_name("an event")?);
    }
    let params = p.untyped_params()?;
    Ok(DeclKind::On { event, params, body: p.block()? })
}
