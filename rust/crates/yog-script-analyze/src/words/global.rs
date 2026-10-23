//! `global` — a `using` of every file.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, using};

pub static WORD: Word = Word {
    text: "global",
    doc: "`global using Yog.Ui;`: the directive is every file's of the mod, as in C#.",
    reserved: false,
    roles: &[Role::Decl { starts: Some(starts), parse }],
};

fn starts(p: &Parser) -> bool {
    p.at_word(&WORD) && p.nth_word(1, &using::WORD)
}

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    using::rest(p, true)
}
