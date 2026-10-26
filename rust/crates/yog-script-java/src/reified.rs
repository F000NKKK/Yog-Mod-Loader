//! A `[Reified]` runtime method's call: its type parameters handed first,
//! as Java's classes (`int.class` for an `i32`: what an array of it holds),
//! its result made the call's own type — Java's generics forget them.

use yog_script_analyze::Type;
use yog_script_analyze::ast::Expr;
use yog_script_ids::jvm::OBJECT;

use super::Gen;

/// What names a Java class as a value.
const CLASS: &str = ".class";

impl Gen<'_> {
    /// The classes call `e`'s method is handed first, when it's `[Reified]`.
    pub(crate) fn reified_classes(&mut self, e: &Expr) -> Vec<String> {
        let Some(used) = self.types.method(e).cloned() else {
            return Vec::new();
        };
        let reified = used.owner.as_deref().is_some_and(|o| {
            let owner = (o.to_string(), used.owner_arity);
            self.index.is_reified(&owner, &used.name, used.overload)
        });
        if !reified {
            return Vec::new();
        }
        let mut classes = Vec::new();
        for t in &used.type_args {
            match self.class_literal(t) {
                Some(c) => classes.push(c),
                None => {
                    self.not_yet(
                        "a type known only where a generic method runs, handed on",
                        e.span,
                    );
                }
            }
        }
        classes
    }

    /// Call `e` of a `[Reified]` method (`called`, written): made its own
    /// type, its result's generics Java can't see.
    pub(crate) fn reified_result(&self, e: &Expr, called: String) -> String {
        let reified = self
            .types
            .method(e)
            .and_then(|m| Some((m.owner.as_deref()?, m)))
            .is_some_and(|(o, m)| {
                let owner = (o.to_string(), m.owner_arity);
                self.index.is_reified(&owner, &m.name, m.overload)
            });
        match self.types.of(e) {
            Some(t) if reified && t.known() => {
                format!("(({}) ({OBJECT}) {called})", self.java(t, false))
            }
            _ => called,
        }
    }

    /// `t` as Java's class value (`String.class`, `int[].class`), when it
    /// names no type parameter.
    fn class_literal(&self, t: &Type) -> Option<String> {
        if !t.known() || names_parameter(t) {
            return None;
        }
        Some(format!("{}{CLASS}", erased(&self.java(t, false))))
    }
}

/// Whether `t` names a type parameter somewhere.
fn names_parameter(t: &Type) -> bool {
    match t {
        Type::Param(_) => true,
        Type::Own(_, args) | Type::Tuple(args, _) => args.iter().any(names_parameter),
        Type::Array(item, _) | Type::Nullable(item) => names_parameter(item),
        _ => false,
    }
}

/// A Java type without its type arguments (`List<String>[]` is `List[]`).
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
