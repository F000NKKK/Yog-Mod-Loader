//! `patch` — reaching into a class the mod doesn't own (`.yns` only).

use yog_ids::script_problem;

use crate::ast::{DeclKind, Patch};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "patch",
    doc: "Reaches into a class the mod doesn't own, the game's or a Java mod's (`.yns` files): \
          `patch LivingEntity { before hurt(…) { … } }`. Every point is checked against every \
          targeted version when the mod is built.",
    reserved: false,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    let at = p.bump().span;
    if !p.nanites {
        p.error(script_problem::PATCH_OUTSIDE_NANITES, at, "a patch goes in a `.yns` file");
    }
    let target = p.ty_plain()?;
    let items = p.patch_items()?;
    Ok(DeclKind::Patch(Patch { target, items }))
}
