//! Each method's name in Java. Mostly its own; but Java tells methods apart
//! by their parameters once generics are erased, and its interfaces can't
//! hide a member as C#'s do — so a method Java couldn't tell from one it
//! inherits (`ISet<T>`'s `new bool Add(T)` over `ICollection<T>`'s
//! `void Add(T)`, `IDictionary<K, V>`'s `Remove(K)` over
//! `ICollection<KeyValuePair<K, V>>`'s `Remove(T)`) is named after its type
//! too (`ISet$Add`). What implements or overrides a method takes that
//! method's name, an explicit implementation too.

use std::collections::HashMap;

use yog_script_analyze::ast::{Decl, DeclKind, Method, Type, TypeHeader, TypeKind};
use yog_script_ids::jvm::{MEMBER_SEP, OBJECT};

use yog_script_analyze::builtin_type;

use crate::index::Index;
use crate::types::primitive;

/// A type as its members' names need it: its type parameters, its bases
/// as written, its methods.
pub(crate) struct Shape {
    generics: Vec<String>,
    bases: Vec<Type>,
    methods: Vec<Method>,
}

/// Each type parameter of a type and what it is where it's seen from.
type Bound = HashMap<String, String>;

/// Type `header { body }`'s shape.
pub(crate) fn shape_of(header: &TypeHeader, body: &[Decl]) -> Shape {
    Shape {
        generics: header
            .generics
            .iter()
            .filter(|g| !g.lifetime)
            .map(|g| g.name.text.clone())
            .collect(),
        bases: header.bases.clone(),
        methods: body
            .iter()
            .filter_map(|d| match &d.kind {
                DeclKind::Method(m) => Some(m.clone()),
                _ => None,
            })
            .collect(),
    }
}

impl Index {
    /// The Java name of `owner`'s method `name`, the `overload`-th of its
    /// name (explicit implementations aside), as a call chose it.
    pub(crate) fn java_name_of(&self, owner: &str, name: &str, overload: usize) -> Option<String> {
        let m = self
            .shape(owner)?
            .methods
            .iter()
            .filter(|m| m.explicit.is_none() && m.name.text == name)
            .nth(overload)?;
        Some(self.java_name(owner, m))
    }

    /// The Java name of method `m` of type `owner`.
    pub(crate) fn java_name(&self, owner: &str, m: &Method) -> String {
        if let Some(interface) = &m.explicit {
            let (name, bound) = written_bound(self, interface, &Bound::new());
            return self
                .implemented(&name, bound, m)
                .unwrap_or_else(|| m.name.text.clone());
        }
        let ancestry = self.ancestry(owner, Bound::new());
        if let Some(name) = ancestry
            .iter()
            .skip(1)
            .find_map(|(base, bound)| self.same_in(base, bound, m))
        {
            return name;
        }
        let own_erased = self.erased(owner, m);
        let clashes = ancestry.iter().skip(1).any(|(base, _)| {
            self.methods_named(base, &m.name.text)
                .any(|b| self.erased(base, b) == own_erased)
        });
        if clashes {
            format!("{owner}{MEMBER_SEP}{}", m.name.text)
        } else {
            m.name.text.clone()
        }
    }

    /// The Java name of the method `m` implements among `name`'s (seen
    /// with `bound`) and what it inherits.
    fn implemented(&self, name: &str, bound: Bound, m: &Method) -> Option<String> {
        self.ancestry(name, bound)
            .iter()
            .find_map(|(base, bound)| self.same_in(base, bound, m))
    }

    /// The Java name of `base`'s method of `m`'s signature (`base`'s types
    /// seen with `bound`), when it has one.
    fn same_in(&self, base: &str, bound: &Bound, m: &Method) -> Option<String> {
        let same = |b: &Method| {
            b.params.len() == m.params.len()
                && b.params
                    .iter()
                    .zip(&m.params)
                    .all(|(x, y)| render(&x.ty, bound) == render(&y.ty, &Bound::new()))
                && b.ret.as_ref().map(|t| render(t, bound))
                    == m.ret.as_ref().map(|t| render(t, &Bound::new()))
        };
        let found = self.methods_named(base, &m.name.text).find(|b| same(b))?;
        Some(self.java_name(base, found))
    }

