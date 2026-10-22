//! `version` — declarations for some versions of the game.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, r#else, mc};

pub static WORD: Word = Word {
    text: "version",
    doc: "Declarations for some of the targeted versions only: `version mc >= 1.21 { … } else { … }`. \
          The only place code may differ by version; the build checks each branch on its versions.",
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
    let then = p.body()?;
    let otherwise = if p.eat_word(&r#else::WORD) {
        if starts(p) { Some(vec![p.decl()?]) } else { Some(p.body()?) }
    } else {
        None
    };
    Ok(DeclKind::Version { cond, then, otherwise })
}
