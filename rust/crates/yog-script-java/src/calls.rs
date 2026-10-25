//! Calls, members, objects made, the built-in macros — each as Java
//! writes it: an own method by its name, a case made with `new`, a
//! built-in type's members on its companion (`s.Length` is
//! `String.Length(s)`), the library's by their places.

use yog_script_analyze::ast::{Arg, Expr, ExprKind, Name, Pass, Type as Written};
use yog_script_analyze::macros::builtin;
use yog_script_analyze::{Type, yog};
use yog_script_ids::system::RESULT;

use super::Gen;
use crate::index::Kind;
use crate::types::RUNTIME;
use yog_script_ids::clr::INVOKE;

impl Gen<'_> {
    pub(crate) fn args(&mut self, args: &[Arg]) -> String {
        args.iter()
            .map(|a| self.handed_expr(&a.value))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub(crate) fn member(&mut self, e: &Expr, on: &Expr, name: &str, safe: bool) -> String {
        let on_type = self.types.of(on).cloned();
        let target = self.expr(on);
        let host = on_type.as_ref().and_then(|t| self.index.host(t));
        let member = match on_type.as_ref().map(Type::non_null) {
            Some(Type::Static(of)) => return self.static_member(e, of, name),
            // A built-in type's property: its companion's static, the value first.
            Some(t) if let Some(host) = host => {
                let n = Self::ident(name);
                let rank = array_rank(t);
                return self.maybe_null(&target, safe, |v| format!("{host}.{n}({v}{rank})"));
            }
            Some(Type::Own(of, _)) if self.index.called(of, name) => {
                Some(format!("{}()", Self::ident(name)))
            }
            Some(Type::Yog(_)) => Some(format!("{}()", yog::field(name))),
            _ => None,
        }
        .unwrap_or_else(|| Self::ident(name));
        let read = self.maybe_null(&target, safe, |v| format!("{v}.{member}"));
        match on_type.as_ref().map(Type::non_null) {
            Some(Type::Yog(_)) => self.viewed(e, read),
            _ => read,
        }
    }

    /// `read(v)` of `target`; `safe` (`?.`): `null` when `target` is.
    fn maybe_null(&mut self, target: &str, safe: bool, read: impl Fn(&str) -> String) -> String {
        if !safe {
            return read(target);
        }
        let t = self.temp();
        self.code.line(format!("var {t} = {target};"));
        format!("({t} == null ? null : {})", read(&t))
    }

    /// `Type.Name`: an enum's case, a static field.
    fn static_member(&mut self, e: &Expr, of: &str, name: &str) -> String {
        let n = Self::ident(name);
        match self.index.kind(of) {
            Some(Kind::DataEnum) => {
                let _ = e;
                format!("new {of}.{n}<>()")
            }
            _ => format!("{}.{n}", self.own(of)),
        }
    }

    pub(crate) fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> String {
        let called = self.called(e, callee, args);
        let called = self.typed_array(e, called);
        self.given_back(e, args, called)
    }

    fn called(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> String {
        let a = self.args(args);
        match &callee.kind {
            ExprKind::Ident(name) => match self.types.of(callee) {
                // A delegate held: called by its interface's method.
                t if self.is_delegate(t) => format!("{}.{INVOKE}({a})", Self::ident(name)),
                _ if self.is_result_case(name) => {
                    format!("new {}.{}<>({a})", self.own(RESULT), Self::ident(name))
                }
                _ => format!("{}({a})", Self::ident(name)),
            },
            ExprKind::Member { expr, name, .. } => self.method_call(e, expr, &name.text, &a),
            _ => {
                let f = self.expr(callee);
                if self.is_delegate(self.types.of(callee)) {
                    format!("{f}.{INVOKE}({a})")
                } else {
                    format!("{f}({a})")
                }
            }
        }
    }

    /// Where call `e`'s method runs when its type is a `[Host]` one (the
    /// method's, else the value's — `t`), with the rank after the value an
    /// array's members take.
    fn host_of_call(&self, e: &Expr, t: &Type) -> Option<(String, String)> {
        let owner = self.types.method(e).and_then(|m| m.owner.as_deref());
        match owner {
            Some(o) if self.index.is_host(o) => {
                let rank = if o == yog_script_ids::system::ARRAY {
                    array_rank(t)
                } else {
                    String::new()
                };
                Some((self.index.place(o, 0), rank))
            }
            Some(_) => None,
            None => self.index.host(t).map(|h| (h, array_rank(t))),
        }
    }

    /// A call whose method gives an array of its type parameter
    /// (`ToArray`, `Array.Empty<T>`): Java's generics gave an `Object[]`,
    /// made the array `e`'s type is (`int[]` for an `i32[]`).
    fn typed_array(&self, e: &Expr, called: String) -> String {
        let generic = self
            .types
            .method(e)
            .is_some_and(|m| of_type_parameter(&m.ret));
        match self.types.of(e).map(Type::non_null) {
            Some(t @ Type::Array(..)) if generic => format!(
                "{}.{}({called}, {}.class)",
                self.array_host(),
                yog_script_ids::jvm::TYPED_ARRAY,
                self.java(t, false)
            ),
            _ => called,
        }
    }

    /// A runtime method taking an argument `ref` (`Array.Resize(ref xs,
    /// n)`): Java has no `ref`, so the runtime's gives the new value back
    /// and the place takes it (`xs = Array.Resize(xs, n)`).
    fn given_back(&mut self, e: &Expr, args: &[Arg], called: String) -> String {
        let runtime = self
            .types
            .method(e)
            .and_then(|m| m.owner.as_deref())
            .is_some_and(|o| self.index.is_intrinsic(o));
        match args.iter().find(|a| a.pass == Some(Pass::Ref)) {
            Some(place) if runtime => format!("{} = {called}", self.expr(&place.value)),
            _ => called,
        }
    }

    fn is_result_case(&self, name: &str) -> bool {
        yog_script_analyze::system::DECLS
            .iter()
            .any(|d| match &d.kind {
                yog_script_analyze::ast::DeclKind::Enum {
                    name: n, variants, ..
                } => n.text == RESULT && variants.iter().any(|v| v.name.text == name),
                _ => false,
            })
    }

    /// `on.name(a)`.
    fn method_call(&mut self, e: &Expr, on: &Expr, name: &str, a: &str) -> String {
        let on_type = self.types.of(on).cloned();
        let target = self.expr(on);
        let n = Self::ident(name);
        match on_type.as_ref().map(Type::non_null) {
            Some(Type::Static(of)) => match self.index.kind(of) {
                Some(Kind::DataEnum) => format!("new {of}.{n}<>({a})"),
                _ => format!("{}.{n}({a})", self.own(of)),
            },
            // A method of a `[Host]` type (`string`'s, `object`'s): its
            // companion's, the value first.
            Some(t) if let Some((host, rank)) = self.host_of_call(e, t) => {
                let sep = if a.is_empty() { "" } else { ", " };
                format!("{host}.{n}({target}{rank}{sep}{a})")
            }
            Some(Type::Yog(group)) => match yog::service(group, name) {
                Some(s) if s.on_handle => {
                    let sep = if a.is_empty() { "" } else { ", " };
                    format!("new {}({target}{sep}{a}).call()", s.record)
                }
                Some(s) => format!("new {}({a}).call()", s.record),
                None => {
                    self.not_yet("this Yog method", e.span);
                    String::new()
                }
            },
            _ => format!("{target}.{n}({a})"),
        }
    }

    pub(crate) fn new_object(
        &mut self,
        e: &Expr,
        ty: Option<&Written>,
        args: &[Arg],
        init: &[(Name, Expr)],
    ) -> String {
        let a = self.args(args);
        let t = match ty {
            Some(t) => self.written(t, true),
            None => self
                .types
                .of(e)
                .map_or_else(|| "Object".into(), |t| self.java(t, true)),
        };
        // A handle is its subject: `new World(dim)` is `dim`.
        if matches!(self.types.of(e), Some(Type::Yog(h)) if yog::is_handle(h)) {
            return a;
        }
        let made = format!("new {t}({a})");
        if init.is_empty() {
            return made;
        }
        let tmp = self.temp();
        self.code.line(format!("var {tmp} = {made};"));
        for (field, v) in init {
            let v = self.handed_expr(v);
            self.code
                .line(format!("{tmp}.{} = {v};", Self::ident(&field.text)));
        }
        tmp
    }

    pub(crate) fn macro_call(&mut self, e: &Expr, name: &str, args: &[Expr]) -> String {
        let at = self.at(e.span);
        let given: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
        let first = given
            .first()
            .cloned()
            .unwrap_or_else(|| self.code.str(name));
        let ys = format!("{RUNTIME}.Ys");
        let Some(m) = builtin::named(name) else {
            self.not_yet("this macro", e.span);
            return String::new();
        };
        if std::ptr::eq(m, &builtin::ASSERT) {
            let message = given
                .get(1)
                .cloned()
                .unwrap_or_else(|| self.code.str("assertion failed"));
            return format!("{ys}.check({first}, {message}, {at})");
        }
        if std::ptr::eq(m, &builtin::ASSERT_EQ) {
            return format!(
                "{ys}.checkEq({first}, {}, {at})",
                given.get(1).cloned().unwrap_or_default()
            );
        }
        if std::ptr::eq(m, &builtin::DBG) {
            let what = self
                .code
                .str(&format!("{}", args.first().map_or(0, |a| a.span.start)));
            return format!("{ys}.dbg({first}, {what}, {at})");
        }
        if m.gives == builtin::Gives::Never {
            let message = if given.is_empty() {
                self.code.str(m.doc)
            } else {
                format!("String.valueOf({first})")
            };
            return format!("{ys}.panic({message}, {at})");
        }
        self.not_yet("this macro", e.span);
        String::new()
    }
}

impl Gen<'_> {
    /// Whether a value of `t` is a delegate: a lambda's or a method's own
    /// type, or a declared delegate's.
    fn is_delegate(&self, t: Option<&Type>) -> bool {
        match t.map(Type::non_null) {
            Some(Type::Fn(..)) => true,
            Some(Type::Own(name, _)) => self.index.kind(name) == Some(Kind::Delegate),
            _ => false,
        }
    }
}

/// After an array handed to `System.Array`'s companion, its rank (`, 2`):
/// Java can't tell `T[,]` from `T[][]`. Nothing after any other value.
fn array_rank(t: &Type) -> String {
    match t {
        Type::Array(_, rank) => format!(", {rank}"),
        _ => String::new(),
    }
}

/// Whether `t` is an array of a type parameter (`T[]`, `T[][]`).
fn of_type_parameter(t: &Type) -> bool {
    match t {
        Type::Array(item, _) => matches!(**item, Type::Param(_)) || of_type_parameter(item),
        _ => false,
    }
}
