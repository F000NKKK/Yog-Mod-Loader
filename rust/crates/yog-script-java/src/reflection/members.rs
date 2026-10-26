//! A type's members in its description, each a call on it with what reads,
//! writes or makes it — written as the compiler writes a use of it
//! (`((Box) o).Item`, `((List) o).Count()`, `host.String.Length((String) o)`).

use yog_script_analyze::ast::{Accessor, Decl, DeclKind, Modifier, Param, Type as Written};
use yog_script_analyze::words::{get, init, set};
use yog_script_ids::clr::ACCESSOR_SEP;
use yog_script_ids::jvm::reflection::{
    ABSTRACT_MEMBER, CONSTRUCTOR, FIELD_MEMBER, INIT_ONLY, LITERAL, MEMBER_INFO, METHOD_PARAMETER,
    PARAMETER, PRIVATE, PROPERTY, PROTECTED, STATIC, TAKES, VIRTUAL,
};

use crate::Gen;
use crate::index::Kind;

/// The description's lambdas' parameters: the object, a value set, the arguments.
pub(super) const OBJECT: &str = "o";
pub(super) const VALUE: &str = "v";
pub(super) const ARGS: &str = "a";
/// The description, as its method names it.
pub(super) const DESCRIPTION: &str = "d";
/// No getter or setter.
const NONE: &str = "null";
pub(super) const NO_TRAITS: &str = "0";
const TRAIT_SEP: &str = " | ";

/// The type whose members are described, as their uses need it.
pub(super) struct Owner<'o> {
    /// Its name and arity, as declared.
    pub name: &'o str,
    pub arity: usize,
    /// Its class in Java, without arguments: what an object is cast to.
    pub raw: String,
    /// A built-in's: its companion (`host.String`) and its values' class
    /// (`String`) — its members are the companion's, the value first.
    pub host: Option<(String, String)>,
    /// Its type parameters, by name.
    pub generics: Vec<String>,
}

