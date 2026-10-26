//! The source's own types and the library's as the Java they become needs
//! them: what each is (a class, a struct, a record, an interface, an enum
//! with or without data, a delegate), which members are computed
//! properties (Java methods), and where each is in Java.

use std::collections::{BTreeSet, HashMap, HashSet};

use yog_script_analyze::ast::{Decl, DeclKind, MethodBody};
use yog_script_analyze::words;

use crate::java_names::TypeRef;
use crate::types::RUNTIME;

use yog_script_ids::jvm::HOST_PACKAGE;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Class,
    /// A struct: an object copied where it goes (`copy()`).
    Struct,
    /// A Java record: its fields read by accessors (`p.X()`).
    Record,
    Interface,
    /// An enum (`System.Enum`'s): its cases without data its constants,
    /// those with data its subclasses.
    Enum,
    /// A functional interface: its one method `Invoke`.
    Delegate,
}

pub(crate) struct Info {
    pub kind: Kind,
    /// Properties computed on reading: Java methods (`x.Room()`).
    pub computed: HashSet<String>,
    /// The types it inherits and implements, by name and arity.
    pub bases: Vec<TypeRef>,
    /// Whether it's made with arguments (it declares a constructor taking
    /// some): an attribute without one takes what a macro reads (`[Derive(Clone)]`).
    pub takes_args: bool,
    /// An `abstract` class: never made by `new`.
    pub is_abstract: bool,
    /// A record's constructor's parameters, by name, in order: what taking
    /// one apart gives (`let (k, v) = pair`).
    pub record_params: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Index {
    /// Each type by its name and arity: one name, several types (C#'s).
    pub types: HashMap<TypeRef, Info>,
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
    /// The `[Intrinsic]` types: the runtime's, written by hand.
    intrinsics: HashSet<String>,
    /// Each type's namespace, as written (`System.Collections.Generic`).
    namespaces: HashMap<String, String>,
    /// Each type's shape, for its methods' Java names.
    shapes: HashMap<crate::java_names::TypeRef, crate::java_names::Shape>,
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
                DeclKind::Namespace {
                    path,
                    body: Some(body),
                } => {
                    let outer = self.namespace.clone();
                    self.namespace.extend(path.iter().map(|n| n.text.clone()));
                    self.walk(body);
                    self.namespace = outer;
                }
                DeclKind::Delegate(m) => {
                    self.declared(&m.name.text, arity(&m.generics));
                    let info = Info {
                        kind: Kind::Delegate,
                        computed: HashSet::new(),
                        bases: Vec::new(),
                        takes_args: false,
                        is_abstract: false,
                        record_params: Vec::new(),
                    };
                    self.types
                        .insert((m.name.text.clone(), arity(&m.generics)), info);
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
        self.shapes.insert(
            (name.text.clone(), arity(&header.generics)),
            crate::java_names::shape_of(header, body),
        );
        if self.library && crate::library::is_intrinsic(d) {
            self.intrinsics.insert(name.text.clone());
        }
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
        let takes_args = header.ctor.as_ref().is_some_and(|p| !p.is_empty())
            || body
                .iter()
                .any(|m| matches!(&m.kind, DeclKind::Ctor(c) if !c.params.is_empty()));
        let info = Info {
            kind,
            computed,
            bases,
            takes_args,
            is_abstract: d.has(yog_script_analyze::ast::Modifier::Abstract),
            record_params: header
                .ctor
                .iter()
                .flatten()
                .map(|p| p.name.text.clone())
                .collect(),
        };
        self.types
            .insert((name.text.clone(), arity(&header.generics)), info);
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
        let info = Info {
            kind: Kind::Enum,
            computed: computed(body),
            bases: Vec::new(),
            takes_args: false,
            is_abstract: false,
            record_params: Vec::new(),
        };
        self.types
            .insert((name.text.clone(), arity(generics)), info);
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
        self.namespaces.insert(
            name.to_string(),
            self.namespace.join(yog_script_ids::system::NAMESPACE_SEP),
        );
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
        let (name, arity) = self.named(name);
        self.place(&name, arity)
    }

    /// Where built-in type `t`'s members' companion is (`string`'s), if
    /// the library has one.
    pub fn host(&self, t: &yog_script_analyze::Type) -> Option<String> {
        let name = match t.non_null() {
            yog_script_analyze::Type::Array(..) => yog_script_ids::system::ARRAY,
            t => yog_script_analyze::system_name(t)?,
        };
        self.hosts.contains(name).then(|| self.place(name, 0))
    }

    /// Whether `name` of type `of` is read by calling it in what `of`
    /// inherits (an interface's property `of` stores in a field).
    pub fn inherited_call(&self, of: &TypeRef, name: &str) -> bool {
        self.types
            .get(of)
            .is_some_and(|i| i.bases.iter().any(|b| self.called(b, name)))
    }

    /// Type `name`'s shape, if it's one of the library's or the source's.
    pub(crate) fn shape(
        &self,
        name: &crate::java_names::TypeRef,
    ) -> Option<&crate::java_names::Shape> {
        self.shapes.get(name)
    }

    /// Type `name`'s namespace (empty when it's in none).
    pub fn namespace_of(&self, name: &str) -> &str {
        self.namespaces.get(name).map_or("", String::as_str)
    }

    /// Whether type `name` is a `[Host]` one: a built-in type's members.
    pub fn is_host(&self, name: &str) -> bool {
        self.hosts.contains(name)
    }

    /// Whether type `name` is the runtime's, written by hand.
    pub fn is_intrinsic(&self, name: &str) -> bool {
        self.intrinsics.contains(name)
    }

    /// Whether type `name` (of no type parameters: an attribute) is made with arguments.
    pub fn takes_args(&self, name: &str) -> bool {
        self.types
            .get(&(name.to_string(), 0))
            .is_some_and(|i| i.takes_args)
    }

    /// Record `t`'s constructor's parameters, in order (none for another type).
    pub fn record_params(&self, t: &TypeRef) -> &[String] {
        self.types
            .get(t)
            .filter(|i| i.kind == Kind::Record)
            .map_or(&[], |i| i.record_params.as_slice())
    }

    /// Whether type `t` is made by `new` and keeps the type arguments it's
    /// made with (`Typed`): a generic class or struct, not abstract — the
    /// compiler's, or one the runtime writes by hand.
    pub fn keeps_types(&self, t: &TypeRef) -> bool {
        t.1 > 0
            && self
                .types
                .get(t)
                .is_some_and(|i| matches!(i.kind, Kind::Class | Kind::Struct) && !i.is_abstract)
    }

    pub fn kind(&self, t: &TypeRef) -> Option<Kind> {
        self.types.get(t).map(|i| i.kind)
    }

    /// Type `name` written without its arguments (`Math`, a case's enum):
    /// the one of the fewest, as `place_named` finds it.
    pub fn named(&self, name: &str) -> TypeRef {
        let arity = self
            .arities
            .get(name)
            .and_then(|a| a.first().copied())
            .unwrap_or_default();
        (name.to_string(), arity)
    }

    /// Whether `name` of type `of` is read by calling it (a computed
    /// property, a record's field).
    pub fn called(&self, of: &TypeRef, name: &str) -> bool {
        self.types.get(of).is_some_and(|i| {
            i.kind == Kind::Record
                || i.computed.contains(name)
                || i.bases.iter().any(|b| self.called(b, name))
        })
    }
}

/// A base as written: its name and arity (`ICollection<T>` is `("ICollection", 1)`).
fn base_name(t: &yog_script_analyze::ast::Type) -> Option<TypeRef> {
    match &t.kind {
        yog_script_analyze::ast::TypeKind::Path { path, args } => {
            path.last().map(|n| (n.text.clone(), args.len()))
        }
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
