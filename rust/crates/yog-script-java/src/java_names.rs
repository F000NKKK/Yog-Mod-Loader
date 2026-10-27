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
use yog_script_ids::system::REIFIED;

use yog_script_analyze::builtin_type;

use crate::index::Index;
use crate::types::primitive;

/// A type as its members' names need it: its type parameters, its bases
/// as written, its methods.
pub(crate) struct Shape {
    generics: Vec<String>,
    bases: Vec<Type>,
    methods: Vec<Method>,
    /// Which of its methods are `[Reified]`: handed their type parameters.
    reified: Vec<bool>,
}

/// Each type parameter of a type and what it is where it's seen from.
type Bound = HashMap<String, String>;

/// A type by its name and its arity (`ValueTuple<T1, T2>` is
/// `("ValueTuple", 2)`): those of one name and other arities are others.
pub(crate) type TypeRef = (String, usize);

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
        methods: methods(body).map(|(_, m)| m.clone()).collect(),
        reified: methods(body)
            .map(|(d, _)| crate::library::marked(d, REIFIED))
            .collect(),
    }
}

/// A body's methods, each with its declaration.
fn methods(body: &[Decl]) -> impl Iterator<Item = (&Decl, &Method)> {
    body.iter().filter_map(|d| match &d.kind {
        DeclKind::Method(m) => Some((d, m)),
        _ => None,
    })
}

impl Shape {
    /// Its `overload`-th method named `name` (explicit implementations aside).
    pub(crate) fn method_named(&self, name: &str, overload: usize) -> Option<&Method> {
        self.methods
            .iter()
            .filter(|m| m.explicit.is_none() && m.name.text == name)
            .nth(overload)
    }
}

impl Index {
    /// Whether `owner`'s method `name`, the `overload`-th of its name, is
    /// `[Reified]`: handed its type parameters first.
    pub(crate) fn is_reified(&self, owner: &TypeRef, name: &str, overload: usize) -> bool {
        let Some(shape) = self.shape(owner) else {
            return false;
        };
        shape
            .methods
            .iter()
            .zip(&shape.reified)
            .filter(|(m, _)| m.explicit.is_none() && m.name.text == name)
            .nth(overload)
            .is_some_and(|(_, reified)| *reified)
    }

    /// Which of its name `m` is among `owner`'s methods (explicit
    /// implementations aside), as a call names it.
    pub(crate) fn overload_of(&self, owner: &TypeRef, m: &Method) -> usize {
        self.methods_named(owner, &m.name.text)
            .position(|x| x == m)
            .unwrap_or_default()
    }

    /// The Java name of `owner`'s method `name`, the `overload`-th of its
    /// name (explicit implementations aside), as a call chose it.
    pub(crate) fn java_name_of(
        &self,
        owner: &TypeRef,
        name: &str,
        overload: usize,
    ) -> Option<String> {
        let m = self
            .shape(owner)?
            .methods
            .iter()
            .filter(|m| m.explicit.is_none() && m.name.text == name)
            .nth(overload)?;
        Some(self.java_name(owner, m))
    }

    /// The Java name of method `m` of type `owner`.
    pub(crate) fn java_name(&self, owner: &TypeRef, m: &Method) -> String {
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
            return format!("{}{MEMBER_SEP}{}", owner.0, m.name.text);
        }
        // Overloads of its own type Java can't tell apart (`Sum` of `i32`s,
        // of `f64`s): the first keeps the name, the rest are numbered.
        let earlier = self
            .methods_named(owner, &m.name.text)
            .take_while(|b| *b != m)
            .filter(|b| self.erased(owner, b) == own_erased)
            .count();
        if earlier == 0 {
            m.name.text.clone()
        } else {
            format!("{}{MEMBER_SEP}{earlier}", m.name.text)
        }
    }

    /// The Java name of the method `m` implements among `name`'s (seen
    /// with `bound`) and what it inherits.
    fn implemented(&self, name: &TypeRef, bound: Bound, m: &Method) -> Option<String> {
        self.ancestry(name, bound)
            .iter()
            .find_map(|(base, bound)| self.same_in(base, bound, m))
    }

    /// The Java name of `base`'s method of `m`'s signature (`base`'s types
    /// seen with `bound`), when it has one.
    fn same_in(&self, base: &TypeRef, bound: &Bound, m: &Method) -> Option<String> {
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
        name: &TypeRef,
        method: &'i str,
    ) -> impl Iterator<Item = &'i Method> {
        self.shape(name)
            .into_iter()
            .flat_map(|s| &s.methods)
            .filter(move |m| m.explicit.is_none() && m.name.text == method)
    }

    /// `m`'s parameters (of type `owner`) as Java erases them.
    fn erased(&self, owner: &TypeRef, m: &Method) -> Vec<String> {
        let mut params: Vec<&str> = self
            .shape(owner)
            .map(|s| s.generics.iter().map(String::as_str).collect())
            .unwrap_or_default();
        params.extend(m.generics.iter().map(|g| g.name.text.as_str()));
        m.params.iter().map(|p| erase(&p.ty, &params)).collect()
    }

    /// `name` (its parameters seen as `bound` says) and every type it
    /// inherits, nearest first, each with its parameters seen from `name`.
    fn ancestry(&self, name: &TypeRef, bound: Bound) -> Vec<(TypeRef, Bound)> {
        let mut out = vec![(name.clone(), bound)];
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
fn written_bound(index: &Index, t: &Type, bound: &Bound) -> (TypeRef, Bound) {
    let TypeKind::Path { path, args } = &t.kind else {
        return ((String::new(), 0), Bound::new());
    };
    let name = (
        path.last().map(|n| n.text.clone()).unwrap_or_default(),
        args.len(),
    );
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
            // `System.Collections.Generic.IEnumerator`: the library's, by its name.
            let name = match path.as_slice() {
                [first, .., last] if first.text == yog_script_ids::system::ROOT => last.text.clone(),
                _ => path.iter().map(|n| n.text.as_str()).collect::<Vec<_>>().join("."),
            };
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
        // A generic type by its name and arity: Java's classes differ by
        // it (`Func<T, bool>` is `Func2`, `Func<T, i32, bool>` `Func3`).
        TypeKind::Path { path, args } => {
            let name = path.last().map(|n| n.text.as_str()).unwrap_or_default();
            // `object` is Java's `Object`, as a type parameter erases to.
            let object = builtin_type(name) == Some(yog_script_analyze::Type::Object);
            if args.is_empty() && (params.contains(&name) || object) {
                OBJECT.to_string()
            } else {
                format!("{name}{}", args.len())
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
