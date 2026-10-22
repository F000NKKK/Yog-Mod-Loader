//! `union` — a value that is one of several cases, each with its own data.

use crate::ast::{Decl, DeclKind, Variant};
use crate::parser::{PResult, Parser};
use crate::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "union",
    doc: "A value that is one of several cases, each with its own data: \
          `union Mode { Idle, Charging(rate: Int), Full }`. A `match` over it covers every case.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a union's name")?;
    let generics = p.generics()?;
    let (variants, body) = cases(p, |p| {
        let name = p.name("a case")?;
        let fields = if p.at(Punct::LParen) { p.params()? } else { Vec::new() };
        Ok(Variant { name, fields })
    })?;
    Ok(DeclKind::Union { name, generics, variants, body })
}

/// `{ A, B(…), C  members }`: cases by `case`, separated by `,` or lines,
/// then declarations.
pub(crate) fn cases<T>(p: &mut Parser, case: impl Fn(&mut Parser) -> PResult<T>) -> PResult<(Vec<T>, Vec<Decl>)> {
    p.need(Punct::LBrace)?;
    let mut out = Vec::new();
    loop {
        p.skip_newlines();
        let a_case = matches!(p.tok(), Tok::Ident(_))
            && matches!(p.nth(1), Tok::Punct(Punct::Comma | Punct::LParen | Punct::RBrace) | Tok::Newline);
        if !a_case {
            break;
        }
        out.push(case(p)?);
        p.eat(Punct::Comma);
    }
    let mut body = Vec::new();
    loop {
        p.skip_newlines();
        if p.eat(Punct::RBrace) {
            return Ok((out, body));
        }
        if *p.tok() == Tok::Eof {
            return Err(p.expected("`}`"));
        }
        body.push(p.decl()?);
        p.end_stmt()?;
    }
}
