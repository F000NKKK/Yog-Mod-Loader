//! A `command "charge" { … }`: each method a way of running it (`/charge
//! fill <target>`), taking who ran it (`Sender`) first, then arguments of
//! the types the game parses (a number, a word or text, a player); what it
//! `requires` refuses the others.

use yog_ids::script_problem;
use yog_script_ids::system::SENDER;

use super::Checker;
use super::model::Type;
use crate::ast::{Decl, DeclKind, Method};

impl Checker<'_, '_, '_> {
    /// A command's ways of running, checked.
    pub(super) fn command(&mut self, body: &[Decl]) {
        for d in body {
            match &d.kind {
                DeclKind::Method(m) => {
                    self.command_method(m);
                    self.decl(d);
                }
                _ => self.report.error(
                    script_problem::TYPE_MISMATCH,
                    d.span,
                    "a command holds the methods it's run by",
                ),
            }
        }
    }

    /// Method `m` of a command: who ran it first, arguments the game parses.
    fn command_method(&mut self, m: &Method) {
        let mut params = m.params.iter();
        let sender = params.next().map(|p| self.type_of(&p.ty));
        if sender != Some(Type::own(SENDER.to_string())) {
            self.report.error(
                script_problem::TYPE_MISMATCH,
                m.name.span,
                format!("a command's method takes who ran it first: `{SENDER} source`"),
            );
        }
        for p in params {
            let t = self.type_of(&p.ty);
            if command_arg(&t).is_none() {
                self.report.error(
                    script_problem::TYPE_MISMATCH,
                    p.ty.span,
                    format!(
                        "a command's argument is a number, a `string` or a `Player`, not a `{t}`"
                    ),
                );
            }
        }
    }
}

/// The game's argument type a command's parameter of type `t` is parsed as.
pub fn command_arg(t: &Type) -> Option<&'static str> {
    use yog_ids::command_arg::{FLOAT, INT, PLAYER, WORD};
    match t {
        Type::I32 => Some(INT),
        Type::F32 | Type::F64 => Some(FLOAT),
        Type::String => Some(WORD),
        Type::Yog(name) if name == crate::yog::player() => Some(PLAYER),
        _ => None,
    }
}
