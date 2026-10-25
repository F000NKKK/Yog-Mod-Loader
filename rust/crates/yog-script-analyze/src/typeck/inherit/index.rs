//! The source's own types as inheritance sees them: what each is, what it
//! inherits, its members' modifiers.

use std::collections::HashMap;

use crate::ast::{Decl, DeclKind, Modifier, Name, Type};
use crate::source::token::Span;
use crate::words::{class, interface, record};

pub(super) struct Info<'d> {
    pub name: &'d str,
    pub span: Span,
    /// A class (or a `record class`): it may have a base class.
    pub class: bool,
    pub interface: bool,
    pub is_abstract: bool,
    pub sealed: bool,
    /// Its bases as written, plain names only (the game's aren't here).
    pub bases: Vec<&'d Name>,
    pub members: Vec<Member>,
}

pub(super) struct Member {
    pub name: String,
    pub span: Span,
    pub is_abstract: bool,
    pub is_virtual: bool,
    pub overrides: bool,
    /// It has a body (an implementation).
    pub body: bool,
}

pub(super) struct Index<'d> {
    pub types: HashMap<&'d str, Info<'d>>,
}

impl<'d> Index<'d> {
    pub fn collect(decls: &'d [Decl]) -> Self {
        let mut index = Index {
            types: HashMap::new(),
        };
        index.walk(decls);
        index
    }

    fn walk(&mut self, decls: &'d [Decl]) {
        for d in decls {
            if let DeclKind::Type {
                word,
                name,
                value,
                header,
                body,
            } = &d.kind
            {
                let class =
                    !value && (word.text == class::WORD.text || word.text == record::WORD.text);
                let interface = word.text == interface::WORD.text;
                let info = Info {
                    name: &name.text,
                    span: name.span,
                    class,
                    interface,
                    is_abstract: d.has(Modifier::Abstract),
                    sealed: d.has(Modifier::Sealed),
                    bases: header.bases.iter().filter_map(plain_name).collect(),
                    members: body.iter().filter_map(|m| member(m, interface)).collect(),
                };
                self.types.insert(&name.text, info);
            }
            self.walk(d.kind.members());
        }
    }

    /// The own types `name` inherits or implements, nearest first, each once.
    pub fn ancestors(&self, name: &str) -> Vec<&Info<'d>> {
        let mut out: Vec<&Info<'d>> = Vec::new();
        let mut next = vec![name];
        while let Some(n) = next.pop() {
            for b in self
                .types
                .get(n)
                .map(|i| i.bases.as_slice())
                .unwrap_or_default()
            {
                if let Some(info) = self.types.get(b.text.as_str())
                    && b.text != name
                    && !out.iter().any(|o| o.name == info.name)
                {
                    out.push(info);
                    next.push(info.name);
                }
            }
        }
        out
    }
}

fn plain_name(t: &Type) -> Option<&Name> {
    match &t.kind {
        crate::ast::TypeKind::Path { path, args } if args.is_empty() && path.len() == 1 => {
            path.first()
        }
        _ => None,
    }
}

/// A method, a property or an indexer, with what inheritance reads of it.
/// A class's property without `abstract` is implemented (`{ get; }` is
/// stored); an interface's only with a body.
fn member(d: &Decl, interface: bool) -> Option<Member> {
    let (name, span, body) = match &d.kind {
        DeclKind::Method(m) => (m.name.text.as_str(), m.name.span, m.body.is_some()),
        DeclKind::Property {
            name,
            accessors,
            value,
            ..
        } => (
            name.text.as_str(),
            name.span,
            !interface || value.is_some() || accessors.iter().any(|a| a.body.is_some()),
        ),
        DeclKind::Indexer(ix) => (
            crate::system::INDEXER,
            ix.span,
            !interface || ix.accessors.iter().any(|a| a.body.is_some()),
        ),
        _ => return None,
    };
    let body = body && !d.has(Modifier::Abstract);
    Some(Member {
        name: name.to_string(),
        span,
        is_abstract: d.has(Modifier::Abstract),
        is_virtual: d.has(Modifier::Virtual),
        overrides: d.has(Modifier::Override),
        body,
    })
}
