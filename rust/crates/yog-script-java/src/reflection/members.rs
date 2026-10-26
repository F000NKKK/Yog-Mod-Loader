//! A type's members in its description, each a call on it with what reads,
//! writes or makes it — written as the compiler writes a use of it
//! (`((Box) o).Item`, `((List) o).Count()`, `host.String.Length((String) o)`).

use yog_script_analyze::ast::{Accessor, Decl, DeclKind, Modifier, Param, Pass, Type as Written};
use yog_script_analyze::words::{get, init, set};
use yog_script_ids::clr::ACCESSOR_SEP;
use yog_script_ids::jvm::reflection::{
    ABSTRACT, CELL_OF, CELLS, CONSTRUCTOR, FIELD_MEMBER, INIT_ONLY, LITERAL, MAKE_BY_REF,
    MEMBER_INFO, METHOD_PARAMETER, PARAMETER, PARAMETER_INFO, PASSED_OUT, PRIVATE, PROPERTY,
    PROTECTED, STATIC, TAKES, VIRTUAL,
};

use super::consts::{ARGS, DESCRIPTION, NO_TRAITS, OBJECT, VALUE};
use crate::Gen;
use crate::index::Kind;
use yog_script_ids::java::{NEW, NULL, RETURN, VAR};
use yog_script_ids::jvm::reflection::CELLS_BACK;
use yog_script_ids::jvm::{REF, TEMP};

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
                // A type's operator: its static method, as .NET's (`op_Addition`).
                DeclKind::Operator(op) => self.method_described(owner, d, &op.method),
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
                    "{DESCRIPTION}.{PROPERTY}({}, {NO_TRAITS}, {ty}, {OBJECT} -> (({}) {OBJECT}).{n}(), {NULL});",
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
                    "{DESCRIPTION}.{FIELD_MEMBER}({}, {MEMBER_INFO}.{STATIC} | {MEMBER_INFO}.{LITERAL}, {ty}, {OBJECT} -> {}.{}, {NULL});",
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
        // A record's never changes, as C#'s.
        let record = self.index.kind(&(owner.name.to_string(), owner.arity)) == Some(Kind::Record);
        let fixed = d.has(Modifier::Const) || d.has(Modifier::Readonly) || record;
        let set = if fixed {
            NULL.to_string()
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
            _ if !readable => NULL.to_string(),
            Some((host, _)) if !stat => format!("{OBJECT} -> {host}.{n}({target})"),
            _ if called => format!("{OBJECT} -> {target}.{n}()"),
            _ => format!("{OBJECT} -> {target}.{n}"),
        };
        let v = self.cast_value(owner, ty, &[], VALUE);
        let set = if !writable || owner.host.is_some() {
            NULL.to_string()
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
        let made = format!("{NEW} {}({args})", owner.raw);
        let runtime = self.index.is_intrinsic(owner.name);
        let make = match around_cells(params, &made, (true, runtime)) {
            Some(block) => block,
            None => made,
        };
        let mut out = format!("{DESCRIPTION}.{CONSTRUCTOR}({traits}, {ARGS} -> {make})");
        out.push_str(&self.takes(owner, params, &[]));
        out.push(';');
        out
    }

    /// `.Takes(…)` of each of `params` (of a method of type parameters `generics`).
    pub(super) fn takes(&self, owner: &Owner, params: &[Param], generics: &[String]) -> String {
        params
            .iter()
            .map(|p| {
                let (name, t) = (
                    self.code.str(&p.name.text),
                    self.member_type(owner, &p.ty, generics),
                );
                let given = p.default.is_some();
                match p.pass.and_then(passing) {
                    Some(how) => format!(".{TAKES}({name}, {t}.{MAKE_BY_REF}(), {given}, {how})"),
                    None => format!(".{TAKES}({name}, {t}, {given})"),
                }
            })
            .collect()
    }

    /// `a[0], a[1]`, each cast to its parameter's type.
    /// A compiled method's `ref` and `out` ones each a cell of it; the
    /// runtime's take it as it is.
    pub(super) fn args_cast(&self, owner: &Owner, params: &[Param], generics: &[String]) -> String {
        let cells = !self.index.is_intrinsic(owner.name);
        params
            .iter()
            .enumerate()
            .map(|(i, p)| match p.pass {
                Some(Pass::Ref | Pass::Out) if cells => {
                    format!("{REF}.{CELL_OF}({ARGS}, {i}, {CELLS})")
                }
                _ => self.cast_value(owner, &p.ty, generics, &format!("{ARGS}[{i}]")),
            })
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
        (d.has(Modifier::Abstract), ABSTRACT),
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
            .join(&super::consts::flags_sep())
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

/// How parameter passing `pass` is described: by reference, `ParameterInfo`'s
/// flags (none for `ref`); `None` for a value's — `in` lends it, `move`
/// hands it on, neither a reference as .NET's.
fn passing(pass: Pass) -> Option<String> {
    match pass {
        Pass::Out => Some(format!("{PARAMETER_INFO}.{PASSED_OUT}")),
        Pass::Ref => Some(NO_TRAITS.to_string()),
        Pass::In | Pass::Move => None,
    }
}

/// A reflection call `call` taking `params`, as a lambda's block: its `ref`
/// and `out` arguments' cells given back into the arguments after it, its
/// value (`gives`) or `null` the result — a `runtime` method's `ref` one
/// given what it gives back. `None` when none's passed so.
pub(super) fn around_cells(
    params: &[Param],
    call: &str,
    (gives, runtime): (bool, bool),
) -> Option<String> {
    let first = params
        .iter()
        .position(|p| matches!(p.pass, Some(Pass::Ref | Pass::Out)))?;
    if runtime {
        return Some(format!("{{ {ARGS}[{first}] = {call}; {RETURN} {NULL}; }}"));
    }
    let cells = format!("{REF}<?>[] {CELLS} = {NEW} {REF}<?>[{}];", params.len());
    let back = format!("{REF}.{CELLS_BACK}({ARGS}, {CELLS});");
    Some(if gives {
        format!("{{ {cells} {VAR} {TEMP} = {call}; {back} {RETURN} {TEMP}; }}")
    } else {
        format!("{{ {cells} {call}; {back} {RETURN} {NULL}; }}")
    })
}
