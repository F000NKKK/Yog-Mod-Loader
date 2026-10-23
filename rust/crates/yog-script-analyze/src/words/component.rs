//! `component` — state and behaviour any object can have.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "component",
    doc: "State and behaviour written once, carried by any object that `has` it: \
          `component Charge(Energy capacity) { saved synced Energy Energy = 0; … }`, then \
          `block Charger has Charge(1000) { … }`, `extend Player has Charge(500);`. Its handlers run \
          on the owner's events; a component is a type (`void Push(Charge into)`).",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a component's name")?;
    let header = p.header(true)?;
    let body = p.body()?;
    Ok(DeclKind::Component { name, header, body })
}
