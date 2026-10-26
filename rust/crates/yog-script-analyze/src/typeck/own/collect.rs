//! The library's and a source's declarations collected into [`OwnTypes`].

use super::{OwnType, OwnTypes, TypeKey, members::members};
use crate::ast::{self, Decl, DeclKind};

impl OwnTypes {
    /// The types of each of `sources` (the library's, then the file's).
    pub fn collect(sources: &[&[Decl]]) -> Self {
        let mut out = OwnTypes::default();
        for decls in sources {
            out.current.clear();
            out.current_usings = super::super::names::Directives::collect(decls).usings;
            out.file(decls);
        }
        out
    }

    /// The library's types, the project's other sources' (`others`: their
    /// types only — a file's own methods and fields are its class's in
    /// Java, not reached by name from another), then `file`'s.
    pub fn collect_project(library: &[Decl], others: &[&[Decl]], file: &[Decl]) -> Self {
        let mut out = OwnTypes::collect(&[library]);
        for decls in others {
            out.current.clear();
            out.current_usings = super::super::names::Directives::collect(decls).usings;
            out.types_only = true;
            out.file(decls);
        }
        out.types_only = false;
        out.current.clear();
        out.current_usings = super::super::names::Directives::collect(file).usings;
        out.file(file);
        out
    }

    fn file(&mut self, decls: &[Decl]) {
        for d in decls {
            let file_level = matches!(
                &d.kind,
                DeclKind::Method(_)
                    | DeclKind::Field(_)
                    | DeclKind::Property { .. }
                    | DeclKind::Event { .. }
            );
            if self.types_only && file_level {
                continue;
            }
            match &d.kind {
                DeclKind::Method(m) => {
                    if let Some(side) = side_of(d) {
                        self.sides.insert(m.name.text.clone(), side);
                    }
                    self.methods
                        .entry(m.name.text.clone())
                        .or_default()
                        .push(m.clone());
                }
                DeclKind::Field(f) => {
                    self.values.insert(f.name.text.clone(), f.ty.clone());
                }
                DeclKind::Property { ty, name, .. } | DeclKind::Event { ty, name } => {
                    self.values.insert(name.text.clone(), ty.clone());
                }
                DeclKind::Unit { name, .. } => self.units.push(name.text.clone()),
                DeclKind::Namespace { path, body } => {
                    let path: Vec<String> = path.iter().map(|n| n.text.clone()).collect();
                    match body {
                        None => self.current = path,
                        Some(body) => {
                            let inner = [self.current.clone(), path].concat();
                            let outer = std::mem::replace(&mut self.current, inner);
                            self.file(body);
                            self.current = outer;
                        }
                    }
                }
                _ => {
                    self.extension_methods_of(d);
                    self.type_decl(d);
                }
            }
        }
    }

    /// A type's extension methods: its static methods whose first
    /// parameter is `this`.
    fn extension_methods_of(&mut self, d: &Decl) {
        let DeclKind::Type {
            name, header, body, ..
        } = &d.kind
        else {
            return;
        };
        let key = TypeKey::new(
            name.text.clone(),
            header.generics.iter().filter(|g| !g.lifetime).count(),
        );
        for m in body.iter().filter(|m| m.has(ast::Modifier::Static)) {
            if let DeclKind::Method(method) = &m.kind
                && method.params.first().is_some_and(|p| p.extension)
            {
                self.extension_methods
                    .entry(method.name.text.clone())
                    .or_default()
                    .push((key.clone(), method.clone()));
            }
        }
    }

    /// Type `name` of `generics`, made if it isn't yet, its namespace noted.
    fn declared(&mut self, name: &str, generics: &[ast::Generic]) -> &mut OwnType {
        self.namespaces
            .insert(name.to_string(), self.current.clone());
        self.usings
            .insert(name.to_string(), self.current_usings.clone());
        let params = type_params(generics);
        let t = self
            .types
            .entry(TypeKey::new(name, params.len()))
            .or_default();
        t.generics = params;
        t.variance = generics
            .iter()
            .filter(|g| !g.lifetime)
            .map(|g| g.variance)
            .collect();
        t
    }

