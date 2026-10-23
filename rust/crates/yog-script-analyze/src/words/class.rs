//! `class` — a type of objects of one's own.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "class",
    doc: "A type of objects: `class Timer(Ticks every) : Machine, ITickable { Ticks Left = every; }`. \
          Its header is its constructor (`new Timer(5s)`); `:` names one base class, then interfaces. \
          As a constraint (`where T : class`), T is an object.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

/// What a type declaration declares.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    /// Objects, shared where they go (`class`, `record`).
    Object,
    /// Values, copied (`struct`, `record struct`).
    Value,
    /// A promise, with no constructor (`interface`).
    Promise,
}

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    type_decl(p, Form::Object)
}

/// `class`/`struct`/`interface Name header { body }`.
pub(crate) fn type_decl(p: &mut Parser, form: Form) -> PResult<DeclKind> {
    let word = p.any_name("a type's word")?;
    type_rest(p, word, form)
}

/// After a type's word(s): its name, header and body; a record may end
/// with `;` instead of a body.
pub(crate) fn type_rest(p: &mut Parser, word: crate::ast::Name, form: Form) -> PResult<DeclKind> {
    let name = p.name("a type's name")?;
    let header = p.header(form != Form::Promise)?;
    let body = if p.eat(Punct::Semi) { Vec::new() } else { p.body()? };
    Ok(DeclKind::Type { word, name, value: form == Form::Value, header, body })
}
