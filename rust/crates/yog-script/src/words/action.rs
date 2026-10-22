//! `action` — what a screen does, run on the server.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, r#fn};

pub static WORD: Word = Word {
    text: "action",
    doc: "What a screen does when asked: `action dump() { block.energy = 0 }`. It runs on the server; \
          the message from the client is made for it.",
    reserved: false,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    r#fn::rest(p).map(DeclKind::Action)
}