    fn type_decl(&mut self, d: &Decl) {
        let (t, body, has) = match &d.kind {
            DeclKind::Type {
                name,
                header,
                body,
                word,
                value,
            } => {
                let record = word.text == crate::words::record::WORD.text;
                let t = self.declared(&name.text, &header.generics);
                t.bases = header.bases.clone();
                t.interface = word.text == crate::words::interface::WORD.text;
                t.is_abstract = d.has(ast::Modifier::Abstract);
                t.heap = !value;
                t.record = record;
                t.readonly_all = d.has(ast::Modifier::Readonly) || record;
                t.threads = super::threads::threads(d);
                t.ctor = header.ctor.clone().unwrap_or_default();
                // A record's constructor parameters are its properties.
                if record {
                    for p in header.ctor.iter().flatten() {
                        t.values.insert(p.name.text.clone(), p.ty.clone());
                        t.stored.push(p.name.text.clone());
                    }
                }
                (t, body.as_slice(), header.has.as_slice())
            }
            DeclKind::Content {
                name,
                header,
                body,
                kind,
            } if kind.text != yog_ids::kind::RECIPE => {
                let t = self.declared(&name.text, &header.generics);
                t.content = Some(kind.text.clone());
                t.heap = true;
                (t, body.as_slice(), header.has.as_slice())
            }
            DeclKind::Component { name, header, body }
            | DeclKind::Screen {
                name, header, body, ..
            } => {
                let t = self.declared(&name.text, &header.generics);
                (t, body.as_slice(), header.has.as_slice())
            }
            DeclKind::Delegate(m) => {
                let t = self.declared(&m.name.text, &m.generics);
                t.delegate = Some(m.clone());
                t.heap = true;
                return;
            }
            // The mod is an object too: its members, its constructor.
            DeclKind::Mod { name, body } => {
                (self.declared(&name.text, &[]), body.as_slice(), &[][..])
            }
            DeclKind::Enum {
                name,
                generics,
                variants,
                body,
                ..
            } => {
                let t = self.declared(&name.text, generics);
                t.cases = variants.iter().map(|v| v.name.text.clone()).collect();
                t.is_enum = true;
                t.variants = variants
                    .iter()
                    .filter(|v| !v.fields.is_empty())
                    .map(|v| (v.name.text.clone(), v.fields.clone()))
                    .collect();
                (t, body.as_slice(), &[][..])
            }
            _ => return,
        };
        t.components.extend(has.iter().filter_map(component_name));
        members(t, body);
    }

    /// `extend T { … }`'s members and components: added to `T` when it's
    /// the source's own, kept beside the game's members when it's the game's.
    pub fn extensions(&mut self, decls: &[Decl]) {
        for d in decls {
            if let DeclKind::Extend { ty, header, body } = &d.kind
                && let Some(key) = TypeKey::of_written(ty)
            {
                let t = match self.types.get_mut(&key) {
                    Some(own) => own,
                    None => self.extended.entry(key.name).or_default(),
                };
                t.components
                    .extend(header.has.iter().filter_map(component_name));
                members(t, body);
            }
        }
    }
}

/// The names of a declaration's type parameters (lifetimes aside).
fn type_params(generics: &[ast::Generic]) -> Vec<String> {
    generics
        .iter()
        .filter(|g| !g.lifetime)
        .map(|g| g.name.text.clone())
        .collect()
}

/// `Charge(1000)` or `Charge`: the component's name.
fn component_name(e: &ast::Expr) -> Option<String> {
    use ast::ExprKind;
    match &e.kind {
        ExprKind::Ident(n) => Some(n.clone()),
        ExprKind::Call { callee, .. } => component_name(callee),
        ExprKind::Member { name, .. } => Some(name.text.clone()),
        _ => None,
    }
}

/// The side declaration `d` says it runs on (`client`, `server`), if any.
fn side_of(d: &ast::Decl) -> Option<crate::index::Side> {
    use crate::index::Side;
    d.modifiers.iter().find_map(|m| match m {
        ast::Modifier::Server => Some(Side::Server),
        ast::Modifier::Client => Some(Side::Client),
        _ => None,
    })
}
