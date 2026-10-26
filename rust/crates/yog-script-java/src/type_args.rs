//! Generics as C#'s, kept where Java's forget them: a type parameter's
//! value wherever code reaches it — a generic type's, the arguments its
//! object was made with (`Type.argument(this, Box.class, 0)`); a generic
//! method's, handed to it before its arguments (`Type $typeT`) by every
//! call. A constructor's code and a static member's have no object to
//! read them off: there they're unknown, as before.

use yog_script_analyze::MethodUse;
use yog_script_analyze::Type;
use yog_script_analyze::ast::{Decl, DeclKind, Expr, ExprKind, Method, Modifier};
use yog_script_ids::java::CLASS_LITERAL;
use yog_script_ids::jvm::reflection::{ARGUMENT_OF, TYPE, TYPE_ARG};

use super::Gen;

impl Gen<'_> {
    /// Type parameter `name`'s value where the code being written reaches it.
    pub(crate) fn type_param(&self, name: &str) -> Option<String> {
        self.type_params
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }

    /// `f`, written with `params` (each a name and its value) in reach.
    pub(crate) fn with_type_params<R>(
        &mut self,
        params: Vec<(String, String)>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.type_params.len();
        self.type_params.extend(params);
        let out = f(self);
        self.type_params.truncate(depth);
        out
    }

    /// The type parameters member `d` reaches through its object: its
    /// generic type's, when it's an instance method, property or indexer.
    pub(crate) fn object_type_params(&self, d: &Decl) -> Vec<(String, String)> {
        if let Some(handed) = self.static_type_params(d) {
            return handed;
        }
        let reads_object = matches!(
            d.kind,
            DeclKind::Method(_) | DeclKind::Property { .. } | DeclKind::Indexer(_)
        ) && !d.has(Modifier::Static);
        let Some((owner, arity)) = self.owner.clone().filter(|_| reads_object) else {
            return Vec::new();
        };
        let class = format!("{}{CLASS_LITERAL}", self.index.place(&owner, arity));
        self.owner_generics
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let v = format!("{TYPE}.{ARGUMENT_OF}({}, {class}, {i})", self.this_ref());
                (g.clone(), v)
            })
            .collect()
    }

    /// A static method of an own generic type: its type's parameters, each
    /// handed first as a `Type` parameter (`$typeT`), as a generic
    /// method's own are; none for another member.
    pub(crate) fn static_type_params(&self, d: &Decl) -> Option<Vec<(String, String)>> {
        let DeclKind::Method(m) = &d.kind else {
            return None;
        };
        let owner = self.owner.as_ref()?;
        if !self.index.static_taking_types(owner, &m.name.text) || !d.has(Modifier::Static) {
            return None;
        }
        Some(
            self.owner_generics
                .iter()
                .map(|g| (g.clone(), format!("{TYPE_ARG}{g}")))
                .collect(),
        )
    }

    /// Method `m`'s own type parameters, each handed as a `Type` parameter
    /// (`$typeT`): their names and values.
    pub(crate) fn method_type_params(m: &Method) -> Vec<(String, String)> {
        m.generics
            .iter()
            .filter(|g| !g.lifetime)
            .map(|g| (g.name.text.clone(), format!("{TYPE_ARG}{}", g.name.text)))
            .collect()
    }

    /// Method `m`'s Java parameters: its type parameters' first.
    pub(crate) fn method_params(&mut self, m: &Method) -> String {
        let mut all: Vec<String> = Self::method_type_params(m)
            .into_iter()
            .map(|(_, v)| format!("{TYPE} {v}"))
            .collect();
        let params = self.params(&m.params);
        if !params.is_empty() {
            all.push(params);
        }
        all.join(", ")
    }

    /// A static method of an own generic type called by `e`: its type's
    /// arguments as written (`Box<string>.Of()`), or as the code reaches
    /// them (`Of()` in its type: its own `T`); none for another call.
    fn class_types_handed(&mut self, e: &Expr, used: &MethodUse) -> Vec<String> {
        let Some(owner) = used.owner.clone() else {
            return Vec::new();
        };
        let key = (owner, used.owner_arity);
        if !self.index.static_taking_types(&key, &used.name) {
            return Vec::new();
        }
        let ExprKind::Call { callee, .. } = &e.kind else {
            return Vec::new();
        };
        let callee = match &callee.kind {
            ExprKind::TypeArgs { expr, .. } => &**expr,
            _ => &**callee,
        };
        let args: Vec<Option<String>> = match &callee.kind {
            ExprKind::Member { expr, .. } => match self.types.of(expr).cloned() {
                Some(Type::Static(_, args)) => args.iter().map(|a| self.value_of(a)).collect(),
                _ => Vec::new(),
            },
            _ => self
                .owner_generics
                .clone()
                .iter()
                .map(|g| self.type_param(g))
                .collect(),
        };
        args.into_iter()
            .map(|v| v.unwrap_or_else(|| yog_script_ids::java::NULL.into()))
            .collect()
    }

    /// Whether `used` is handed its type arguments: a generic method this
    /// compiler writes (not the runtime's by hand, not a built-in's).
    pub(crate) fn takes_types(&self, used: &MethodUse) -> bool {
        used.generics > 0
            && !used
                .owner
                .as_deref()
                .is_some_and(|o| self.index.is_intrinsic(o) || self.index.is_host(o))
    }

    /// The type arguments call `e` hands its method first, when it takes
    /// them: a static method of an own generic type its type's, then a
    /// generic method its own.
    pub(crate) fn handed_types(&mut self, e: &Expr) -> Vec<String> {
        let Some(used) = self.types.method(e).cloned() else {
            return Vec::new();
        };
        let mut out = self.class_types_handed(e, &used);
        if !self.takes_types(&used) {
            return out;
        }
        for t in &used.type_args {
            match self.value_of(t) {
                Some(v) => out.push(v),
                None => {
                    self.not_yet("a type argument not known here", e.span);
                    return Vec::new();
                }
            }
        }
        out
    }
}
