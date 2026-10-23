//! `mod` — the mod itself.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
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
    let body = p.body()?;
    Ok(DeclKind::Mod { name, body })
}
