//! `state` — values kept per player, world or session.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "state",
    doc: "Values kept per player, world, level or session: `player state { saved var visits = 0 }`, \
          then `p.state.visits`. `saved` ones are in the save; `synced` ones reach the clients.",
    reserved: false,
    roles: &[Role::Decl { starts: Some(starts), parse }],
};

/// `<scope> state {`: the scope comes first.
fn starts(p: &Parser) -> bool {
    matches!(p.tok(), Tok::Ident(_)) && p.nth_word(1, &WORD) && *p.nth(2) == Tok::Punct(Punct::LBrace)
}

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    let scope = p.name("a scope (`player`, `world`, `level`, `session`)")?;
    p.bump();
    Ok(DeclKind::State { scope, body: p.body()? })
}
