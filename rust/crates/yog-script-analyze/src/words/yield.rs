//! `yield`.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, r#break, r#return};

pub static WORD: Word = Word {
    text: "yield",
    doc: "In an iterator (a method giving an `IEnumerable<T>`, `IEnumerator<T>` or `IAsyncEnumerable<T>`): `yield return x;` gives the next item, the method going on from there when the next is asked for; `yield break;` ends it.",
    reserved: false,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let at = p.bump().span;
    if p.eat_word(&r#break::WORD) {
        p.end()?;
        return Ok(Stmt::Yield(None, at));
    }
    p.need_word(&r#return::WORD)?;
    let value = p.expr()?;
    p.end()?;
    Ok(Stmt::Yield(Some(value), at))
}
