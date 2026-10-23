//! Names by namespace, as in C#: the source's own first, then what its
//! `using`s bring in, then `Yog` (every file's, as `System` is in C#).
//! The game's classes are namespace `Game` (a `.yns` file's only): `using
//! Game.World.Entity;` or written in full, `Game.World.Entity.LivingEntity`.
//! Yog's are `Yog.Player` in full, where a `using` brings in a `Player` too
//! (and Yog's `game` group is `Yog.Game` in a `.yns`).

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use super::surface::SURFACE;
use crate::ast::Name;
use crate::index::namespace;
use crate::source::token::Span;

/// What a namespace's segments are joined with.
const SEP: &str = ".";

/// The root namespaces: Yog's surface, and the game's classes.
pub(super) fn yog_root() -> String {
    yog_codegen::pascal(yog_ids::ns::YOG)
}

pub(super) fn game_root() -> String {
    yog_codegen::pascal(yog_ids::bus_word::GAME)
}

impl Checker<'_, '_, '_> {
    /// A name not the source's own: what a `using` brings in, Yog's, or a
    /// root namespace.
    pub(super) fn imported(&mut self, name: &str, at: Span) -> Type {
        let usings = self.usings.clone();
        for using in &usings {
            // A `using` brings in what's in a namespace, not namespaces.
            let found = self.in_namespace(using, name, at);
            if found.known() && !matches!(found, Type::Namespace(_)) {
                return found;
            }
        }
        // In a `.yns`, `Game` is the game's classes (Yog's `game` group is
        // then `Yog.Game`); a `.ys` has no game classes: `Game` is Yog's.
        if self.nanites && name == game_root() {
            return Type::Namespace(name.to_string());
        }
        if SURFACE.knows(name) {
            return Type::Yog(name.to_string());
        }
        if name == yog_root() {
            return Type::Namespace(name.to_string());
        }
        Type::Unknown
    }

    /// `A.B.C` written as a type: the namespace walked from its root.
    pub(super) fn qualified(&mut self, path: &[Name], at: Span) -> Type {
        let Some((first, rest)) = path.split_first() else {
            return Type::Unknown;
        };
        let mut t = self.imported(&first.text, at);
        for n in rest {
            t = match &t {
                Type::Namespace(ns) => self.in_namespace(&split(ns), &n.text, n.span),
                // A nested class, after the class it's in.
                Type::Game(outer) => {
                    let nested = format!("{outer}{}{}", crate::index::NESTED, n.text);
                    match self.lookup.as_ref().and_then(|l| l.class(&nested)) {
                        Some(_) => Type::Game(nested),
                        None => return Type::Unknown,
                    }
                }
                _ => return Type::Unknown,
            };
        }
        t
    }

    /// `name` in namespace `ns`: a class, Yog's type, or a namespace inside it.
    pub(super) fn in_namespace(&mut self, ns: &[String], name: &str, at: Span) -> Type {
        let Some((root, under)) = ns.split_first() else {
            return Type::Unknown;
        };
        if *root == yog_root() && under.is_empty() {
            return if SURFACE.knows(name) {
                Type::Yog(name.to_string())
            } else {
                Type::Unknown
            };
        }
        if *root != game_root() {
            return Type::Unknown;
        }
        if !self.nanites {
            self.report.error(
                script_problem::TARGETS,
                at,
                "the game's classes are a `.yns` file's: a `.ys` reaches Yog (its objects need no `using`)",
            );
            return Type::Unknown;
        }
        let class = namespace::class_of(under, name);
        let Some(found) = self
            .lookup
            .as_ref()
            .and_then(|l| l.class(&class))
            .map(|c| c.side)
        else {
            let mut deeper = ns.to_vec();
            deeper.push(name.to_string());
            return Type::Namespace(deeper.join(SEP));
        };
        self.check_side(found, name, at);
        Type::Game(class)
    }
}

/// `Game.World` → `["Game", "World"]`.
pub(super) fn split(ns: &str) -> Vec<String> {
    ns.split(SEP).map(String::from).collect()
}
