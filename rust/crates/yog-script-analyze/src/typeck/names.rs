//! Names by namespace, as in C#. A simple name is, in turn: a type of the
//! namespace the code is in (or one enclosing it); a `using` alias; a type
//! one of the `using`s brings in — `System` and `System.Yog` among them,
//! every file's, as .NET's implicit usings — where two bringing one in is
//! an ambiguity; a namespace. Everything starts with `System`: Yog's
//! objects are `System.Yog`'s, the game's classes `System.Game`'s — a
//! `.yns` file's only (`using System.Game.World.Entity;`).

use std::collections::HashMap;

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use super::surface::SURFACE;
use crate::ast::{self, Decl, DeclKind, Name};
use crate::index::namespace;
use crate::source::token::Span;
use yog_script_ids::system::{COLLECTIONS_GENERIC, LINQ, ROOT, TEXT};

/// What a namespace's segments are joined with.
const SEP: &str = yog_script_ids::system::NAMESPACE_SEP;

/// `System.Yog`: Yog's objects, groups and events.
pub(super) fn yog_ns() -> Vec<String> {
    vec![ROOT.to_string(), yog_codegen::pascal(yog_ids::ns::YOG)]
}

/// `System.Game`: the game's classes (a `.yns`'s).
pub(super) fn game_ns() -> Vec<String> {
    vec![
        ROOT.to_string(),
        yog_codegen::pascal(yog_ids::bus_word::GAME),
    ]
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
            usings: vec![
                vec![ROOT.to_string()],
                COLLECTIONS_GENERIC.map(String::from).to_vec(),
                LINQ.map(String::from).to_vec(),
                TEXT.map(String::from).to_vec(),
                yog_ns(),
            ],
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
        self.ns.starts_with(ns).then(|| Type::own(name.to_string()))
    }

    /// Whether own type `name` is reached by its name alone here: its
    /// namespace is the code's, one enclosing it, or one a `using` brings in.
    pub(super) fn own_in_reach(&self, name: &str) -> bool {
        self.own
            .namespaces
            .get(name)
            .is_none_or(|ns| self.ns.starts_with(ns) || self.directives.usings.contains(ns))
    }

    /// A namespace `name` starts: Yog's, the game's (a `.yns`), or the source's.
    fn root(&self, name: &str) -> Type {
        let own_root = self
            .own
            .namespaces
            .values()
            .any(|ns| ns.first().is_some_and(|f| f == name));
        if name == ROOT || own_root {
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
        // `System` is the root before anything else.
        let mut t = if first.text == ROOT {
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
        if self.own.namespaces.get(name).is_some_and(|own| own == ns) {
            return Type::own(name.to_string());
        }
        if ns == yog_ns().as_slice() && SURFACE.knows(name) {
            return Type::Yog(name.to_string());
        }
        if ns.starts_with(&game_ns()) {
            return self.in_game(ns, name, at);
        }
        let deeper: Vec<String> = [ns, &[name.to_string()]].concat();
        let is_own_ns = self
            .own
            .namespaces
            .values()
            .any(|own| own.starts_with(&deeper));
        if is_own_ns || deeper == yog_ns() || deeper == game_ns() {
            Type::Namespace(deeper.join(SEP))
        } else {
            Type::Unknown
        }
    }

    /// `name` in the game's namespace `ns` (`System.Game`, `System.Game.World`, …).
    fn in_game(&mut self, ns: &[String], name: &str, at: Span) -> Type {
        if !self.nanites {
            self.report.error(
                script_problem::TARGETS,
                at,
                "the game's classes are a `.yns` file's: a `.ys` reaches Yog (its objects need no `using`)",
            );
            return Type::Unknown;
        }
        let class = namespace::class_of(&ns[game_ns().len()..], name);
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

/// `System.Game.World` → `["System", "Game", "World"]`.
pub(super) fn split(ns: &str) -> Vec<String> {
    ns.split(SEP).map(String::from).collect()
}
