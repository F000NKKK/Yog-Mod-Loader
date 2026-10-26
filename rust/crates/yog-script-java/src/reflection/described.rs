//! A type's description, written in its class: `$TYPE` (its name and
//! namespace as declared, what it is, its type parameters) and, read when
//! first asked for, `$describe` — the class it inherits, its interfaces,
//! its members.

use yog_script_analyze::ast::{Decl, DeclKind, Generic, Modifier, Type as Written, TypeKind};
use yog_script_analyze::system_type;
use yog_script_ids::jvm::reflection::{
    ABSTRACT, BASE, CLASS, DECLARE, DELEGATE, DESCRIBE, DESCRIPTION as DESCRIPTION_CLASS, ENUM,
    FIELD, INTERFACE, INTERFACE_TRAIT, PRIMITIVE, SEALED, STRUCT, TYPE,
};

use super::members::{DESCRIPTION, NO_TRAITS, Owner};
use crate::Gen;
use crate::index::Kind;

/// What joins a namespace and a name, and `Type`'s flags.
const NAMESPACE_SEP: &str = ".";
const TRAIT_SEP: &str = " | ";
/// Java's word that the description's casts (to a generic type's raw
/// class, to a type parameter) are known safe.
const UNCHECKED: &str = "@SuppressWarnings({\"unchecked\", \"rawtypes\"})";

/// A type declaration as its description needs it.
struct Declared<'d> {
    name: &'d str,
    generics: &'d [Generic],
    bases: &'d [Written],
}

