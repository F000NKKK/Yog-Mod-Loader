//! `record` — a type made of named values.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::class::{self, Form};
use crate::words::{Role, Word, r#struct};

pub static WORD: Word = Word {
    text: "record",
    doc: "A type made of named values, compared by value: `record Pos(i32 X, i32 Y, i32 Z);`. Its \
          constructor's parameters are its properties; `with` copies one with some changed. \
          `record struct` is a value, `record` (`record class`) an object.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    let word = p.any_name("a type's word")?;
    let form = if p.eat_word(&r#struct::WORD) {
        Form::Value
    } else {
        p.eat_word(&class::WORD);
        Form::Object
    };
    class::type_rest(p, word, form)
}
