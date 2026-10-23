//! `interface` — what a type promises.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, class};

pub static WORD: Word = Word {
    text: "interface",
    doc: "What a type promises: `interface IChargeable { Energy Room { get; } }`. Types, content and \
          patched classes (`add : IChargeable;`) implement it; it inherits any interfaces and may give members bodies.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    class::type_decl(p, class::Form::Promise)
}
