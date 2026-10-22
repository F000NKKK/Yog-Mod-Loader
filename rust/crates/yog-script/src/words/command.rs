//! `command` — a chat command and its subcommands.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "command",
    doc: "A command: `command \"charge\" { server fn fill(src: Sender, target: Player = src.player) requires src.isOp { … } }`. \
          Each function is a subcommand; its parameters are the arguments, typed.",
    reserved: false,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.plain_string("the command")?;
    Ok(DeclKind::Command { name, body: p.body()? })
}
