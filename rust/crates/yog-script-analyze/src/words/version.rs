//! `version` — declarations for some versions of the game.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, r#else, mc};

pub static WORD: Word = Word {
    text: "version",
    doc: "Declarations for some of the targeted versions only: `version (mc >= 1.21) { … } else { … }`. \
          The only place code may differ by version; the build checks each branch on its versions.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    p.need(Punct::LParen)?;
    p.need_word(&mc::WORD)?;
    let cond = mc::cond(p)?;
    p.need(Punct::RParen)?;
    let then = p.body()?;
    let otherwise = if p.eat_word(&r#else::WORD) {
        if p.at_word(&WORD) { Some(vec![p.decl()?]) } else { Some(p.body()?) }
    } else {
        None
    };
    Ok(DeclKind::Version { cond, then, otherwise })
}
