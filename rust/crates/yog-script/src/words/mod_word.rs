//! `mod` — the mod itself: its id and what describes it.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "mod",
    doc: "Declares the mod: `mod charge { version = \"1.0.0\"; targets = mc 1.20.1..26.2; resources = \"charge.yres\" }`. One per mod.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("the mod's id")?;
    p.need(Punct::LBrace)?;
    let mut props = Vec::new();
    loop {
        p.skip_newlines();
        if p.eat(Punct::RBrace) {
            return Ok(DeclKind::Mod { name, props });
        }
        if *p.tok() == Tok::Eof {
            return Err(p.expected("`}`"));
        }
        match p.prop().and_then(|prop| p.end_stmt().map(|()| prop)) {
            Ok(prop) => props.push(prop),
            Err(_) => p.recover(),
        }
    }
}
