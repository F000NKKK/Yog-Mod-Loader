//! `event` — what an object announces, as in C#.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "event",
    doc: "What an object announces, as in C#: `public event Action<Player, int> Charged;`. Subscribed to \
          with `+=` (a lambda or a method: `Charged += (p, n) => …;`), left with `-=`, raised with \
          `Charged?.Invoke(player, 3);`. Yog's are its groups': `Player.Join += e => …;`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let ty = p.ty()?;
    let name = p.name("an event's name")?;
    p.end()?;
    Ok(DeclKind::Event { ty, name })
}
