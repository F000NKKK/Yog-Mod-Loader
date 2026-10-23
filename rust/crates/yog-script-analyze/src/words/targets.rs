//! `targets` — the game versions a `.yns` reaches into.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, mc};

pub static WORD: Word = Word {
    text: "targets",
    doc: "The game versions a `.yns` file's patches are for: `targets mc 1.20.1..26.2;`, `targets mc >= 1.21;`. \
          Required in a `.yns` (patches reach the game's classes, which differ by version); every patch is \
          checked on each version in it.",
    reserved: false,
    roles: &[Role::Decl { starts: Some(starts), parse }],
};

fn starts(p: &Parser) -> bool {
    p.at_word(&WORD) && p.nth_word(1, &mc::WORD)
}

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    p.need_word(&mc::WORD)?;
    let cond = mc::cond(p)?;
    p.end()?;
    Ok(DeclKind::Targets(cond))
}
