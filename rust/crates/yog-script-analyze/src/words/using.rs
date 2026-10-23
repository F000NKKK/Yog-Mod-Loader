//! `using` — names from elsewhere, or an alias.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "using",
    doc: "Brings a namespace's names into this file: `using Yog.Ui;`. \
          `using Grid = Dictionary<Pos, int>;` names a type.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    if matches!(p.tok(), Tok::Ident(_)) && p.nth_is(1, Punct::Eq) {
        let name = p.name("an alias")?;
        p.bump();
        let ty = p.ty()?;
        p.end()?;
        return Ok(DeclKind::Alias { name, ty });
    }
    let path = p.path("a namespace")?;
    p.end()?;
    Ok(DeclKind::Using(path))
}
