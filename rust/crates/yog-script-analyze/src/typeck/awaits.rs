//! `async` and `await`, as C#'s: an `async` method gives a `Task` (its
//! `return` gives nothing), a `Task<T>` (its `return` gives a `T`) or
//! nothing (`async void`, a handler's); `await t` is what `t` finishes
//! with — a `Task<T>`'s `T`, a `Task`'s nothing.

use yog_ids::script_problem;
use yog_script_ids::system::TASK;

use super::Checker;
use super::model::Type;
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// What an `async` method declared to give `declared` returns inside.
    pub(super) fn async_returns(&mut self, declared: &Type, at: Span) -> Type {
        match declared {
            Type::Void | Type::Unknown => Type::Void,
            t => {
                match awaited(t) {
                    Some(inside) => inside,
                    None => {
                        self.report.error(
                        script_problem::TYPE_MISMATCH,
                        at,
                        format!("an `async` method gives a `Task`, a `Task<T>` or nothing, not a `{t}`"),
                    );
                        Type::Unknown
                    }
                }
            }
        }
    }

    /// `await` of a `t`: what it finishes with.
    pub(super) fn awaits(&mut self, t: &Type, at: Span) -> Type {
        if !t.known() {
            return Type::Unknown;
        }
        awaited(t).unwrap_or_else(|| {
            self.report.error(
                script_problem::TYPE_MISMATCH,
                at,
                format!("`await` waits for a `Task`, not a `{t}`"),
            );
            Type::Unknown
        })
    }
}

/// What task type `t` finishes with: a `Task`'s nothing, a `Task<T>`'s `T`.
fn awaited(t: &Type) -> Option<Type> {
    match t.non_null() {
        Type::Own(name, args) if name == TASK => match args.as_slice() {
            [] => Some(Type::Void),
            [inside] => Some(inside.clone()),
            _ => None,
        },
        _ => None,
    }
}
