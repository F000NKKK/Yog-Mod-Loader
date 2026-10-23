//! `screen` — a view model over `.yui` markup.

use crate::ast::{DeclKind, TypeHeader, TypeKind};
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "screen",
    doc: "A view model over `.yui` markup: `screen ChargerScreen(Charger block) : Ui(\"charger.yui\") { … }`. \
          Bindings to `synced` state are live; its `server` methods are actions a client calls.",
    reserved: false,
    roles: &[Role::Decl { starts: Some(starts), parse }],
};

/// The markup's base: `: Ui("file.yui")`.
const UI: &str = "Ui";

fn starts(p: &Parser) -> bool {
    p.at_word(&WORD) && matches!(p.nth(1), Tok::Ident(_)) && matches!(p.nth(2), Tok::Punct(Punct::LParen | Punct::Colon))
}

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a screen's name")?;
    let mut header = p.header(true)?;
    let ui = ui_file(p, &mut header)?;
    let body = p.body()?;
    Ok(DeclKind::Screen { name, header, ui, body })
}

/// The `Ui("…")` base the header read, as its file; a missing one reported.
fn ui_file(p: &mut Parser, header: &mut TypeHeader) -> PResult<String> {
    let at = header.bases.iter().position(|b| {
        matches!(&b.kind, TypeKind::Path { path, args } if args.is_empty() && path.len() == 1 && path[0].text == UI)
    });
    match at {
        Some(i) if p.at(Punct::LParen) => {
            header.bases.remove(i);
            p.bump();
            let file = p.plain_string("the markup's file")?;
            p.need(Punct::RParen)?;
            Ok(file)
        }
        _ => Err(p.expected("`: Ui(\"file.yui\")`")),
    }
}
