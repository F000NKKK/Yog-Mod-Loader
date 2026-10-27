//! The source's own types as inheritance sees them: what each is, what it
//! inherits, its members' modifiers.

use std::collections::HashMap;

use crate::ast::{Decl, DeclKind, Modifier, Name, Type};
use crate::source::token::Span;
use crate::words::{class, interface, record};

pub(super) struct Info<'d> {
    pub name: &'d str,
    /// How many type parameters it takes: one name, several types (C#'s).
    pub arity: usize,
    pub span: Span,
    /// A class (or a `record class`): it may have a base class.
    pub class: bool,
    pub interface: bool,
    pub is_abstract: bool,
    pub sealed: bool,
    /// A `readonly class` or a record: never changed, so shared.
    pub readonly: bool,
    /// Its bases as written, plain names only (the game's aren't here),
    /// each with as many arguments as it's written with.
    pub bases: Vec<(&'d Name, usize)>,
    pub members: Vec<Member>,
    /// The library's (its rules were checked with it), the runtime's
    /// (`[Intrinsic]`: implemented by hand).
    pub library: bool,
    pub intrinsic: bool,
}

pub(super) struct Member {
    pub name: String,
    pub span: Span,
    pub is_abstract: bool,
    pub is_virtual: bool,
    pub overrides: bool,
    /// It has a body (an implementation).
    pub body: bool,
    /// The interface it implements explicitly, by name (`ICollection`).
    pub explicit: Option<String>,
    /// It says its access (`public`): an explicit implementation can't.
    pub access: bool,
}

pub(super) struct Index<'d> {
    pub types: HashMap<(&'d str, usize), Info<'d>>,
}

impl<'d> Index<'d> {
    pub fn collect(decls: &'d [Decl]) -> Self {
        let mut index = Index {
            types: HashMap::new(),
        };
        index.walk(decls);
        index
    }

    /// The types so far are the library's.
    pub fn mark_library(&mut self) {
        self.types.values_mut().for_each(|i| i.library = true);
    }

    /// `decls`' types too (a source's, beside the library's).
    pub fn add(&mut self, decls: &'d [Decl]) {
        self.walk(decls);
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
                let arity = header.generics.iter().filter(|g| !g.lifetime).count();
                let info = Info {
                    name: &name.text,
                    arity,
                    span: name.span,
                    class,
                    interface,
                    is_abstract: d.has(Modifier::Abstract),
                    sealed: d.has(Modifier::Sealed),
                    readonly: d.has(Modifier::Readonly) || word.text == record::WORD.text,
                    bases: header.bases.iter().filter_map(plain_base).collect(),
                    members: body.iter().filter_map(|m| member(m, interface)).collect(),
                    library: false,
                    intrinsic: d.attributes.iter().any(|a| {
                        a.path
                            .last()
                            .is_some_and(|n| n.text == yog_script_ids::system::INTRINSIC)
                    }),
                };
                self.types.insert((&name.text, arity), info);
            }
            self.walk(d.kind.members());
        }
    }

    /// Base `b` as written: the type of its name and arity.
    pub fn base(&self, (b, arity): &(&'d Name, usize)) -> Option<&Info<'d>> {
        self.types.get(&(b.text.as_str(), *arity))
    }

    /// The own types `info` inherits or implements, nearest first, each
    /// once — `object` last, every type's base.
    pub fn ancestors(&self, info: &Info<'d>) -> Vec<&Info<'d>> {
        let mut out = self.written_ancestors(info);
        if let Some(object) = self.types.get(&(yog_script_ids::clr::OBJECT, 0))
            && !same(info, object)
            && !out.iter().any(|o| same(o, object))
        {
            out.push(object);
        }
        out
    }

    fn written_ancestors(&self, start: &Info<'d>) -> Vec<&Info<'d>> {
        let mut out: Vec<&Info<'d>> = Vec::new();
        let mut next = vec![start];
        while let Some(n) = next.pop() {
            for b in &n.bases {
                if let Some(info) = self.base(b)
                    && !same(info, start)
                    && !out.iter().any(|o| same(o, info))
                {
                    out.push(info);
                    next.push(info);
                }
            }
        }
        out
    }
}

/// Whether `a` and `b` are one type: one name, one arity.
fn same(a: &Info, b: &Info) -> bool {
    a.name == b.name && a.arity == b.arity
}

/// A base by its plain name and how many arguments it's written with.
fn plain_base(t: &Type) -> Option<(&Name, usize)> {
    match &t.kind {
        // `Base`, `System.Logging.LogValues`: by its name, as `TypeKey`s are.
        crate::ast::TypeKind::Path { path, args } => path.last().map(|n| (n, args.len())),
        _ => None,
    }
}

fn plain_name(t: &Type) -> Option<&Name> {
    match &t.kind {
        // `IList<T>` by its name: its arguments don't change what it asks.
        crate::ast::TypeKind::Path { path, .. } => path.last(),
        _ => None,
    }
}

/// A method, a property or an indexer, with what inheritance reads of it.
/// A class's property without `abstract` is implemented (`{ get; }` is
/// stored); an interface's only with a body.
fn member(d: &Decl, interface: bool) -> Option<Member> {
    let mut explicit = None;
    let (name, span, body) = match &d.kind {
        DeclKind::Method(m) => {
            explicit = m
                .explicit
                .as_ref()
                .and_then(plain_name)
                .map(|n| n.text.clone());
            (m.name.text.as_str(), m.name.span, m.body.is_some())
        }
        DeclKind::Property {
            name,
            accessors,
            value,
            explicit: of,
            ..
        } => {
            explicit = of.as_ref().and_then(plain_name).map(|n| n.text.clone());
            (
                name.text.as_str(),
                name.span,
                !interface || value.is_some() || accessors.iter().any(|a| a.body.is_some()),
            )
        }
        DeclKind::Indexer(ix) => (
            yog_script_ids::clr::INDEXER,
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
        explicit,
        access: [
            Modifier::Public,
            Modifier::Private,
            Modifier::Protected,
            Modifier::Internal,
        ]
        .into_iter()
        .any(|m| d.has(m)),
    })
}
