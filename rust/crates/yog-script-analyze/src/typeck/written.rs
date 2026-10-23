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

/// The library's collections, by the names a source writes.
pub(super) const LIST: &str = "List";
pub(super) const SET: &str = "HashSet";
pub(super) const MAP: &str = "Dictionary";
/// Delegates, as C#'s: `Action<A, B>` gives nothing, `Func<A, R>` an `R`.
pub(super) const ACTION: &str = "Action";
pub(super) const FUNC: &str = "Func";

/// A plain one-name type's name (`Player`, not `List<i32>` or `a.B`).
pub(crate) fn simple_path(ty: &ast::Type) -> Option<&str> {
    match &ty.kind {
        TypeKind::Path { path, args } if args.is_empty() && path.len() == 1 => Some(&path[0].text),
        _ => None,
    }
}

impl Checker<'_, '_, '_> {
    pub(super) fn type_of(&mut self, t: &ast::Type) -> Type {
        match &t.kind {
            TypeKind::Nullable(inner) => self.type_of(inner).nullable(),
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
    fn named(&mut self, name: &str, mut args: Vec<Type>, at: Span) -> Type {
        if let Some(t) = super::builtin::by_word(name) {
            return t;
        }
        match name {
            LIST | SET => return Type::List(Box::new(args.pop().unwrap_or(Type::Unknown))),
            MAP if args.len() == 2 => {
                let value = args.pop().unwrap_or(Type::Unknown);
                let key = args.pop().unwrap_or(Type::Unknown);
                return Type::Map(Box::new(key), Box::new(value));
            }
            TICKS | SECONDS | BLOCKS => return Type::Unit(name.to_string()),
            ACTION => return Type::Fn(args, Box::new(Type::Void)),
            FUNC if !args.is_empty() => {
                let ret = args.pop().unwrap_or(Type::Unknown);
                return Type::Fn(args, Box::new(ret));
            }
            _ => {}
        }
        if self.own.units.iter().any(|u| u == name) {
            return Type::Unit(name.to_string());
        }
        if let Some(t) = self.type_param(name) {
            return t;
        }
        if self.own.types.contains_key(name) {
            return Type::Own(name.to_string(), args);
        }
        self.imported(name, at)
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
