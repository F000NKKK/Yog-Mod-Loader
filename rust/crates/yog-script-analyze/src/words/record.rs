//! `record` — a type made of named values.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, class};

pub static WORD: Word = Word {
    text: "record",
    doc: "A type made of named values, compared by value: `record Pos(int X, int Y, int Z);`. Its \
          constructor's parameters are its properties; members may follow in `{ … }`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    class::type_decl(p, true)
}
