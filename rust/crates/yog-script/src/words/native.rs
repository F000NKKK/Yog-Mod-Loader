//! `native` — functions of the mod's Rust library.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word, r#fn};

pub static WORD: Word = Word {
    text: "native",
    doc: "Functions of the mod's own Rust library, called with these signatures: \
          `native \"charge_grid\" { fn simulate(nodes: List<Int>) -> List<Int> }`. Optional: a mod needs no Rust.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let library = p.plain_string("the library's name")?;
    p.need(Punct::LBrace)?;
    let mut fns = Vec::new();
    loop {
        p.skip_newlines();
        if p.eat(Punct::RBrace) {
            return Ok(DeclKind::Native { library, fns });
        }
        if *p.tok() == Tok::Eof {
            return Err(p.expected("`}`"));
        }
        p.need_word(&r#fn::WORD)?;
        fns.push(r#fn::rest(p)?);
        p.end_stmt()?;
    }
}
