//! `native` — functions of the mod's Rust library.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, void};

pub static WORD: Word = Word {
    text: "native",
    doc: "Functions of the mod's own Rust library, by their signatures: \
          `native \"charge_grid\" { List<int> Simulate(List<int> nodes); }`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let library = p.plain_string("a library's name")?;
    p.need(Punct::LBrace)?;
    let mut methods = Vec::new();
    while !p.eat(Punct::RBrace) {
        let ret = if p.eat_word(&void::WORD) { None } else { Some(p.ty()?) };
        let name = p.name("a function's name")?;
        methods.push(p.method_rest(ret, name)?);
    }
    Ok(DeclKind::Native { library, methods })
}