impl Gen<'_> {
    /// `$TYPE` and `$describe` of type declaration `d`, in its class.
    pub(crate) fn describe(&mut self, d: &Decl) {
        let Some(what) = declared(d) else {
            return;
        };
        let holder = self
            .index
            .place(what.name, type_params(what.generics).len());
        if let Some(v) = self.declaration(d, &holder) {
            self.code
                .line(format!("public static final {TYPE} {FIELD} = {v};"));
            self.describe_method(d);
        }
    }

    /// `Type.Declare(…)` of type declaration `d`, its description filled by
    /// `holder`'s `$describe`.
    pub(crate) fn declaration(&self, d: &Decl, holder: &str) -> Option<String> {
        let what = declared(d)?;
        let params: Vec<String> = type_params(what.generics);
        let java = self.class_of(what.name, params.len());
        let namespace = self.index.namespace_of(what.name);
        let full = if namespace.is_empty() {
            what.name.to_string()
        } else {
            format!("{namespace}{NAMESPACE_SEP}{}", what.name)
        };
        let traits: Vec<String> = self
            .traits(d, what.name)
            .iter()
            .map(|t| format!("{TYPE}.{t}"))
            .collect();
        let names: Vec<String> = params.iter().map(|p| self.code.str(p)).collect();
        Some(format!(
            "{TYPE}.{DECLARE}({java}.class, {}, {}, new String[] {{ {} }}, {holder}::{DESCRIBE})",
            self.code.str(&full),
            traits.join(TRAIT_SEP),
            names.join(", "),
        ))
    }

    /// `$describe`: what `d` inherits and its members, a call each.
    pub(crate) fn describe_method(&mut self, d: &Decl) {
        let lines = self.described_lines(d);
        self.code.line(UNCHECKED);
        self.code.open(format!(
            "private static void {DESCRIBE}({DESCRIPTION_CLASS} {DESCRIPTION})"
        ));
        for l in lines {
            self.code.line(l);
        }
        self.code.close("");
    }

    fn described_lines(&mut self, d: &Decl) -> Vec<String> {
        let Some(what) = declared(d) else {
            return Vec::new();
        };
        let Some(owner) = self.owner_of(&what) else {
            return Vec::new();
        };
        let mut out = self.bases_described(&owner, what.bases);
        let attributes = self.attributes_described(&d.attributes);
        if !attributes.is_empty() {
            out.push(format!("{DESCRIPTION}{attributes};"));
        }
        match &d.kind {
            DeclKind::Type {
                word, header, body, ..
            } => {
                let made = owner.made(self, d);
                out.extend(self.members_described(&owner, body, made));
                let record = word.text == yog_script_analyze::words::record::WORD.text;
                if record {
                    out.extend(
                        self.record_described(&owner, header.ctor.as_deref().unwrap_or_default()),
                    );
                }
                let declares = body.iter().any(|m| matches!(m.kind, DeclKind::Ctor(_)));
                match &header.ctor {
                    Some(params) if made => {
                        out.push(self.constructor_described(&owner, NO_TRAITS, params));
                    }
                    // C#'s: a class declaring none has one taking nothing.
                    None if made && !declares && !self.index.is_intrinsic(what.name) => {
                        out.push(self.constructor_described(&owner, NO_TRAITS, &[]));
                    }
                    _ => {}
                }
            }
            DeclKind::Enum { variants, .. }
                if self.index.kind(&(what.name.to_string(), owner.arity)) == Some(Kind::Enum) =>
            {
                let cases: Vec<String> = variants.iter().map(|v| v.name.text.clone()).collect();
                out.extend(self.cases_described(&owner, &cases));
            }
            DeclKind::Mod { body, .. } => out.extend(self.members_described(&owner, body, false)),
            _ => {}
        }
        out
    }

    /// The type `what` is, as its members' uses need it; none for a
    /// built-in the runtime has no values' class of (`Array`).
    fn owner_of<'d>(&self, what: &Declared<'d>) -> Option<Owner<'d>> {
        let generics = type_params(what.generics);
        let raw = self.index.place(what.name, generics.len());
        let host = if self.index.is_host(what.name) {
            let t = system_type(what.name)?;
            Some((raw.clone(), self.java(&t, true)))
        } else {
            None
        };
        Some(Owner {
            name: what.name,
            arity: generics.len(),
            raw,
            host,
            generics,
        })
    }

    /// The class type `name` of `arity` is described by: its own; a
    /// built-in's companion's (`host.Int32`), or — one without
    /// (`Void`) — its values' (`java.lang.Void`).
    pub(crate) fn class_of(&self, name: &str, arity: usize) -> String {
        match system_type(name) {
            Some(t) if !self.index.is_host(name) => self.java(&t, true),
            _ => self.index.place(name, arity),
        }
    }

    /// `d.Base(…);`, `d.Interface(…);`: what it inherits.
    fn bases_described(&self, owner: &Owner, bases: &[Written]) -> Vec<String> {
        bases
            .iter()
            .map(|b| {
                let call = if self.is_interface(b) {
                    INTERFACE
                } else {
                    BASE
                };
                format!("{DESCRIPTION}.{call}({});", self.member_type(owner, b, &[]))
            })
            .collect()
    }

    fn is_interface(&self, t: &Written) -> bool {
        match &t.kind {
            TypeKind::Path { path, args } => path.last().is_some_and(|n| {
                self.index.kind(&(n.text.clone(), args.len())) == Some(Kind::Interface)
            }),
            _ => false,
        }
    }

    /// What type `d` (named `name`) is, as `Type`'s flags.
    fn traits(&self, d: &Decl, name: &str) -> Vec<&'static str> {
        let mut out = match &d.kind {
            DeclKind::Type { word, value, .. } => {
                if word.text == yog_script_analyze::words::interface::WORD.text {
                    vec![INTERFACE_TRAIT]
                } else if *value {
                    vec![STRUCT]
                } else {
                    vec![CLASS]
                }
            }
            DeclKind::Enum { .. } => vec![ENUM],
            DeclKind::Delegate(_) => vec![DELEGATE],
            _ => vec![CLASS, SEALED],
        };
        if d.has(Modifier::Abstract) {
            out.push(ABSTRACT);
        }
        if d.has(Modifier::Sealed) || d.has(Modifier::Static) {
            out.push(SEALED);
        }
        // A number, `bool`, `char`: their types in `System` (`Int32`).
        if system_type(name).is_some_and(|t| crate::types::primitive(&t).is_some()) {
            out.push(PRIMITIVE);
        }
        out
    }
}

/// Type declaration `d`'s name, type parameters and bases.
fn declared(d: &Decl) -> Option<Declared<'_>> {
    Some(match &d.kind {
        DeclKind::Type { name, header, .. } => Declared {
            name: &name.text,
            generics: &header.generics,
            bases: &header.bases,
        },
        DeclKind::Enum { name, generics, .. } => Declared {
            name: &name.text,
            generics,
            bases: &[],
        },
        DeclKind::Delegate(m) => Declared {
            name: &m.name.text,
            generics: &m.generics,
            bases: &[],
        },
        DeclKind::Mod { name, .. } => Declared {
            name: &name.text,
            generics: &[],
            bases: &[],
        },
        _ => return None,
    })
}

fn type_params(generics: &[Generic]) -> Vec<String> {
    generics
        .iter()
        .filter(|g| !g.lifetime)
        .map(|g| g.name.text.clone())
        .collect()
}
