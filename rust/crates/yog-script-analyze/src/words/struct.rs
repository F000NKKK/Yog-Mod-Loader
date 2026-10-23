//! `struct` — a type of values.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, class};

pub static WORD: Word = Word {
    text: "struct",
    doc: "A type of values, copied where they go: `struct Pos(i32 X, i32 Y, i32 Z) { … }`. It \
          implements interfaces but inherits nothing. As a constraint (`where T : struct`), T is a value.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    class::type_decl(p, class::Form::Value)
}
