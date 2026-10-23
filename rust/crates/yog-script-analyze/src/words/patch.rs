//! `patch` — code reaching into a class the mod doesn't own.

use yog_ids::script_problem;

use crate::ast::{DeclKind, Patch};
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "patch",
    doc: "Reaches into a class the mod doesn't own (`.yns` files only): `patch LivingEntity { before \
          bool Hurt(DamageSource source, float amount) { … } }`. Methods and points by their meaning, \
          checked against every targeted version.",
    reserved: false,
    roles: &[Role::Decl { starts: Some(starts), parse }],
};

fn starts(p: &Parser) -> bool {
    p.at_word(&WORD) && matches!(p.nth(1), Tok::Ident(_)) && !p.nth_is(1, Punct::Eq)
        && !matches!(p.nth(2), Tok::Punct(Punct::Eq | Punct::Semi | Punct::LParen))
}

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    let start = p.bump().span;
    if !p.nanites {
        p.error(
            script_problem::PATCH_OUTSIDE_NANITES,
            start,
            "patches go in `.yns` files (Yog Nanite Script)",
        );
    }
    let target = p.ty()?;
    let items = p.patch_items()?;
    Ok(DeclKind::Patch(Patch { target, items }))
}
