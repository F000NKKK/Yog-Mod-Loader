//! The types a source declares (and the library), as the checker looks
//! members up in them: fields and properties by type, methods by
//! signature, indexers, the components an object has, an enum's cases.
//! A type is its name and its arity, as C#'s: `List` and `List<T>`,
//! `Action<T>` and `Action<T1, T2>` are types apart (the CLR's `List`1`).

mod collect;
mod members;

use std::collections::HashMap;

use crate::ast::{self, Method};

/// A declared type's identity: its name and how many type parameters it takes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TypeKey {
    pub name: String,
    pub arity: usize,
}

impl TypeKey {
    pub fn new(name: impl Into<String>, arity: usize) -> Self {
        TypeKey {
            name: name.into(),
            arity,
        }
    }

    /// The type a written type names (`IList<T>` is `IList` of one): its
    /// last name and how many arguments it's given.
    pub fn of_written(t: &ast::Type) -> Option<Self> {
        match &t.kind {
            ast::TypeKind::Path { path, args } => {
                Some(TypeKey::new(path.last()?.text.clone(), args.len()))
            }
            _ => None,
        }
    }
}

/// A declared type's members, still as written (typed when looked up).
#[derive(Default)]
pub(crate) struct OwnType {
    pub values: HashMap<String, ast::Type>,
    pub methods: HashMap<String, Vec<Method>>,
    /// The components an object of it has (`has Charge(…)`), by name.
    pub components: Vec<String>,
    pub cases: Vec<String>,
    /// What it inherits and implements, as written (`: Machine, IList<T>`).
    pub bases: Vec<ast::Type>,
    /// An `interface`: what classes implement, never one's base class.
    pub interface: bool,
    /// An `abstract class`: inherited, never made.
    pub is_abstract: bool,
    /// Its objects live on the heap (a class, a `record`, an interface):
    /// never `Copy`.
    pub heap: bool,
    /// Its `readonly` methods (Rust's `&self`): the others change it.
    pub readonly: Vec<String>,
    /// A `readonly struct` or a record: every member is.
    pub readonly_all: bool,
    /// A record: set once, never changed; shared, so `Copy`.
    pub record: bool,
    /// Its primary constructor's parameters (`class Obsolete(string message)`).
    pub ctor: Vec<ast::Param>,
    /// Its type parameters' names (`T` of `Box<T>`).
    pub generics: Vec<String>,
    /// An enum's cases that carry data, their fields by case.
    pub variants: HashMap<String, Vec<ast::Param>>,
    /// Its indexers (`this[i32 i]`), still as written.
    pub indexers: Vec<Indexer>,
    /// A delegate: its signature, as written (what `Invoke` takes and gives).
    pub delegate: Option<Method>,
    /// The operators and conversions it declares.
    pub operators: Vec<ast::Operator>,
}

/// An indexer as written: its getter (`get_Item`, what `x[i]` is typed as
/// a call of), and whether it's set too (`x[i] = v`).
#[derive(Clone)]
pub(crate) struct Indexer {
    pub get: Method,
    pub settable: bool,
}

/// Every type the source declares, by name and arity; the units it declares.
#[derive(Default)]
pub(crate) struct OwnTypes {
    types: HashMap<TypeKey, OwnType>,
    pub units: Vec<String>,
    /// The file's own methods (outside types), by name.
    pub methods: HashMap<String, Vec<Method>>,
    pub values: HashMap<String, ast::Type>,
    /// What `extend` adds to the game's types, by their names.
    pub extended: HashMap<String, OwnType>,
    /// Each type's namespace (`namespace A.B;`), by its name.
    pub namespaces: HashMap<String, Vec<String>>,
    /// The namespace the declarations being collected are in.
    current: Vec<String>,
}

impl OwnTypes {
    /// The type `name` of `arity` type parameters.
    pub fn get(&self, name: &str, arity: usize) -> Option<&OwnType> {
        self.types.get(&TypeKey::new(name, arity))
    }

    /// The type `key` names.
    pub fn of_key(&self, key: &TypeKey) -> Option<&OwnType> {
        self.types.get(key)
    }

    /// A type named without its arguments (`Math`, a component, `this`'s):
    /// the one that takes none, else the one there is.
    pub fn named(&self, name: &str) -> Option<(&TypeKey, &OwnType)> {
        self.types
            .iter()
            .filter(|(k, _)| k.name == name)
            .min_by_key(|(k, _)| k.arity)
    }

    /// Whether a type is named `name`, of any arity.
    pub fn has(&self, name: &str) -> bool {
        self.named(name).is_some()
    }

    /// Every type, by key.
    pub fn all(&self) -> impl Iterator<Item = (&TypeKey, &OwnType)> {
        self.types.iter()
    }

    /// `key` and every type it inherits or implements, nearest first, each
    /// once (a cycle doesn't loop) — by key; their type arguments are the
    /// checker's (`Checker::lineage_of`).
    pub fn lineage(&self, key: &TypeKey) -> Vec<TypeKey> {
        let mut out = vec![key.clone()];
        let mut i = 0;
        while let Some(at) = out.get(i) {
            let bases: Vec<TypeKey> = self
                .types
                .get(at)
                .map(|t| t.bases.iter().filter_map(TypeKey::of_written).collect())
                .unwrap_or_default();
            for b in bases {
                if !out.contains(&b) {
                    out.push(b);
                }
            }
            i += 1;
        }
        out
    }

    /// The lineage of the type named `name` (written without arguments).
    pub fn lineage_named(&self, name: &str) -> Vec<TypeKey> {
        self.named(name)
            .map(|(k, _)| self.lineage(k))
            .unwrap_or_default()
    }

    /// Whether the type named `a` is `b`, inherits it or implements it.
    pub fn is_a(&self, a: &str, b: &str) -> bool {
        self.lineage_named(a).iter().any(|t| t.name == b)
    }

    /// The class the type named `name` inherits (not an interface it implements).
    pub fn base_class(&self, name: &str) -> Option<&str> {
        let (_, t) = self.named(name)?;
        t.bases
            .iter()
            .filter_map(TypeKey::of_written)
            .find(|b| self.types.get(b).is_some_and(|o| !o.interface))
            .and_then(|b| self.types.get_key_value(&b))
            .map(|(k, _)| k.name.as_str())
    }
}
