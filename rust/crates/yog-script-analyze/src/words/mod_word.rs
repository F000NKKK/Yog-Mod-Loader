//! `mod` — the mod itself.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "mod",
    doc: "The mod itself: `mod Charge { Version = \"1.0.0\"; Targets = mc 1.20.1..26.2; Resources = \"charge.yres\"; }`. \
          Its id is its name in snake_case unless `Id = \"…\";` says otherwise.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("the mod's name")?;
    p.need(Punct::LBrace)?;
    let mut props = Vec::new();
    while !p.eat(Punct::RBrace) {
        props.push(p.prop()?);
    }
    Ok(DeclKind::Mod { name, props })
}
