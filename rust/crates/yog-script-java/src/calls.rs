//! Calls, members, objects made, the built-in macros — each as Java
//! writes it: an own method by its name, a case made with `new`, a
//! built-in type's members on its companion (`s.Length` is
//! `String.Length(s)`), the library's by their places.

use yog_script_analyze::ast::{Arg, Expr, ExprKind, Name, Pass, Stmt, Type as Written};
use yog_script_analyze::{Type, yog};
use yog_script_ids::members::MADE;
use yog_script_ids::system::RESULT;

use super::Gen;
use crate::index::Kind;
use crate::types::RUNTIME;
use yog_script_ids::clr::INVOKE;
use yog_script_ids::java::{CLASS_LITERAL, NEW, NULL, VAR};
use yog_script_ids::jvm::OBJECT;

impl Gen<'_> {
    pub(crate) fn args(&mut self, args: &[Arg]) -> String {
        let mut out = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let v = match a.pass {
                Some(pass @ (Pass::Ref | Pass::Out)) if !self.runtime_call => {
                    self.cell_arg(&a.value, pass)
                }
                _ => self.handed_expr(&a.value),
            };
            let later_awaits = args[i + 1..]
                .iter()
                .any(|b| crate::machine::awaits(&b.value));
            out.push(self.kept_before(v, &a.value, later_awaits));
        }
        out.join(", ")
    }

    /// `<A, B>` before a generic method's name in its call `e`, when its
    /// type arguments are known: Java can't infer one its arguments don't
    /// name (`Channel.<Integer>CreateBounded(4)`); nothing otherwise.
    pub(crate) fn type_witness(&self, e: &Expr) -> String {
        let Some(used) = self.types.method(e) else {
            return String::new();
        };
        if used.type_args.is_empty()
            || used
                .type_args
                .iter()
                .any(|t| !t.known() || *t == Type::Null || matches!(t, Type::Fn(..)))
        {
            return String::new();
        }
        let args: Vec<String> = used.type_args.iter().map(|t| self.java(t, true)).collect();
        format!("<{}>", args.join(", "))
    }

    /// Call `e`'s arguments: those its `params` parameter takes (the call's
    /// expanded form) packed in an array of its item type.
    pub(crate) fn call_args(&mut self, e: &Expr, args: &[Arg]) -> String {
        let used = self.types.method(e).cloned();
        let Some((from, used)) = used.and_then(|u| u.rest_from.map(|k| (k, u))) else {
            let defaults = self.call_defaults(e, args.len());
            return joined_args(self.args(args), &defaults);
        };
        // Its item type: an argument's as taken, else the array's declared.
        let item = used
            .taken
            .get(from)
            .cloned()
            .or_else(|| match used.params.last() {
                Some(Type::Array(item, _)) => Some((**item).clone()),
                _ => None,
            })
            .unwrap_or(Type::Unknown);
        // An extension's receiver is its first argument, not one written here.
        let from = from - usize::from(used.extension);
        self.packed_args(args, from, &item)
    }

    /// `new` expression `e`'s arguments, its constructor's `params` ones
    /// packed as a call's are.
    pub(crate) fn new_args(&mut self, e: &Expr, args: &[Arg]) -> String {
        match self.types.constructor_rest(e).cloned() {
            Some((from, item)) => self.packed_args(args, from, &item),
            None => {
                let defaults = self.new_defaults(e, args.len());
                joined_args(self.args(args), &defaults)
            }
        }
    }

    /// `args`, those from `from` on packed in an array of `item`.
    fn packed_args(&mut self, args: &[Arg], from: usize, item: &Type) -> String {
        let from = from.min(args.len());
        let head = self.args(&args[..from]);
        let tail = self.args(&args[from..]);
        let java = self.java(item, false);
        // A type parameter's array is made as an object one (Java's).
        let array = if crate::reified::names_parameter(item) {
            format!("({java}[]) {NEW} {OBJECT}[] {{ {tail} }}")
        } else {
            crate::types::made_array(&format!("{java}[]"), &format!("{{ {tail} }}"))
        };
        if head.is_empty() {
            array
        } else {
            format!("{head}, {array}")
        }
    }

    pub(crate) fn member(&mut self, e: &Expr, on: &Expr, name: &str, safe: bool) -> String {
        let on_type = self.types.of(on).cloned();
        let target = self.expr(on);
        let (target, on_type) = self.through_bound(on, target, on_type);
        let read = self.member_of(&target, on_type.as_ref(), name, safe);
        match on_type.as_ref().map(Type::non_null) {
            Some(Type::Yog(_)) => self.viewed(e, read),
            _ => read,
        }
    }

    /// Member `name` of `target` (of type `on_type`) read: a field, a
    /// property's getter, a built-in's companion's, a tuple's item;
    /// `safe` (`?.`): `null` when `target` is.
    pub(crate) fn member_of(
        &mut self,
        target: &str,
        on_type: Option<&Type>,
        name: &str,
        safe: bool,
    ) -> String {
        if let Some(read) = self.nullable_member(target, on_type, name) {
            return read;
        }
        let host = on_type.and_then(|t| self.index.host(t));
        let member = match on_type.map(Type::non_null) {
            Some(Type::Static(of, args))
                if self.index.static_per_type(&(of.clone(), args.len()), name) =>
            {
                return self.per_type_static(of, args, name);
            }
            Some(Type::Static(of, _)) => return self.static_member(of, name),
            // A built-in type's property: its companion's static, the value first.
            Some(t) if let Some(host) = host => {
                let n = Self::ident(name);
                let rank = array_rank(t);
                return self.maybe_null(target, safe, |v| format!("{host}.{n}({v}{rank})"));
            }
            Some(Type::Own(of, args)) if self.index.called(&(of.clone(), args.len()), name) => {
                Some(format!("{}()", Self::ident(name)))
            }
            Some(Type::Yog(_)) => Some(format!("{}()", yog::field(name))),
            Some(Type::Tuple(_, names)) => Some(Self::tuple_field(names, name)),
            _ => None,
        }
        .unwrap_or_else(|| Self::ident(name));
        self.maybe_null(target, safe, |v| format!("{v}.{member}"))
    }

    /// `read(v)` of `target`; `safe` (`?.`): `null` when `target` is.
    fn maybe_null(&mut self, target: &str, safe: bool, read: impl Fn(&str) -> String) -> String {
        if !safe {
            return read(target);
        }
        let t = self.temp();
        self.code.line(format!("{VAR} {t} = {target};"));
        format!("({t} == {NULL} ? {NULL} : {})", read(&t))
    }

    /// `Type.Name`: an enum's case, a static field.
    fn static_member(&self, of: &str, name: &str) -> String {
        let n = Self::ident(name);
        // A property the runtime computes: its getter.
        if self.index.called(&self.index.named(of), name) {
            format!("{}.{n}()", self.own(of))
        } else {
            format!("{}.{n}", self.own(of))
        }
    }

    pub(crate) fn call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> String {
        let called = self.called(e, callee, args);
        let called = self.typed_array(e, called);
        let called = self.reified_result(e, called);
        self.given_back(e, args, called)
    }

    fn called(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> String {
        // `F<T>(…)`: Java infers what's given (a `[Reified]` one's handed).
        let callee = match &callee.kind {
            ExprKind::TypeArgs { expr, .. } => &**expr,
            _ => callee,
        };
        // An event raised: `Joined(p)`, `Joined.Invoke(p)`, `Joined?.Invoke(p)`.
        let raised = match &callee.kind {
            ExprKind::Member { expr, name, .. } if name.text == INVOKE => &**expr,
            _ => callee,
        };
        if let Some(ev) = self.event_of(raised) {
            return self.raise(e, &ev, args);
        }
        if let ExprKind::Member {
            expr,
            name,
            safe: true,
        } = &callee.kind
        {
            return self.safe_call(e, expr, &name.text, args);
        }
        let a = self.call_args(e, args);
        match &callee.kind {
            ExprKind::Ident(name) => match self.types.of(callee) {
                // A delegate held: called by its interface's method.
                t if self.is_delegate(t) => format!("{}.{INVOKE}({a})", self.expr(callee)),
                _ if self.is_result_case(name) => {
                    format!("{NEW} {}.{}<>({a})", self.own(RESULT), Self::ident(name))
                }
                _ if let Some(holder) = self.local_fn_holder(name) => {
                    let on = if holder.is_empty() {
                        String::new()
                    } else {
                        format!("{holder}.")
                    };
                    let a = self.with_classes(e, Vec::new(), a);
                    format!("{on}{}({a})", Self::ident(name))
                }
                _ => {
                    let a = self.with_classes(e, Vec::new(), a);
                    format!("{}{}({a})", self.witnessed(e), self.called_name(e, name))
                }
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

    /// What a call `e` by its name alone is qualified with to hand Java its
    /// type arguments (`W.<List<Integer>>Made()`): its type, `this`, or the
    /// source's class; nothing when none are written.
    fn witnessed(&self, e: &Expr) -> String {
        let witness = self.type_witness(e);
        let Some(used) = self.types.method(e).filter(|_| !witness.is_empty()) else {
            return String::new();
        };
        let qualifier = match &used.owner {
            Some(o)
                if self
                    .index
                    .is_static(&(o.clone(), used.owner_arity), &used.name) =>
            {
                self.index.place(o, used.owner_arity)
            }
            Some(_) => self.this_ref(),
            None => self.class_name.clone(),
        };
        format!("{qualifier}.{witness}")
    }

    /// Call `e`'s arguments: a `[Reified]` method's classes or a generic
    /// one's type arguments first, then
    /// `before` (an extension's receiver), then `args`, written.
    fn with_classes(&mut self, e: &Expr, before: Vec<String>, args: String) -> String {
        let mut all = self.reified_classes(e);
        all.extend(self.handed_types(e));
        all.extend(before);
        if !args.is_empty() {
            all.push(args);
        }
        all.join(", ")
    }

    /// The Java name of the method call `e` chose (`name` in Yog Script):
    /// its own, unless Java needs it named after its type too.
    pub(crate) fn called_name(&self, e: &Expr, name: &str) -> String {
        let java = self
            .types
            .method(e)
            .and_then(|m| Some((m.owner.as_deref()?, m)))
            .filter(|(owner, _)| !self.index.is_host(owner))
            .and_then(|(owner, m)| {
                let owner = (owner.to_string(), m.owner_arity);
                self.index.java_name_of(&owner, &m.name, m.overload)
            });
        Self::ident(java.as_deref().unwrap_or(name))
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
            Some(t @ Type::Array(..)) if generic => self.as_array(called, t),
            _ => called,
        }
    }

    /// A runtime method taking an argument `ref` (`Array.Resize(ref xs,
    /// n)`): Java has no `ref`, so the runtime's gives the new value back
    /// and the place takes it (`xs = Array.Resize(xs, n)`).
    fn given_back(&mut self, e: &Expr, args: &[Arg], called: String) -> String {
        let runtime = self.calls_runtime(e);
        let Some((at, place)) = args
            .iter()
            .enumerate()
            .find(|(_, a)| a.pass == Some(Pass::Ref))
            .filter(|_| runtime)
        else {
            return called;
        };
        // An array of the method's type parameter that may be null given
        // back (`Resize`'s `ref T[]?`): made out of nothing by generic code,
        // so made the place's own array, as a result is.
        let generic = self
            .types
            .method(e)
            .and_then(|m| m.params.get(at))
            .is_some_and(|t| matches!(t, Type::Nullable(inner) if of_type_parameter(inner)));
        let value = match self.types.of(&place.value).map(Type::non_null) {
            Some(t @ Type::Array(..))
                if generic && t.known() && !crate::reified::names_parameter(t) =>
            {
                self.as_array(called, t)
            }
            _ => called,
        };
        format!("{} = {value}", self.expr(&place.value))
    }

    /// Whether call `e`'s method is one the runtime writes by hand.
    pub(crate) fn calls_runtime(&self, e: &Expr) -> bool {
        self.types
            .method(e)
            .and_then(|m| m.owner.as_deref())
            .is_some_and(|o| self.index.is_intrinsic(o))
    }

    /// `called`, an array generic code gave, made Java's array of `t`.
    fn as_array(&self, called: String, t: &Type) -> String {
        // Generic code's arrays are of objects (Java's erased `T[]`): cast only.
        if crate::reified::names_parameter(t) {
            return format!(
                "(({}) {called})",
                crate::reified::erased(&self.java(t, false))
            );
        }
        format!(
            "{}.{}({called}, {}{CLASS_LITERAL})",
            self.array_host(),
            yog_script_ids::jvm::TYPED_ARRAY,
            self.java(t, false)
        )
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
        let target = self.expr(on);
        self.method_call_on(e, on, target, name, a)
    }

    /// Call `e` of method `name` on `on`, its value already `target`.
    pub(crate) fn method_call_on(
        &mut self,
        e: &Expr,
        on: &Expr,
        target: String,
        name: &str,
        a: &str,
    ) -> String {
        let on_type = self.types.of(on).cloned();
        let (target, on_type) = self.through_bound(on, target, on_type);
        let used = self.types.method(e).cloned();
        let owner = used.as_ref().and_then(|u| u.owner.clone());
        let gives = self.types.of(e).cloned();
        if let Some(v) = self.nullable_call((owner.as_deref(), name), &target, a, gives.as_ref()) {
            return v;
        }
        let n = self.called_name(e, name);
        // A number's `GetType()`: its type as declared, read after the
        // value is — Java's box can't tell a `u8` from an `i32`.
        if name == yog_script_ids::members::GET_TYPE
            && let Some(t) = on_type
                .as_ref()
                .filter(|t| crate::types::primitive(t).is_some())
            && let Some(v) = self.value_of(t)
        {
            use yog_script_ids::jvm::{BEFORE, HELPERS};
            return format!("{RUNTIME}.{HELPERS}.{BEFORE}({v}, {target})");
        }
        // An extension method: its type's static, the receiver first.
        if let Some(owner) = self
            .types
            .method(e)
            .filter(|m| m.extension)
            .and_then(|m| m.owner.as_deref())
        {
            let place = self.index.place(owner, 0);
            let a = self.with_classes(e, vec![target], a.to_string());
            return format!("{place}.{n}({a})");
        }
        let a = &self.with_classes(e, Vec::new(), a.to_string());
        match on_type.as_ref().map(Type::non_null) {
            Some(Type::Static(of, _)) => match self.index.kind(&self.index.named(of)) {
                // A case carrying data, made of it (`Mode.Charging(4)`).
                Some(Kind::Enum) if !self.index.case_fields(Some(of), name).is_empty() => {
                    // A generic enum's case: its arguments inferred (`<>`).
                    let diamond = if self.index.named(of).1 > 0 { "<>" } else { "" };
                    format!("{NEW} {}.{n}{diamond}({a})", self.own(of))
                }
                _ => format!("{}.{}{n}({a})", self.own(of), self.type_witness(e)),
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
                    format!("{NEW} {}({target}{sep}{a}).call()", s.record)
                }
                Some(s) => format!("{NEW} {}({a}).call()", s.record),
                None => {
                    self.not_yet("this Yog method", e.span);
                    String::new()
                }
            },
            _ => format!("{target}.{n}({a})"),
        }
    }

    /// `new T(args) { … }` (`e`).
    pub(crate) fn new_expr(&mut self, e: &Expr) -> String {
        let ExprKind::New {
            ty,
            args,
            init,
            items,
        } = &e.kind
        else {
            return String::new();
        };
        let args = args.as_deref().unwrap_or_default();
        self.new_object(e, ty.as_ref(), args, (init, items))
    }

    pub(crate) fn new_object(
        &mut self,
        e: &Expr,
        ty: Option<&Written>,
        args: &[Arg],
        (init, items): (&[(Name, Expr)], &[Stmt]),
    ) -> String {
        let a = self.new_args(e, args);
        let t = match ty {
            Some(t) => self.written(t, true),
            None => self
                .types
                .of(e)
                .map_or_else(|| OBJECT.into(), |t| self.java(t, true)),
        };
        // A handle is its subject: `new World(dim)` is `dim`.
        if matches!(self.types.of(e), Some(Type::Yog(h)) if yog::is_handle(h)) {
            return a;
        }
        let made = match self.types.of(e).cloned() {
            Some(of) if let Some(made) = self.param_made(&of) => made,
            Some(of) => self.new_of(&of, &t, &a),
            None => format!("{NEW} {t}({a})"),
        };
        if init.is_empty() && items.is_empty() {
            return made;
        }
        let tmp = self.temp();
        self.code.line(format!("{VAR} {tmp} = {made};"));
        for (field, v) in init {
            let v = self.handed_expr(v);
            self.code
                .line(format!("{tmp}.{} = {v};", Self::ident(&field.text)));
        }
        // A collection's items: statements on it, `$made` naming it.
        self.renamed.push((MADE.to_string(), tmp.clone()));
        items.iter().for_each(|s| self.stmt(s));
        self.renamed.pop();
        tmp
    }
}

impl Gen<'_> {
    /// Whether a value of `t` is a delegate: a lambda's or a method's own
    /// type, or a declared delegate's.
    fn is_delegate(&self, t: Option<&Type>) -> bool {
        match t.map(Type::non_null) {
            Some(Type::Fn(..)) => true,
            Some(Type::Own(name, args)) => {
                self.index.kind(&(name.clone(), args.len())) == Some(Kind::Delegate)
            }
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

/// Arguments `written` and then `more`, as Java lists them.
fn joined_args(written: String, more: &[String]) -> String {
    let mut all: Vec<String> = Vec::new();
    if !written.is_empty() {
        all.push(written);
    }
    all.extend(more.iter().cloned());
    all.join(", ")
}
