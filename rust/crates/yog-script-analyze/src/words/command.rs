//! `command` — a chat command and its subcommands.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Tok;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "command",
    doc: "A chat command: `command \"charge\" { server void Fill(Sender source, Player target = source.Player) \
          requires source.IsOp { … } }`. Each method is a subcommand; its parameters are the arguments.",
    reserved: false,
    roles: &[Role::Decl { starts: Some(starts), parse }],
};

fn starts(p: &Parser) -> bool {
    p.at_word(&WORD) && matches!(p.nth(1), Tok::Str(_))
}

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.plain_string("a command's name")?;
    Ok(DeclKind::Command { name, body: p.body()? })
}
