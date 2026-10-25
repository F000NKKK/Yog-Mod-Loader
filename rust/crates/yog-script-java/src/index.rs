//! The source's own types and the library's as the Java they become needs
//! them: what each is (a class, a struct, a record, an interface, an enum
//! with or without data, a delegate), which members are computed
//! properties (Java methods), and where each is in Java.

use std::collections::{BTreeSet, HashMap, HashSet};

use yog_script_analyze::ast::{Decl, DeclKind, MethodBody};
use yog_script_analyze::words;

use crate::types::RUNTIME;

/// The runtime's package of built-in types' members' companions (`[Host]`).
const HOST_PACKAGE: &str = "host";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Class,
    /// A struct: an object copied where it goes (`copy()`).
    Struct,
    /// A Java record: its fields read by accessors (`p.X()`).
    Record,
    Interface,
    /// A Java enum: its cases are constants.
    Enum,
    /// A sealed interface, a record per case.
    DataEnum,
    /// A functional interface: its one method `Invoke`.
    Delegate,
}

pub(crate) struct Info {
    pub kind: Kind,
    /// Properties computed on reading: Java methods (`x.Room()`).
    pub computed: HashSet<String>,
    /// The types it inherits and implements, by name.
    pub bases: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Index {
    pub types: HashMap<String, Info>,
    /// Each enum's cases' fields, by (enum, case).
    pub cases: HashMap<(String, String), Vec<String>>,
    /// The arities each name is declared of (`Action`: 0 to 16).
    arities: HashMap<String, BTreeSet<usize>>,
    /// The library's types' Java packages, by name.
    packages: HashMap<String, String>,
    /// The namespace the declarations being walked are in, and whether
    /// they're the library's.
    namespace: Vec<String>,
    library: bool,
    /// The `[Host]` types: built-in types' members' companions.
    hosts: HashSet<String>,
}

impl Index {
    /// The types of the library and of `decls`.
    pub fn collect(decls: &[Decl]) -> Self {
        let mut index = Index {
            library: true,
            ..Index::default()
        };
        index.walk(&yog_script_analyze::system::DECLS);
        index.library = false;
        index.namespace.clear();
        index.walk(decls);
        index
    }

    fn walk(&mut self, decls: &[Decl]) {
        for d in decls {
            match &d.kind {
                DeclKind::Namespace { path, body: None } => {
                    self.namespace = path.iter().map(|n| n.text.clone()).collect();
                }
                DeclKind::Delegate(m) => {
                    self.declared(&m.name.text, arity(&m.generics));
                    let info = Info {
                        kind: Kind::Delegate,
                        computed: HashSet::new(),
                        bases: Vec::new(),
                    };
                    self.types.insert(m.name.text.clone(), info);
                }
                DeclKind::Type { .. } => self.type_decl(d),
                DeclKind::Enum { .. } => self.enum_decl(d),
                kind => self.walk(kind.members()),
            }
        }
    }

    /// A class, struct, record or interface.
    fn type_decl(&mut self, d: &Decl) {
        let DeclKind::Type {
            word,
            name,
            value,
            body,
            header,
            ..
        } = &d.kind
        else {
            return;
        };
        self.declared(&name.text, arity(&header.generics));
        if self.library && crate::library::is_host(d) {
            // Apart, so it doesn't hide Java's of its name (`String`) in
            // the runtime's package.
            self.packages
                .insert(name.text.clone(), format!("{RUNTIME}.{HOST_PACKAGE}"));
            self.hosts.insert(name.text.clone());
        }
        let kind = if word.text == words::record::WORD.text {
            Kind::Record
        } else if word.text == words::interface::WORD.text {
            Kind::Interface
        } else if *value {
            Kind::Struct
        } else {
            Kind::Class
        };
        // An interface's properties, and a hand-written type's, are its
        // methods in Java.
        let computed = if kind == Kind::Interface || crate::library::is_intrinsic(d) {
            properties(body)
        } else {
            computed(body)
        };
        let bases = header.bases.iter().filter_map(base_name).collect();
        let info = Info {
            kind,
            computed,
            bases,
        };
        self.types.insert(name.text.clone(), info);
    }

    /// An enum: its cases' fields, whether any case holds data.
    fn enum_decl(&mut self, d: &Decl) {
        let DeclKind::Enum {
            name,
            variants,
            body,
            generics,
            ..
        } = &d.kind
        else {
            return;
        };
        self.declared(&name.text, arity(generics));
        for v in variants {
            let fields = v.fields.iter().map(|f| f.name.text.clone()).collect();
            self.cases
                .insert((name.text.clone(), v.name.text.clone()), fields);
        }
        let kind = if variants.iter().any(|v| !v.fields.is_empty()) {
            Kind::DataEnum
        } else {
            Kind::Enum
        };
        let info = Info {
            kind,
            computed: computed(body),
            bases: Vec::new(),
        };
        self.types.insert(name.text.clone(), info);
    }

