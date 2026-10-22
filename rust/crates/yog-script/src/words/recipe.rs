//! `recipe` — how something is made.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "recipe",
    doc: "How something is made: `recipe Battery shaped { \"RIR\" \"IGI\" \"RIR\"  R = @item:minecraft:redstone … }`. \
          The kinds are open: `shaped`, `shapeless`, smelting, or a mod's own.",
    reserved: false,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let result = p.ty_plain()?;
    let kind = p.name("a recipe kind (`shaped`)")?;
    p.need(Punct::LBrace)?;
    let mut rows = Vec::new();
    let mut keys = Vec::new();
    loop {
        p.skip_newlines();
        match p.tok() {
            Tok::Punct(Punct::RBrace) => {
                p.bump();
                return Ok(DeclKind::Recipe { result, kind, rows, keys });
            }
            Tok::Str(_) => rows.push(p.plain_string("a row")?),
            Tok::Eof => return Err(p.expected("`}`")),
            _ => keys.push(p.prop()?),
        }
        p.end_stmt()?;
    }
}
