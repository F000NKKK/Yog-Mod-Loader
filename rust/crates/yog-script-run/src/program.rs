//! The declarations a run can reach — the System library's and the
//! source's — indexed by name: types with their members, the file's methods.

use std::collections::HashMap;

use yog_script_analyze::ast::{
    Ctor, Decl, DeclKind, Expr, Method, MethodBody, Modifier, Param, Variant,
};
use yog_script_analyze::words;

/// A type: what making one sets, what calling on one finds.
#[derive(Default)]
pub struct TypeDef<'a> {
    /// The class it inherits (its first base that's a class of the program).
    pub base: Option<&'a str>,
    pub bases: Vec<&'a str>,
    /// Its primary constructor's parameters.
    pub ctor: &'a [Param],
    /// A record: its constructor's parameters are its fields.
    pub record: bool,
    /// A struct (or a `record struct`): copied where it goes.
    pub value: bool,
    /// Its stored fields and auto-properties, with their first values.
    pub fields: Vec<(&'a str, Option<&'a Expr>)>,
    /// The constructors its body writes.
    pub ctors: Vec<&'a Ctor>,
    /// Its static fields and constants, with their first values: one each.
    pub statics: Vec<(&'a str, Option<&'a Expr>)>,
    /// Properties computed on reading (`T X => …;`), by name.
    pub computed: HashMap<&'a str, &'a MethodBody>,
    pub methods: HashMap<&'a str, Vec<Member<'a>>>,
    pub variants: Vec<&'a Variant>,
}

/// A method and whether it's the type's (`static`) or its objects'.
#[derive(Clone, Copy)]
pub struct Member<'a> {
    pub method: &'a Method,
    pub is_static: bool,
}

#[derive(Default)]
pub struct Program<'a> {
    pub types: HashMap<&'a str, TypeDef<'a>>,
    /// The file's own methods, outside types.
    pub methods: HashMap<&'a str, Vec<&'a Method>>,
    /// The `async` methods, by where they are: what they give is a task.
    asyncs: std::collections::HashSet<usize>,
    /// Each type's methods' type, by where they are: whose statics their
    /// code reaches by name.
    owners: HashMap<usize, &'a str>,
}

impl<'a> Program<'a> {
    /// The program of `sources`' declarations, in order (the library's first).
    pub fn of(sources: &[&'a [Decl]]) -> Self {
        let mut p = Program::default();
        for decls in sources {
            p.add(decls);
        }
        p
    }

    /// A type's `async` methods noted.
    fn note_asyncs(&mut self, body: &[Decl]) {
        for d in body {
            if let DeclKind::Method(m) = &d.kind
                && d.has(Modifier::Async)
            {
                self.asyncs.insert(std::ptr::from_ref(m).addr());
            }
        }
    }

    /// Type `name`'s methods noted as its.
    fn note_owner(&mut self, name: &'a str, body: &'a [Decl]) {
        for d in body {
            if let DeclKind::Method(m) = &d.kind {
                self.owners.insert(std::ptr::from_ref(m).addr(), name);
            }
        }
    }

    /// The type whose method `m` is (none: the file's).
    pub fn owner_of(&self, m: &Method) -> Option<&'a str> {
        self.owners.get(&std::ptr::from_ref(m).addr()).copied()
    }

