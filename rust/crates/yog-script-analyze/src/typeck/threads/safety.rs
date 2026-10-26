//! Whether a value may cross threads (`ys-concurrency.md` §2), inferred
//! from its type as Rust's auto traits are: sendable (moved to another
//! thread) and shareable (reached by several at once). A type's
//! `[Threads(…)]` says it where its fields don't (the runtime's); else its
//! stored fields do, and a shareable one is `readonly` too.

use std::collections::HashSet;

use yog_script_ids::system::threads::{BOUND, GUARDED, SHARED};

use super::super::Checker;
use super::super::model::Type;
use super::super::own::TypeKey;

/// What crossing asks of a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Need {
    Send,
    Share,
}

/// Why a value can't cross: the field holding what can't (`Cache.Hits`;
/// none when it's the value's own type), what that is and why.
pub(super) struct Blame {
    pub path: Vec<String>,
    pub culprit: Type,
    pub why: &'static str,
}

const CHANGEABLE: &str =
    "changeable, so one thread at a time: guard it (`Lock<T>`, `Atomic<T>`) or make it `readonly`";
const GAME: &str = "the game's, reached on its thread only";
const HELD: &str = "held by the thread that made it";

impl Blame {
    fn of(t: &Type, why: &'static str) -> Self {
        Blame {
            path: Vec::new(),
            culprit: t.clone(),
            why,
        }
    }

    /// Found in field `field` of type `owner`.
    fn under(mut self, owner: &str, field: &str) -> Self {
        match self.path.first_mut() {
            Some(root) => *root = field.to_string(),
            None => self.path.push(field.to_string()),
        }
        self.path.insert(0, owner.to_string());
        self
    }
}

impl Checker<'_, '_, '_> {
    /// Why a `t` can't cross as `need` asks, if it can't.
    pub(super) fn blame(&mut self, t: &Type, need: Need) -> Option<Blame> {
        self.blame_in(t, need, &mut HashSet::new())
    }

    fn blame_in(
        &mut self,
        t: &Type,
        need: Need,
        seen: &mut HashSet<(String, Need)>,
    ) -> Option<Blame> {
        match t {
            Type::Nullable(inner) => self.blame_in(inner, need, seen),
            Type::Array(_, _) if need == Need::Share => Some(Blame::of(t, CHANGEABLE)),
            Type::Array(item, _) => self.blame_in(item, need, seen),
            Type::Tuple(items, _) => items.iter().find_map(|i| self.blame_in(i, need, seen)),
            Type::Game(_) | Type::Yog(_) => Some(Blame::of(t, GAME)),
            Type::Own(name, args) if seen.insert((t.to_string(), need)) => {
                self.own_blame(t, (name, args), need, seen)
            }
            _ => None,
        }
    }

    /// Own type `t` (`name<args>`): by its `[Threads(…)]`, else its fields.
    fn own_blame(
        &mut self,
        t: &Type,
        (name, args): (&str, &[Type]),
        need: Need,
        seen: &mut HashSet<(String, Need)>,
    ) -> Option<Blame> {
        let own = self.own.of_key(&TypeKey::new(name, args.len()))?;
        if own.delegate.is_some() || own.interface || own.is_enum {
            return None;
        }
        let (stored, readonly) = (own.stored.clone(), own.readonly_all);
        let each_arg = |c: &mut Self, need, seen: &mut HashSet<_>| {
            args.iter().find_map(|a| c.blame_in(a, need, seen))
        };
        match own.threads.as_deref() {
            Some(BOUND) => return Some(Blame::of(t, HELD)),
            Some(GUARDED) => return each_arg(self, Need::Send, seen),
            Some(SHARED) => return each_arg(self, Need::Share, seen),
            _ => {}
        }
        if need == Need::Share && !readonly {
            return Some(Blame::of(t, CHANGEABLE));
        }
        for f in &stored {
            let ft = self.own_member(t, f);
            if let Some(b) = self.blame_in(&ft, need, seen) {
                return Some(b.under(name, f));
            }
        }
        each_arg(self, need, seen)
    }
}
