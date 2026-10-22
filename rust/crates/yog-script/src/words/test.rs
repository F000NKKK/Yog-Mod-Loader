//! `test` — a check run in a headless game.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "test",
    doc: "A check `yog test` runs in a headless world on every targeted version: \
          `test \"a charger fills a battery\" { … expect(…) }`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.plain_string("a test's name")?;
    Ok(DeclKind::Test { name, body: p.block()? })
}
