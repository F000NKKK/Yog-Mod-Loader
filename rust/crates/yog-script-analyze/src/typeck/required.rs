//! `new T(args) { init }`'s initializer: each member it sets fitting its
//! type, and every `required` member of `T` (and what it inherits) among
//! them — unless the constructor called is `[SetsRequiredMembers]`
//! (C#'s CS9035).

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use super::own::TypeKey;
use crate::ast::{Expr, Name};

impl Checker<'_, '_, '_> {
    /// `init` of `new t(…)` (expression `e`): each value checked, the
    /// `required` members left out said.
    pub(super) fn initialized(&mut self, e: &Expr, t: &Type, init: &[(Name, Expr)]) {
        for (name, v) in init {
            let wanted = self.own_member(t, &name.text);
            if wanted.known() {
                self.fits(v, &wanted);
            } else {
                self.expr(v);
            }
        }
        let Type::Own(name, args) = t.non_null() else {
            return;
        };
        let key = TypeKey::new(name, args.len());
        if self.sets_required(e, &key) {
            return;
        }
        let missing: Vec<String> = self
            .own
            .lineage(&key)
            .iter()
            .filter_map(|k| self.own.of_key(k))
            .flat_map(|o| o.required.iter())
            .filter(|r| !init.iter().any(|(n, _)| n.text == **r))
            .map(|r| format!("`{r}`"))
            .collect();
        if !missing.is_empty() {
            self.report.error(
                script_problem::REQUIRED,
                e.span,
                format!(
                    "a new `{name}` sets its required {}: `new {name} {{ … }}`",
                    missing.join(", ")
                ),
            );
        }
    }

    /// Whether the constructor `new` expression `e` calls (of type `key`)
    /// is `[SetsRequiredMembers]`.
    fn sets_required(&self, e: &Expr, key: &TypeKey) -> bool {
        let Some(own) = self.own.of_key(key) else {
            return false;
        };
        let chosen = self
            .types
            .constructor(e)
            .or_else(|| own.ctors.iter().position(Vec::is_empty));
        chosen.is_some_and(|i| own.ctor_sets_required.get(i).copied().unwrap_or(false))
    }
}
