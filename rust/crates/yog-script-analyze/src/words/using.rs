//! `using` — what a namespace has, here; or an alias.

use crate::ast::{Binding, Block, DeclKind, Name, Stmt};
use yog_script_ids::members::USED;
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word, r#let, r#static};

pub static WORD: Word = Word {
    text: "using",
    doc: "As in C#: `using Yog.Ui;` brings in the types of a namespace (not the namespaces in it); \
          `using static Yog.Chat;` the static members of a type; `using Grid = Dictionary<Pos, i32>;` \
          names a type or a namespace; `global using …;` is every file's. A name two `using`s bring \
          in is ambiguous: write it in full or alias it. In code, as C#'s `using` statement: \
          `using (let g = l.Enter()) { … }` disposes `g` when the statement ends, \
          `using let g = l.Enter();` when its block does (as any local's value is).",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }, Role::Stmt { parse: stmt }],
};

/// `using (let x = e) body`, `using (e) body`: a block of the local and the
/// body (its value disposed when the block ends); `using let x = e;` a local.
fn stmt(p: &mut Parser) -> PResult<Stmt> {
    let at = p.span();
    p.need_word(&WORD)?;
    if !p.eat(Punct::LParen) {
        let local = r#let::local(p)?;
        p.end()?;
        return Ok(local);
    }
    let local = if p.at_word(&r#let::WORD) {
        r#let::local(p)?
    } else {
        let value = p.expr()?;
        let name = Name { text: USED.to_string(), span: value.span };
        Stmt::Local { ty: None, binding: Binding::One(name), value: Some(value), mutable: false }
    };
    p.need(Punct::RParen)?;
    let body = p.stmt()?;
    Ok(Stmt::Block(Block { stmts: vec![local, body], span: at.to(p.prev()) }))
}

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
