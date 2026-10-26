//! A type's description, written in its class as `$TYPE`: its name and
//! namespace as declared, what it is, its type parameters, and — read
//! when first asked for — the class it inherits and its interfaces.

use yog_script_analyze::ast::{Decl, DeclKind, Generic, Modifier, Type as Written, TypeKind};
use yog_script_analyze::system_type;
use yog_script_ids::jvm::reflection::{
    ABSTRACT, BASE, CLASS, DECLARE, DELEGATE, ENUM, FIELD, INTERFACE, INTERFACE_TRAIT, PARAMETER,
    PRIMITIVE, SEALED, STRUCT, TYPE,
};

use crate::Gen;
use crate::index::Kind;

/// The description's lambda's parameter.
const DESCRIPTION: &str = "d";
/// What joins a namespace and a name, and `Type`'s flags.
const NAMESPACE_SEP: &str = ".";
const TRAIT_SEP: &str = " | ";
/// A description saying nothing more.
const NOTHING: &str = "{}";

/// A type declaration as its description needs it.
struct Declared<'d> {
    name: &'d str,
    generics: &'d [Generic],
    bases: &'d [Written],
}

impl Gen<'_> {
    /// `$TYPE` of type declaration `d`, in its class.
    pub(crate) fn describe(&mut self, d: &Decl) {
        if let Some(v) = self.declaration(d) {
            self.code
                .line(format!("public static final {TYPE} {FIELD} = {v};"));
        }
    }

    /// `Type.Declare(…)` of type declaration `d`, when it's a type.
    pub(crate) fn declaration(&self, d: &Decl) -> Option<String> {
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
            "{TYPE}.{DECLARE}({java}.class, {}, {}, new String[] {{ {} }}, {DESCRIPTION} -> {})",
            self.code.str(&full),
            traits.join(TRAIT_SEP),
            names.join(", "),
            self.bases_described(what.bases, &params)
        ))
    }

    /// The Java class type `name` of `arity` is described by: its own; a
    /// built-in's companion's (`host.Int32`), or — one without
    /// (`Void`) — its values' (`java.lang.Void`).
    pub(crate) fn class_of(&self, name: &str, arity: usize) -> String {
        match system_type(name) {
            Some(t) if !self.index.is_host(name) => self.java(&t, true),
            _ => self.index.place(name, arity),
        }
    }

    /// `d.Base(…).Interface(…)`: what it inherits, its type parameters
    /// (`params`) as the description reaches them.
    fn bases_described(&self, bases: &[Written], params: &[String]) -> String {
        let param = |name: &str| {
            params
                .iter()
                .position(|p| p == name)
                .map(|i| format!("{DESCRIPTION}.{PARAMETER}({i})"))
        };
        let mut out = DESCRIPTION.to_string();
        for b in bases {
            let Some(v) = self.type_value(b, &param) else {
                continue;
            };
            let call = if self.is_interface(b) {
                INTERFACE
            } else {
                BASE
            };
            out.push_str(&format!(".{call}({v})"));
        }
        if out == DESCRIPTION {
            NOTHING.to_string()
        } else {
            out
        }
    }

    fn is_interface(&self, t: &Written) -> bool {
        match &t.kind {
            TypeKind::Path { path, .. } => path
                .last()
                .is_some_and(|n| self.index.kind(&n.text) == Some(Kind::Interface)),
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