    /// Static field (or constant) `name` of type `of`: its first value, if any.
    pub fn static_field(&self, of: &str, name: &str) -> Option<Option<&'a Expr>> {
        let def = self.types.get(of)?;
        def.statics
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
    }

    /// Whether `m` is `async`: what it gives is a task of it.
    pub fn is_async(&self, m: &Method) -> bool {
        self.asyncs.contains(&std::ptr::from_ref(m).addr())
    }

    fn add(&mut self, decls: &'a [Decl]) {
        for d in decls {
            self.note_asyncs(std::slice::from_ref(d));
            match &d.kind {
                DeclKind::Method(m) => self.methods.entry(&m.name.text).or_default().push(m),
                DeclKind::Type {
                    word,
                    name,
                    value,
                    header,
                    body,
                } => {
                    let def = TypeDef {
                        bases: header.bases.iter().filter_map(simple_name).collect(),
                        ctor: header.ctor.as_deref().unwrap_or_default(),
                        record: word.text == words::record::WORD.text,
                        value: *value,
                        ..members(body)
                    };
                    self.types.insert(&name.text, def);
                    self.note_asyncs(body);
                    self.note_owner(&name.text, body);
                }
                DeclKind::Enum {
                    name,
                    variants,
                    body,
                    ..
                } => {
                    let def = TypeDef {
                        variants: variants.iter().collect(),
                        ..members(body)
                    };
                    self.types.insert(&name.text, def);
                    self.note_asyncs(body);
                    self.note_owner(&name.text, body);
                }
                kind => self.add(kind.members()),
            }
        }
        let classes: Vec<(&'a str, Option<&'a str>)> = self
            .types
            .iter()
            .map(|(n, t)| {
                (
                    *n,
                    t.bases.iter().copied().find(|b| self.types.contains_key(b)),
                )
            })
            .collect();
        for (n, base) in classes {
            if let Some(t) = self.types.get_mut(n) {
                t.base = base;
            }
        }
    }

    /// `name` and the classes it inherits, nearest first.
    pub fn lineage(&self, name: &'a str) -> Vec<&'a str> {
        let mut out = vec![name];
        while let Some(base) = out
            .last()
            .and_then(|n| self.types.get(n))
            .and_then(|t| t.base)
        {
            if out.contains(&base) {
                break;
            }
            out.push(base);
        }
        out
    }

    /// Method `name` taking `count` arguments of type `of` (or a class it
    /// inherits).
    pub fn method(&self, of: &'a str, name: &str, count: usize) -> Option<Member<'a>> {
        self.lineage(of).into_iter().find_map(|t| {
            self.types
                .get(t)?
                .methods
                .get(name)?
                .iter()
                .copied()
                .find(|m| takes(m.method, count))
        })
    }

    /// The file's method `name` taking `count` arguments.
    pub fn file_method(&self, name: &str, count: usize) -> Option<&'a Method> {
        self.methods
            .get(name)?
            .iter()
            .copied()
            .find(|m| takes(m, count))
    }

    /// The enum and the case that `name` is, among `of`'s cases.
    pub fn variant(&self, of: &str, name: &str) -> Option<&'a Variant> {
        self.types
            .get(of)?
            .variants
            .iter()
            .copied()
            .find(|v| v.name.text == name)
    }
}

/// Whether `m` can be called with `count` arguments (the rest defaulted).
fn takes(m: &Method, count: usize) -> bool {
    let required = m
        .params
        .iter()
        .take_while(|p| p.default.is_none() && !p.params)
        .count();
    // A `params` one takes any number past the others.
    let rest = m.params.last().is_some_and(|p| p.params);
    (required..=m.params.len()).contains(&count) || (rest && count >= required)
}

/// A body's fields, computed properties and methods.
fn members<'a>(body: &'a [Decl]) -> TypeDef<'a> {
    let mut def = TypeDef::default();
    for d in body {
        match &d.kind {
            DeclKind::Ctor(c) => def.ctors.push(c),
            DeclKind::Field(f) if d.has(Modifier::Static) || d.has(Modifier::Const) => {
                def.statics.push((&f.name.text, f.value.as_ref()));
            }
            DeclKind::Field(f) => def.fields.push((&f.name.text, f.value.as_ref())),
            DeclKind::Method(m) => def.methods.entry(&m.name.text).or_default().push(Member {
                method: m,
                is_static: d.has(Modifier::Static),
            }),
            DeclKind::Property {
                name,
                accessors,
                value,
                ..
            } => match accessors.iter().find_map(|a| a.body.as_ref()) {
                Some(get) => {
                    def.computed.insert(&name.text, get);
                }
                None => def.fields.push((&name.text, value.as_ref())),
            },
            _ => {}
        }
    }
    def
}

fn simple_name(t: &yog_script_analyze::ast::Type) -> Option<&str> {
    match &t.kind {
        yog_script_analyze::ast::TypeKind::Path { path, .. } => {
            path.last().map(|n| n.text.as_str())
        }
        _ => None,
    }
}
