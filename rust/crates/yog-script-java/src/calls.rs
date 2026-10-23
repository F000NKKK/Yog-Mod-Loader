//! Calls, members, objects made, the built-in macros — each as Java
//! writes it: an own method by its name, a case made with `new`, the
//! library's members as Java's (`s.Length` is `s.length()`, `Math.Max` is
//! `Math.max`).

use yog_script_analyze::ast::{Arg, Expr, ExprKind, Name, Type as Written};
use yog_script_analyze::macros::builtin;
use yog_script_analyze::system::RESULT;
use yog_script_analyze::{Type, yog};

use super::Gen;
use crate::index::Kind;
use crate::types::{RUNTIME, delegate, own};

/// The library's members done by Java's own: (Yog Script's, Java's), a
/// `()` after the Java name when it's a method there.
const STRING_MEMBERS: &[(&str, &str)] = &[("Length", "length()")];
const LIST_MEMBERS: &[(&str, &str)] = &[("Count", "size()")];
const STRING_METHODS: &[(&str, &str)] = &[
    ("ToUpper", "toUpperCase"),
    ("ToLower", "toLowerCase"),
    ("Trim", "trim"),
    ("StartsWith", "startsWith"),
    ("EndsWith", "endsWith"),
    ("Contains", "contains"),
];
const LIST_METHODS: &[(&str, &str)] = &[
    ("Add", "add"),
    ("Contains", "contains"),
    ("IndexOf", "indexOf"),
    ("Clear", "clear"),
    ("RemoveAt", "remove"),
];
const MATH: &str = "Math";
const MATH_METHODS: &[(&str, &str)] = &[
    ("Max", "max"),
    ("Min", "min"),
    ("Abs", "abs"),
    ("Sqrt", "sqrt"),
    ("Floor", "floor"),
    ("Round", "rint"),
];
/// A string's `Substring`, done by the runtime (by characters and a length).
const SUBSTRING: &str = "Substring";

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
        let member = match on_type.as_ref().map(Type::non_null) {
            Some(Type::Static(of)) => return self.static_member(e, of, name),
            Some(Type::String) => java_member(STRING_MEMBERS, name),
            Some(Type::List(_)) => java_member(LIST_MEMBERS, name),
            Some(Type::Own(of, _)) if self.index.called(of, name) => {
                Some(format!("{}()", Self::ident(name)))
            }
            Some(Type::Yog(_)) => Some(format!("{}()", yog::field(name))),
            _ => None,
        }
        .unwrap_or_else(|| Self::ident(name));
        if safe {
            let t = self.temp();
            self.code.line(format!("var {t} = {target};"));
            return format!("({t} == null ? null : {t}.{member})");
        }
        format!("{target}.{member}")
    }

    /// `Type.Name`: an enum's case, a static field.
    fn static_member(&mut self, e: &Expr, of: &str, name: &str) -> String {
        let n = Self::ident(name);
        match self.index.kind(of) {
            Some(Kind::DataEnum) => {
                let _ = e;
                format!("new {of}.{n}<>()")
            }
            _ => format!("{}.{n}", own(of)),
        }
    }

    pub(crate) fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> String {
        let a = self.args(args);
        match &callee.kind {
            ExprKind::Ident(name) => match self.types.of(callee) {
                // A delegate held: called by its interface's method.
                Some(Type::Fn(params, ret)) => {
                    format!("{}.{}({a})", Self::ident(name), delegate(params, ret).1)
                }
                _ if self.is_result_case(name) => {
                    format!("new {}.{}<>({a})", own(RESULT), Self::ident(name))
                }
                _ => format!("{}({a})", Self::ident(name)),
            },
            ExprKind::Member { expr, name, .. } => self.method_call(e, expr, &name.text, &a),
            _ => {
                let f = self.expr(callee);
                match self.types.of(callee) {
                    Some(Type::Fn(params, ret)) => format!("{f}.{}({a})", delegate(params, ret).1),
                    _ => format!("{f}({a})"),
                }
            }
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
            Some(Type::Static(of)) if of == MATH => match java_member(MATH_METHODS, name) {
                Some(m) => format!("Math.{m}({a})"),
                None => format!("Math.{n}({a})"),
            },
            Some(Type::Static(of)) => match self.index.kind(of) {
                Some(Kind::DataEnum) => format!("new {of}.{n}<>({a})"),
                _ => format!("{}.{n}({a})", own(of)),
            },
            Some(Type::String) if name == SUBSTRING => {
                let _ = e;
                format!("{RUNTIME}.Ys.substring({target}, {a})")
            }
            Some(Type::String) => format!(
                "{target}.{}({a})",
                java_member(STRING_METHODS, name).unwrap_or(n)
            ),
            Some(Type::List(_)) => format!(
                "{target}.{}({a})",
                java_member(LIST_METHODS, name).unwrap_or(n)
            ),
            Some(Type::Fn(params, ret)) => format!("{target}.{}({a})", delegate(params, ret).1),
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
            Some(t) => crate::types::written(t, true),
            None => self
                .types
                .of(e)
                .map_or_else(|| "Object".into(), |t| crate::types::java(t, true)),
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

fn java_member(table: &[(&str, &str)], name: &str) -> Option<String> {
    table
        .iter()
        .find(|(ys, _)| *ys == name)
        .map(|(_, java)| (*java).to_string())
}
