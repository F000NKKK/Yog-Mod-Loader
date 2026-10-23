//! `using` — what a namespace has, here; or an alias.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word, r#static};

pub static WORD: Word = Word {
    text: "using",
    doc: "As in C#: `using Yog.Ui;` brings in the types of a namespace (not the namespaces in it); \
          `using static Yog.Chat;` the static members of a type; `using Grid = Dictionary<Pos, int>;` \
          names a type or a namespace; `global using …;` is every file's. A name two `using`s bring \
          in is ambiguous: write it in full or alias it.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    rest(p, false)
}

/// After `global`, or at `using`: the directive.
pub(crate) fn rest(p: &mut Parser, global: bool) -> PResult<DeclKind> {
    p.need_word(&WORD)?;
    if matches!(p.tok(), Tok::Ident(_)) && p.nth_is(1, Punct::Eq) {
        let name = p.name("an alias")?;
        p.bump();
        let ty = p.ty()?;
        p.end()?;
        return Ok(DeclKind::Alias { name, ty });
    }
    let statics = p.eat_word(&r#static::WORD);
    let path = p.path("a namespace")?;
    p.end()?;
    Ok(DeclKind::Using { path, global, statics })
}
