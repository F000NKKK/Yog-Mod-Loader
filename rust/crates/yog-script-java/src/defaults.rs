//! Arguments left out for parameters with defaults (C#'s optional
//! parameters): each given its default at the call, as C# compiles it —
//! a constant (`0`, `"x"`, `null`, `LogLevel.None`, `default`).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, ExprKind, Param};

use crate::Gen;
use crate::exprs::zero;

impl Gen<'_> {
    /// The defaults call `e` (giving `given` arguments) leaves to its
    /// method's parameters, written.
    pub(crate) fn call_defaults(&mut self, e: &Expr, given: usize) -> Vec<String> {
        let Some(used) = self.types.method(e).cloned() else {
            return Vec::new();
        };
        if used.rest_from.is_some() || given >= used.params.len() {
            return Vec::new();
        }
        let owner = used.owner.map(|o| (o, used.owner_arity));
        let params = self
            .index
            .method_params(owner.as_ref(), &used.name, used.overload)
            .map(<[Param]>::to_vec)
            .unwrap_or_default();
        let given = given + usize::from(used.extension);
        self.defaults(&params, given, &used.params)
    }

    /// The defaults `new` expression `e` (giving `given` arguments) leaves
    /// to its constructor's parameters, written.
    pub(crate) fn new_defaults(&mut self, e: &Expr, given: usize) -> Vec<String> {
        let Some(Type::Own(name, args)) = self.types.of(e).cloned() else {
            return Vec::new();
        };
        let t = (name, args.len());
        // A struct's `new` isn't chosen by the checker: the first constructor
        // taking as many, the rest defaulted.
        let index = self.types.constructor(e).or_else(|| {
            (0..)
                .map_while(|i| self.index.ctor_params(&t, i))
                .position(|ps| {
                    let required = ps.iter().take_while(|p| p.default.is_none()).count();
                    (required..=ps.len()).contains(&given)
                })
        });
        let Some(index) = index else {
            return Vec::new();
        };
        let params = self
            .index
            .ctor_params(&t, index)
            .map(<[Param]>::to_vec)
            .unwrap_or_default();
        self.defaults(&params, given, &[])
    }

    /// `params`' defaults from the `given`-th on (`typed`: their types, when known).
    fn defaults(&mut self, params: &[Param], given: usize, typed: &[Type]) -> Vec<String> {
        params
            .iter()
            .enumerate()
            .skip(given)
            .map_while(|(i, p)| {
                let d = p.default.as_ref()?;
                Some(self.constant_value(d, typed.get(i)))
            })
            .collect()
    }

    /// Constant `e` (a parameter's default, an attribute's argument), of
    /// type `t` when known: an enum's case by its type's place where the
    /// code isn't typed (a library's declaration), `default` its type's zero.
    pub(crate) fn constant_value(&mut self, e: &Expr, t: Option<&Type>) -> String {
        match &e.kind {
            ExprKind::Member { expr, name, .. }
                if let ExprKind::Ident(of) = &expr.kind
                    && self.types.of(expr).is_none() =>
            {
                // A built-in's by its `System` type (`i32.MaxValue` is `Int32`'s).
                let of = yog_script_analyze::builtin_type(of)
                    .as_ref()
                    .and_then(yog_script_analyze::system_name)
                    .unwrap_or(of);
                if !self.index.types.contains_key(&self.index.named(of)) {
                    return self.expr(e);
                }
                format!("{}.{}", self.index.place_named(of), Self::ident(&name.text))
            }
            ExprKind::Default(_) => t.map_or(yog_script_ids::java::NULL, zero).to_string(),
            _ => self.expr(e),
        }
    }
}