impl Gen<'_> {
    /// The description's calls of `owner`'s members (`body`), each a statement.
    /// `made`: its objects are made by its constructors.
    pub(super) fn members_described(
        &mut self,
        owner: &Owner,
        body: &[Decl],
        made: bool,
    ) -> Vec<String> {
        let mut out = Vec::new();
        for d in body {
            let line = match &d.kind {
                DeclKind::Field(f) => Some(self.field_described(owner, d, &f.name.text, &f.ty)),
                DeclKind::Property {
                    ty,
                    name,
                    accessors,
                    explicit: None,
                    ..
                } => Some(self.property_described(owner, d, (&name.text, ty), accessors)),
                DeclKind::Method(m) => self.method_described(owner, d, m),
                DeclKind::Ctor(c) if made => {
                    Some(self.constructor_described(owner, &member_traits(d), &c.params))
                }
                _ => None,
            };
            if let Some(line) = line {
                out.push(self.with_attributes(line, &d.attributes));
            }
        }
        out
    }

    /// A record's constructor's parameters: its properties, read by their accessors.
    pub(super) fn record_described(&self, owner: &Owner, params: &[Param]) -> Vec<String> {
        params
            .iter()
            .map(|p| {
                let ty = self.member_type(owner, &p.ty, &[]);
                let n = Self::ident(&p.name.text);
                format!(
                    "{DESCRIPTION}.{PROPERTY}({}, {NO_TRAITS}, {ty}, {OBJECT} -> (({}) {OBJECT}).{n}(), {NONE});",
                    self.code.str(&p.name.text),
                    owner.raw
                )
            })
            .collect()
    }

    /// An enum's cases: its constant fields.
    pub(super) fn cases_described(&self, owner: &Owner, cases: &[String]) -> Vec<String> {
        let ty = super::values::of_class(&owner.raw);
        cases
            .iter()
            .map(|c| {
                format!(
                    "{DESCRIPTION}.{FIELD_MEMBER}({}, {MEMBER_INFO}.{STATIC} | {MEMBER_INFO}.{LITERAL}, {ty}, {OBJECT} -> {}.{}, {NONE});",
                    self.code.str(c),
                    owner.raw,
                    Self::ident(c)
                )
            })
            .collect()
    }

    fn field_described(&self, owner: &Owner, d: &Decl, name: &str, ty: &Written) -> String {
        let n = Self::ident(name);
        let at = if d.has(Modifier::Static) || d.has(Modifier::Const) {
            owner.raw.clone()
        } else {
            format!("(({}) {OBJECT})", owner.raw)
        };
        let fixed = d.has(Modifier::Const) || d.has(Modifier::Readonly);
        let set = if fixed {
            NONE.to_string()
        } else {
            let v = self.cast_value(owner, ty, &[], VALUE);
            format!("({OBJECT}, {VALUE}) -> {at}.{n} = {v}")
        };
        format!(
            "{DESCRIPTION}.{FIELD_MEMBER}({}, {}, {}, {OBJECT} -> {at}.{n}, {set});",
            self.code.str(name),
            member_traits(d),
            self.member_type(owner, ty, &[])
        )
    }

    fn property_described(
        &self,
        owner: &Owner,
        d: &Decl,
        (name, ty): (&str, &Written),
        accessors: &[Accessor],
    ) -> String {
        let n = Self::ident(name);
        let has = |word: &str| accessors.iter().any(|a| a.name.text == word);
        let readable = accessors.is_empty() || has(get::WORD.text);
        let writable = has(set::WORD.text) || has(init::WORD.text);
        let stat = d.has(Modifier::Static);
        let called = self
            .index
            .called(&(owner.name.to_string(), owner.arity), name)
            || owner.host.is_some();
        let target = match (&owner.host, stat) {
            (_, true) => owner.raw.clone(),
            (Some((_, value)), false) => format!("(({value}) {OBJECT})"),
            (None, false) => format!("(({}) {OBJECT})", owner.raw),
        };
        let get = match &owner.host {
            _ if !readable => NONE.to_string(),
            Some((host, _)) if !stat => format!("{OBJECT} -> {host}.{n}({target})"),
            _ if called => format!("{OBJECT} -> {target}.{n}()"),
            _ => format!("{OBJECT} -> {target}.{n}"),
        };
        let v = self.cast_value(owner, ty, &[], VALUE);
        let set = if !writable || owner.host.is_some() {
            NONE.to_string()
        } else if called {
            let setter = format!("{}{ACCESSOR_SEP}{n}", set::WORD.text);
            format!("({OBJECT}, {VALUE}) -> {target}.{setter}({v})")
        } else {
            format!("({OBJECT}, {VALUE}) -> {target}.{n} = {v}")
        };
        format!(
            "{DESCRIPTION}.{PROPERTY}({}, {}, {}, {get}, {set});",
            self.code.str(name),
            member_traits(d),
            self.member_type(owner, ty, &[])
        )
    }

    /// A constructor taking `params`: a new object of the type, made with them.
    pub(super) fn constructor_described(
        &self,
        owner: &Owner,
        traits: &str,
        params: &[Param],
    ) -> String {
        let args = self.args_cast(owner, params, &[]);
        let mut out = format!(
            "{DESCRIPTION}.{CONSTRUCTOR}({traits}, {ARGS} -> new {}({args}))",
            owner.raw
        );
        out.push_str(&self.takes(owner, params, &[]));
        out.push(';');
        out
    }

    /// `.Takes(…)` of each of `params` (of a method of type parameters `generics`).
    pub(super) fn takes(&self, owner: &Owner, params: &[Param], generics: &[String]) -> String {
        params
            .iter()
            .map(|p| {
                format!(
                    ".{TAKES}({}, {}, {})",
                    self.code.str(&p.name.text),
                    self.member_type(owner, &p.ty, generics),
                    p.default.is_some()
                )
            })
            .collect()
    }

    /// `a[0], a[1]`, each cast to its parameter's type.
    pub(super) fn args_cast(&self, owner: &Owner, params: &[Param], generics: &[String]) -> String {
        params
            .iter()
            .enumerate()
            .map(|(i, p)| self.cast_value(owner, &p.ty, generics, &format!("{ARGS}[{i}]")))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// A member's type `t` as a `Type`: the type's parameters and the
    /// method's (`generics`) as the description reaches them.
    pub(super) fn member_type(&self, owner: &Owner, t: &Written, generics: &[String]) -> String {
        let param = |name: &str| {
            if let Some(i) = generics.iter().position(|g| g == name) {
                return Some(format!("{DESCRIPTION}.{METHOD_PARAMETER}({i})"));
            }
            owner
                .generics
                .iter()
                .position(|g| g == name)
                .map(|i| format!("{DESCRIPTION}.{PARAMETER}({i})"))
        };
        self.type_value(t, &param)
            .unwrap_or_else(|| super::values::of_class(yog_script_ids::jvm::OBJECT))
    }

    /// `value` (an `Object`) as type `t` in Java: cast to its class
    /// erased, as is when it's a type parameter (Java's generics are).
    pub(super) fn cast_value(
        &self,
        owner: &Owner,
        t: &Written,
        generics: &[String],
        value: &str,
    ) -> String {
        let java = erased(&self.written(t, true));
        let item = java.trim_end_matches("[]");
        let rank = &java[item.len()..];
        let param = owner.generics.iter().chain(generics).any(|g| g == item);
        match (param, rank.is_empty()) {
            (true, true) => value.to_string(),
            (true, false) => format!("({}{rank}) {value}", yog_script_ids::jvm::OBJECT),
            (false, _) => format!("({java}) {value}"),
        }
    }
}

/// `MemberInfo`'s flags of member `d`.
pub(super) fn member_traits(d: &Decl) -> String {
    // A method's `readonly` is about its `this`, not what Reflection says.
    let is_field = matches!(d.kind, DeclKind::Field(_));
    let flags: Vec<&str> = [
        (d.has(Modifier::Static) || d.has(Modifier::Const), STATIC),
        (d.has(Modifier::Private), PRIVATE),
        (d.has(Modifier::Protected), PROTECTED),
        (d.has(Modifier::Abstract), ABSTRACT_MEMBER),
        (
            d.has(Modifier::Virtual) || d.has(Modifier::Override),
            VIRTUAL,
        ),
        (
            is_field && d.has(Modifier::Readonly) && !d.has(Modifier::Const),
            INIT_ONLY,
        ),
        (d.has(Modifier::Const), LITERAL),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, f)| f)
    .collect();
    if flags.is_empty() {
        NO_TRAITS.to_string()
    } else {
        flags
            .iter()
            .map(|f| format!("{MEMBER_INFO}.{f}"))
            .collect::<Vec<_>>()
            .join(TRAIT_SEP)
    }
}

/// Java type `java` without its type arguments (`List<T>` is `List`).
fn erased(java: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in java.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

impl Owner<'_> {
    /// Whether its objects are made by a constructor of its own (not an
    /// interface, an abstract class, a built-in).
    pub(super) fn made(&self, g: &Gen, d: &Decl) -> bool {
        self.host.is_none()
            && !d.has(Modifier::Abstract)
            && !d.has(Modifier::Static)
            && g.index.kind(&(self.name.to_string(), self.arity)) != Some(Kind::Interface)
    }
}
