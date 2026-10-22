//! `screen` — a screen's model over its `.yui` markup.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::token::Punct;
use crate::words::{Role, Word, ui};

pub static WORD: Word = Word {
    text: "screen",
    doc: "A screen's model over its markup: `screen ChargerScreen(block: Charger) ui \"charger.yui\" { … }`. \
          Its `val`s bind live (`synced` state needs no messages); its `action`s run on the server.",
    reserved: false,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a screen's name")?;
    let params = if p.at(Punct::LParen) { p.params()? } else { Vec::new() };
    let ui = ui::file(p)?;
    Ok(DeclKind::Screen { name, params, ui, body: p.body()? })
}
