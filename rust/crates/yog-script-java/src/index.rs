//! The source's own types as the Java they become needs them: what each
//! is (a class, a struct, a record, an interface, an enum with or without
//! data) and which members are computed properties (Java methods).

use std::collections::{HashMap, HashSet};

use yog_script_analyze::ast::{Decl, DeclKind, MethodBody};
use yog_script_analyze::words;

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
}

pub(crate) struct Info {
    pub kind: Kind,
    /// Properties computed on reading: Java methods (`x.Room()`).
    pub computed: HashSet<String>,
}

#[derive(Default)]
pub(crate) struct Index {
    pub types: HashMap<String, Info>,
}

impl Index {
    pub fn collect(decls: &[Decl]) -> Self {
        let mut index = Index::default();
        index.walk(decls);
        index
    }

    fn walk(&mut self, decls: &[Decl]) {
        for d in decls {
            match &d.kind {
                DeclKind::Type { word, name, value, body, .. } => {
                    let kind = if word.text == words::record::WORD.text {
                        Kind::Record
                    } else if word.text == words::interface::WORD.text {
                        Kind::Interface
                    } else if *value {
                        Kind::Struct
                    } else {
                        Kind::Class
                    };
                    self.types.insert(name.text.clone(), Info { kind, computed: computed(body) });
                }
                DeclKind::Enum { name, variants, body, .. } => {
                    let kind = if variants.iter().any(|v| !v.fields.is_empty()) { Kind::DataEnum } else { Kind::Enum };
                    self.types.insert(name.text.clone(), Info { kind, computed: computed(body) });
                }
                kind => self.walk(kind.members()),
            }
        }
    }

    pub fn kind(&self, name: &str) -> Option<Kind> {
        self.types.get(name).map(|i| i.kind)
    }

    /// Whether `name` of type `of` is read by calling it (a computed
    /// property, a record's field).
    pub fn called(&self, of: &str, name: &str) -> bool {
        self.types.get(of).is_some_and(|i| i.kind == Kind::Record || i.computed.contains(name))
    }
}

/// A body's computed properties (`T X => …;`, `T X { get => …; }`).
fn computed(body: &[Decl]) -> HashSet<String> {
    body.iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Property { name, accessors, .. }
                if accessors.iter().any(|a| matches!(a.body, Some(MethodBody::Block(_) | MethodBody::Expr(_)))) =>
            {
                Some(name.text.clone())
            }
            _ => None,
        })
        .collect()
}
