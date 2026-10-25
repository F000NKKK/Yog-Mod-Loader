//! Types as written (`ast::Type`) to types as checked: the built-in ones,
//! the units, the source's own, collections, and by namespace Yog's and
//! the game's (`names`) — whose side must be the code's.

use yog_ids::script_problem;

use super::Checker;
use super::model::{BLOCKS, SECONDS, TICKS, Type};
use crate::ast::{self, TypeKind};
use crate::index::Side;
use crate::source::token::Span;
use crate::words;
use yog_script_ids::system::ROOT;

impl Checker<'_, '_, '_> {
    pub(super) fn type_of(&mut self, t: &ast::Type) -> Type {
        match &t.kind {
            TypeKind::Nullable(inner) => self.type_of(inner).nullable(),
            TypeKind::Array { item, rank } => Type::Array(Box::new(self.type_of(item)), *rank),
            TypeKind::Tuple(items) => {
                Type::Tuple(items.iter().map(|(t, _)| self.type_of(t)).collect())
            }
            // A borrow is typed as what it lends; the borrow checker sees the rest.
            TypeKind::Borrow { ty, .. } => self.type_of(ty),
            TypeKind::Lifetime(_) => Type::Unknown,
            // `A & B`: an object with both; checked as the first.
            TypeKind::Both(all) => all.first().map_or(Type::Unknown, |t| self.type_of(t)),
            TypeKind::Path { path, args } => {
                let args: Vec<Type> = args.iter().map(|a| self.type_of(a)).collect();
                match path.as_slice() {
                    [one] => self.named(&one.text, args, t.span),
                    _ => self.qualified(path, t.span),
                }
            }
        }
    }

    /// A type by its one name.
    fn named(&mut self, name: &str, args: Vec<Type>, at: Span) -> Type {
        if let Some(t) = super::builtin::by_word(name) {
            return t;
        }
        if [TICKS, SECONDS, BLOCKS].contains(&name) {
            return Type::Unit(name.to_string());
        }
        if self.own.units.iter().any(|u| u == name) {
            return Type::Unit(name.to_string());
        }
        if let Some(t) = self.type_param(name) {
            return t;
        }
        // `String` is `string`, `Int32` `i32` — unless the code's own
        // namespaces declare a type of the name.
        if let Some(b) = super::builtin::by_system_name(name)
            && self.own.namespaces.get(name).is_none_or(|ns| *ns == [ROOT])
        {
            return b;
        }
        if self.own.get(name, args.len()).is_some() && self.own_in_reach(name) {
            return Type::Own(name.to_string(), args);
        }
        let t = self.imported(name, at);
        // A `.yns` checked without the game's classes can't know them.
        let blind = self.nanites && self.lookup.is_none();
        if !t.known() && !blind {
            self.report.error(
                script_problem::UNKNOWN_TYPE,
                at,
                format!("no type `{name}` here: is a `using` missing?"),
            );
        }
        t
    }

    /// A game class of `side` used where the code runs on `self.side`.
    pub(super) fn check_side(&mut self, side: Side, name: &str, at: Span) {
        if side == Side::Shared || side == self.side {
            return;
        }
        let (theirs, ours) = (side_word(side), side_word(self.side));
        self.report.error(
            script_problem::WRONG_SIDE,
            at,
            format!("`{name}` exists only on the {theirs}: {ours} code can't reach it (mark the method `{theirs}`)"),
        );
    }
}

fn side_word(side: Side) -> &'static str {
    match side {
        Side::Client => words::client::WORD.text,
        Side::Server => words::server::WORD.text,
        Side::Shared => words::shared::WORD.text,
    }
}