    /// `name`'s own methods named `method` (explicit implementations aside).
    fn methods_named<'i>(
        &'i self,
        name: &str,
        method: &'i str,
    ) -> impl Iterator<Item = &'i Method> {
        self.shape(name)
            .into_iter()
            .flat_map(|s| &s.methods)
            .filter(move |m| m.explicit.is_none() && m.name.text == method)
    }

    /// `m`'s parameters (of type `owner`) as Java erases them.
    fn erased(&self, owner: &str, m: &Method) -> Vec<String> {
        let mut params: Vec<&str> = self
            .shape(owner)
            .map(|s| s.generics.iter().map(String::as_str).collect())
            .unwrap_or_default();
        params.extend(m.generics.iter().map(|g| g.name.text.as_str()));
        m.params.iter().map(|p| erase(&p.ty, &params)).collect()
    }

    /// `name` (its parameters seen as `bound` says) and every type it
    /// inherits, nearest first, each with its parameters seen from `name`.
    fn ancestry(&self, name: &str, bound: Bound) -> Vec<(String, Bound)> {
        let mut out = vec![(name.to_string(), bound)];
        let mut i = 0;
        while let Some((current, bound)) = out.get(i).cloned() {
            for b in self
                .shape(&current)
                .map(|s| s.bases.as_slice())
                .unwrap_or_default()
            {
                let (base, seen) = written_bound(self, b, &bound);
                if !out.iter().any(|(o, _)| *o == base) {
                    out.push((base, seen));
                }
            }
            i += 1;
        }
        out
    }
}

/// Base `t` as written where `bound` holds: its name, its parameters bound
/// to its arguments.
fn written_bound(index: &Index, t: &Type, bound: &Bound) -> (String, Bound) {
    let TypeKind::Path { path, args } = &t.kind else {
        return (String::new(), Bound::new());
    };
    let name = path.last().map(|n| n.text.clone()).unwrap_or_default();
    let generics = index
        .shape(&name)
        .map(|s| s.generics.clone())
        .unwrap_or_default();
    let seen = generics
        .into_iter()
        .zip(args.iter().map(|a| render(a, bound)))
        .collect();
    (name, seen)
}

/// `t` written out, its type parameters as `bound` sees them.
fn render(t: &Type, bound: &Bound) -> String {
    match &t.kind {
        TypeKind::Path { path, args } => {
            let name: Vec<&str> = path.iter().map(|n| n.text.as_str()).collect();
            let name = name.join(".");
            if args.is_empty() {
                return bound.get(&name).cloned().unwrap_or(name);
            }
            let args: Vec<String> = args.iter().map(|a| render(a, bound)).collect();
            format!("{name}<{}>", args.join(","))
        }
        TypeKind::Nullable(inner) => format!("{}?", render(inner, bound)),
        TypeKind::Array { item, rank } => format!("{}[{}]", render(item, bound), rank),
        TypeKind::Borrow { ty, .. } => render(ty, bound),
        TypeKind::Tuple(items) => {
            let items: Vec<String> = items.iter().map(|(t, _)| render(t, bound)).collect();
            format!("({})", items.join(","))
        }
        TypeKind::Both(all) => all.first().map(|t| render(t, bound)).unwrap_or_default(),
        TypeKind::Lifetime(n) => n.text.clone(),
    }
}

/// `t` as Java erases it: a type parameter (of `params`) its bound,
/// `Object`; a generic type without its arguments.
fn erase(t: &Type, params: &[&str]) -> String {
    match &t.kind {
        TypeKind::Path { path, args } => {
            let name = path.last().map(|n| n.text.as_str()).unwrap_or_default();
            if args.is_empty() && params.contains(&name) {
                OBJECT.to_string()
            } else {
                name.to_string()
            }
        }
        // A number's `?` boxes it in Java; anything else is as it is.
        TypeKind::Nullable(inner) => {
            let erased = erase(inner, params);
            match builtin_type(&erased).as_ref().and_then(primitive) {
                Some(_) => format!("{erased}?"),
                None => erased,
            }
        }
        TypeKind::Array { item, rank } => format!("{}[{}]", erase(item, params), rank),
        TypeKind::Borrow { ty, .. } => erase(ty, params),
        _ => OBJECT.to_string(),
    }
}
