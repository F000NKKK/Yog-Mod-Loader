//! Names by namespace, as in C#. A simple name is, in turn: a type of the
//! namespace the code is in (or one enclosing it); a `using` alias; a type
//! one of the `using`s brings in — `Yog` among them, every file's, as .NET's
//! implicit usings — where two bringing one in is an ambiguity; a namespace.
//! The game's classes are namespace `Game`, a `.yns` file's only
//! (`using Game.World.Entity;`); there Yog's `game` group is `Yog.Game`.

use std::collections::HashMap;

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use super::surface::SURFACE;
use crate::ast::{self, Decl, DeclKind, Name};
use crate::index::namespace;
use crate::source::token::Span;

/// What a namespace's segments are joined with.
const SEP: &str = ".";

pub(super) fn yog_root() -> String {
    yog_codegen::pascal(yog_ids::ns::YOG)
}

pub(super) fn game_root() -> String {
    yog_codegen::pascal(yog_ids::bus_word::GAME)
}

/// A file's `using` directives (its `global` ones too: a file is checked
/// on its own here).
#[derive(Default)]
pub(super) struct Directives {
    pub usings: Vec<Vec<String>>,
    pub statics: Vec<Vec<String>>,
    pub aliases: HashMap<String, ast::Type>,
}

impl Directives {
    pub fn collect(decls: &[Decl]) -> Self {
        let mut out = Directives {
            usings: vec![vec![yog_root()]],
            ..Directives::default()
        };
        out.add(decls);
        out
    }

    fn add(&mut self, decls: &[Decl]) {
        for d in decls {
            match &d.kind {
                DeclKind::Using { path, statics, .. } => {
                    let path = path.iter().map(|n| n.text.clone()).collect();
                    if *statics {
                        self.statics.push(path)
                    } else {
                        self.usings.push(path)
                    }
                }
                DeclKind::Alias { name, ty } => {
                    self.aliases.insert(name.text.clone(), ty.clone());
                }
                DeclKind::Namespace {
                    body: Some(body), ..
                } => self.add(body),
                _ => {}
            }
        }
    }
}

impl Checker<'_, '_, '_> {
    /// A simple name that isn't a local or a built-in type.
    pub(super) fn imported(&mut self, name: &str, at: Span) -> Type {
        if let Some(t) = self.own_visible(name) {
            return t;
        }
        if let Some(alias) = self.directives.aliases.get(name).cloned() {
            return self.type_of(&alias);
        }
        let usings = self.directives.usings.clone();
        let mut found: Vec<(String, Type)> = Vec::new();
        for using in &usings {
            let t = self.in_namespace(using, name, at);
            if t.known() && !matches!(t, Type::Namespace(_)) && !found.iter().any(|(_, f)| *f == t)
            {
                found.push((using.join(SEP), t));
            }
        }
        match found.as_slice() {
            [] => self.root(name),
            [(_, one)] => one.clone(),
            [(a, first), (b, _), ..] => {
                self.report.error(
                    script_problem::AMBIGUOUS,
                    at,
                    format!("`{name}` is both `{a}.{name}` and `{b}.{name}`: write it in full, or alias one (`using {name} = {a}.{name};`)"),
                );
                first.clone()
            }
        }
    }

    /// The source's own type `name`, when its namespace is the code's or
    /// one enclosing it.
    fn own_visible(&self, name: &str) -> Option<Type> {
        let ns = self.own.namespaces.get(name)?;
        self.ns.starts_with(ns).then(|| Type::Own(name.to_string()))
    }

    /// A namespace `name` starts: Yog's, the game's (a `.yns`), or the source's.
    fn root(&self, name: &str) -> Type {
        let own_root = self
            .own
            .namespaces
            .values()
            .any(|ns| ns.first().is_some_and(|f| f == name));
        let game = self.nanites && name == game_root();
        if game || name == yog_root() || own_root {
            Type::Namespace(name.to_string())
        } else {
            Type::Unknown
        }
    }

    /// `A.B.C` written as a type: walked from its first name.
    pub(super) fn qualified(&mut self, path: &[Name], at: Span) -> Type {
        let Some((first, rest)) = path.split_first() else {
            return Type::Unknown;
        };
        // In a `.yns`, `Game` and `Yog` are the roots before anything else.
        let mut t = if first.text == game_root() && self.nanites || first.text == yog_root() {
            Type::Namespace(first.text.clone())
        } else {
            self.imported(&first.text, at)
        };
        for n in rest {
            t = match &t {
                Type::Namespace(ns) => self.in_namespace(&split(ns), &n.text, n.span),
                // Yog's type of a group (`Player.JoinArgs`, `Client.Control`).
                Type::Yog(group) => {
                    let inner = super::surface::subgroup(group, &n.text);
                    if !SURFACE.knows(&inner) {
                        return Type::Unknown;
                    }
                    Type::Yog(inner)
                }
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

    /// `name` in namespace `ns`: a type, or a namespace inside it.
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
        if *root == game_root() && (self.nanites || !SURFACE.knows(root)) {
            return self.in_game(ns, name, at);
        }
        if self.own.namespaces.get(name).is_some_and(|own| own == ns) {
            return Type::Own(name.to_string());
        }
        let deeper: Vec<String> = [ns, &[name.to_string()]].concat();
        let is_own_ns = self
            .own
            .namespaces
            .values()
            .any(|own| own.starts_with(&deeper));
        if is_own_ns {
            Type::Namespace(deeper.join(SEP))
        } else {
            Type::Unknown
        }
    }

    /// `name` in the game's namespace `ns` (`Game`, `Game.World`, …).
    fn in_game(&mut self, ns: &[String], name: &str, at: Span) -> Type {
        if !self.nanites {
            self.report.error(
                script_problem::TARGETS,
                at,
                "the game's classes are a `.yns` file's: a `.ys` reaches Yog (its objects need no `using`)",
            );
            return Type::Unknown;
        }
        let class = namespace::class_of(&ns[1..], name);
        match self
            .lookup
            .as_ref()
            .and_then(|l| l.class(&class))
            .map(|c| c.side)
        {
            Some(side) => {
                self.check_side(side, name, at);
                Type::Game(class)
            }
            None => Type::Namespace([ns, &[name.to_string()]].concat().join(SEP)),
        }
    }
}

/// `Game.World` → `["Game", "World"]`.
pub(super) fn split(ns: &str) -> Vec<String> {
    ns.split(SEP).map(String::from).collect()
}
