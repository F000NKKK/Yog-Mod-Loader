//! `from` — a query, as C#'s: `from x in xs where … select …`.

use crate::ast::Expr;
use crate::parser::{PResult, Parser};
use crate::source::token::Tok;
use crate::words::{Role, Word, r#in};

pub static WORD: Word = Word {
    text: "from",
    doc: "A query over items, as C#'s: `from p in players where p.Level > 3 orderby p.Name select p.Name`, \
          with `let`, more `from`s, `join … on … equals … (into …)`, `group … by …`, `into` — the same \
          as LINQ's calls (`players.Where(p => …).OrderBy(…).Select(…)`).",
    reserved: false,
    roles: &[Role::Prefix { starts: Some(starts), parse }],
};

/// `from x in` or `from T x in`: a query; `from` is a name otherwise.
fn starts(p: &Parser) -> bool {
    if !p.at_word(&WORD) {
        return false;
    }
    let named_at = |n: usize| matches!(p.nth(n), Tok::Ident(_)) && p.nth_word(n + 1, &r#in::WORD);
    named_at(1) || p.type_len(1).is_some_and(|len| named_at(1 + len))
}

fn parse(p: &mut Parser) -> PResult<Expr> {
    p.query()
}
