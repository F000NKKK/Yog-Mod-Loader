//! `namespace` — where the declarations after it live.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "namespace",
    doc: "As in C#: `namespace Charge.Machines;` puts the rest of the file in a namespace, \
          `namespace Charge.Machines { … }` what's in its braces. Other files reach its types \
          with `using Charge.Machines;` or in full.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let path = p.path("a namespace")?;
    if p.eat(Punct::Semi) {
        return Ok(DeclKind::Namespace { path, body: None });
    }
    let body = p.body()?;
    Ok(DeclKind::Namespace { path, body: Some(body) })
}
