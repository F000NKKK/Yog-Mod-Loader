//! The types a source declares, as the checker looks members up in them:
//! fields and properties by type, methods by signature, the components an
//! object has, an enum's or a union's cases.

use std::collections::HashMap;

use crate::ast::{self, Decl, DeclKind, Method};

/// A declared type's members, still as written (typed when looked up).
#[derive(Default)]
pub(crate) struct OwnType {
    pub values: HashMap<String, ast::Type>,
    pub methods: HashMap<String, Vec<Method>>,
    /// The components an object of it has (`has Charge(…)`), by name.
    pub components: Vec<String>,
    pub cases: Vec<String>,
}

/// Every type the source declares, by name; the units it declares.
#[derive(Default)]
pub(crate) struct OwnTypes {
    pub types: HashMap<String, OwnType>,
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
    pub fn collect(decls: &[Decl]) -> Self {
        let mut out = OwnTypes::default();
        out.file(decls);
        out
    }

    fn file(&mut self, decls: &[Decl]) {
        for d in decls {
            match &d.kind {
                DeclKind::Method(m) => self
                    .methods
                    .entry(m.name.text.clone())
                    .or_default()
                    .push(m.clone()),
                DeclKind::Field(f) => {
                    self.values.insert(f.name.text.clone(), f.ty.clone());
                }
                DeclKind::Property { ty, name, .. } => {
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
                _ => self.type_decl(d),
            }
        }
    }

    fn type_decl(&mut self, d: &Decl) {
        let (name, body, has, ctor) = match &d.kind {
            DeclKind::Type {
                name,
                header,
                body,
                word,
            } => {
                // A record's constructor parameters are its properties.
                let props = (word.text == crate::words::record::WORD.text)
                    .then_some(header.ctor.as_deref())
                    .flatten();
                (name, body.as_slice(), header.has.as_slice(), props)
            }
            DeclKind::Content {
                name,
                header,
                body,
                kind,
            } if kind.text != yog_ids::kind::RECIPE => {
                (name, body.as_slice(), header.has.as_slice(), None)
            }
            DeclKind::Component { name, header, body }
            | DeclKind::Screen {
                name, header, body, ..
            } => (name, body.as_slice(), header.has.as_slice(), None),
            // The mod is an object too: its members, its constructor.
            DeclKind::Mod { name, body } => (name, body.as_slice(), &[][..], None),
            DeclKind::Enum { name, cases } => {
                self.namespaces
                    .insert(name.text.clone(), self.current.clone());
                let t = self.types.entry(name.text.clone()).or_default();
                t.cases = cases.iter().map(|c| c.text.clone()).collect();
                return;
            }
            DeclKind::Union {
                name,
                variants,
                body,
                ..
            } => {
                self.namespaces
                    .insert(name.text.clone(), self.current.clone());
                let t = self.types.entry(name.text.clone()).or_default();
                t.cases = variants.iter().map(|v| v.name.text.clone()).collect();
                members(t, body);
                return;
            }
            _ => return,
        };
        self.namespaces
            .insert(name.text.clone(), self.current.clone());
        let t = self.types.entry(name.text.clone()).or_default();
        for p in ctor.into_iter().flatten() {
            t.values.insert(p.name.text.clone(), p.ty.clone());
        }
        t.components = has.iter().filter_map(component_name).collect();
        members(t, body);
    }

    /// `extend T { … }`'s members and components: added to `T` when it's
    /// the source's own, kept beside the game's members when it's the game's.
    pub fn extensions(&mut self, decls: &[Decl]) {
        for d in decls {
            if let DeclKind::Extend { ty, header, body } = &d.kind
                && let Some(name) = super::written::simple_path(ty)
            {
                let t = match self.types.get_mut(name) {
                    Some(own) => own,
                    None => self.extended.entry(name.to_string()).or_default(),
                };
                t.components
                    .extend(header.has.iter().filter_map(component_name));
                members(t, body);
            }
        }
    }
}

/// The values and methods of a body.
fn members(t: &mut OwnType, body: &[Decl]) {
    for d in body {
        match &d.kind {
            DeclKind::Field(f) => {
                t.values.insert(f.name.text.clone(), f.ty.clone());
            }
            DeclKind::Property { ty, name, .. } => {
                t.values.insert(name.text.clone(), ty.clone());
            }
            DeclKind::Event { ty, name } => {
                t.values.insert(name.text.clone(), ty.clone());
            }
            DeclKind::Method(m) => t
                .methods
                .entry(m.name.text.clone())
                .or_default()
                .push(m.clone()),
            _ => {}
        }
    }
}

/// `Charge(1000)` or `Charge`: the component's name.
fn component_name(e: &crate::ast::Expr) -> Option<String> {
    use crate::ast::ExprKind;
    match &e.kind {
        ExprKind::Ident(n) => Some(n.clone()),
        ExprKind::Call { callee, .. } => component_name(callee),
        ExprKind::Member { name, .. } => Some(name.text.clone()),
        _ => None,
    }
}