    /// Case `case`'s fields' names (of enum `of`, or whichever has it).
    pub fn case_fields(&self, of: Option<&str>, case: &str) -> Vec<String> {
        let of = of
            .map(str::to_string)
            .or_else(|| self.enum_of(case))
            .unwrap_or_default();
        self.cases
            .get(&(of, case.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// The enum a case named `case` is of.
    pub fn enum_of(&self, case: &str) -> Option<String> {
        self.cases
            .keys()
            .find(|(_, c)| c == case)
            .map(|(o, _)| o.clone())
    }

    /// Type `name` of `arity` declared here: its arity noted, and its
    /// package when it's the library's.
    fn declared(&mut self, name: &str, arity: usize) {
        self.arities
            .entry(name.to_string())
            .or_default()
            .insert(arity);
        if self.library {
            self.packages
                .insert(name.to_string(), package(&self.namespace));
        } else {
            // The source's own type hides the library's of its name, as C#'s.
            self.packages.remove(name);
        }
    }

    /// Where type `name` of `arity` is in Java: the library's at its
    /// package, the source's nested in its class; a name several arities
    /// share told apart as the CLR does, by its arity (`Action2`), the
    /// one of none keeping its name.
    pub fn place(&self, name: &str, arity: usize) -> String {
        let shared = self.arities.get(name).is_some_and(|a| a.len() > 1);
        let simple = if shared && arity > 0 {
            format!("{name}{arity}")
        } else {
            name.to_string()
        };
        match self.packages.get(name) {
            Some(package) => format!("{package}.{simple}"),
            None => simple,
        }
    }

    /// Where type `name` is, named without its arguments (`Math.Max`): the
    /// one of the fewest.
    pub fn place_named(&self, name: &str) -> String {
        let arity = self
            .arities
            .get(name)
            .and_then(|a| a.first().copied())
            .unwrap_or_default();
        self.place(name, arity)
    }

    /// Where built-in type `t`'s members' companion is (`string`'s), if
    /// the library has one.
    pub fn host(&self, t: &yog_script_analyze::Type) -> Option<String> {
        let name = yog_script_analyze::system_name(t.non_null())?;
        self.hosts.contains(name).then(|| self.place(name, 0))
    }

    pub fn kind(&self, name: &str) -> Option<Kind> {
        self.types.get(name).map(|i| i.kind)
    }

    /// Whether `name` of type `of` is read by calling it (a computed
    /// property, a record's field).
    pub fn called(&self, of: &str, name: &str) -> bool {
        self.types.get(of).is_some_and(|i| {
            i.kind == Kind::Record
                || i.computed.contains(name)
                || i.bases.iter().any(|b| self.called(b, name))
        })
    }
}

/// A base's name as written (`ICollection<T>` is `ICollection`).
fn base_name(t: &yog_script_analyze::ast::Type) -> Option<String> {
    match &t.kind {
        yog_script_analyze::ast::TypeKind::Path { path, .. } => path.last().map(|n| n.text.clone()),
        _ => None,
    }
}

/// Every property of a body.
fn properties(body: &[Decl]) -> HashSet<String> {
    body.iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Property { name, .. } => Some(name.text.clone()),
            _ => None,
        })
        .collect()
}

/// A body's computed properties (`T X => …;`, `T X { get => …; }`).
fn computed(body: &[Decl]) -> HashSet<String> {
    body.iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Property {
                name, accessors, ..
            } if accessors
                .iter()
                .any(|a| matches!(a.body, Some(MethodBody::Block(_) | MethodBody::Expr(_)))) =>
            {
                Some(name.text.clone())
            }
            _ => None,
        })
        .collect()
}

/// How many type parameters `generics` declare (lifetimes aside).
fn arity(generics: &[yog_script_analyze::ast::Generic]) -> usize {
    generics.iter().filter(|g| !g.lifetime).count()
}

/// The runtime's package a library namespace is at: `System` the
/// runtime's own, each namespace under it a package of its lowercased name
/// (`System.Collections.Generic` → `dev.yog.script.collections.generic`),
/// a Java word escaped as an identifier would be.
fn package(namespace: &[String]) -> String {
    std::iter::once(RUNTIME.to_string())
        .chain(namespace.iter().skip(1).map(|n| {
            <yog_codegen_java::Java as yog_codegen::Identifiers>::ident(&n.to_lowercase())
        }))
        .collect::<Vec<_>>()
        .join(".")
}
