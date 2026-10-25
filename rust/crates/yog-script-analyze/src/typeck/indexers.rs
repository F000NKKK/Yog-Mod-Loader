//! Indexing (`x[i]`): a call of the indexer the indexed type declares or
//! inherits (`this[i32 i]`), its parameters matched as a method's are.

use yog_ids::script_problem;

use super::Checker;
use super::members::Callee;
use super::model::Type;
use crate::ast::{Arg, Expr};
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// `on[index]`: what the indexer gives.
    pub(super) fn indexed(&mut self, on: Type, index: &Expr, at: Span) -> Type {
        match on {
            Type::List(item) => {
                self.expr(index);
                return *item;
            }
            Type::Map(_, value) => {
                self.expr(index);
                return *value;
            }
            _ => {}
        }
        let Some(callee) = self.indexer(&on) else {
            self.expr(index);
            return Type::Unknown;
        };
        let given = vec![self.expr(index)];
        let args = [Arg {
            name: None,
            pass: None,
            value: index.clone(),
        }];
        self.called(callee, &given, &args, at)
    }

    /// `on[…] = v` where `on`'s indexers are all `get`-only: said so.
    pub(super) fn index_written(&mut self, on: &Expr, at: Span) {
        let Some(Type::Own(type_name, _)) = self.types.of(on).map(Type::non_null).cloned() else {
            return;
        };
        let args = self.types.of(on).map(Type::non_null).map(|t| match t {
            Type::Own(_, a) => a.clone(),
            _ => Vec::new(),
        });
        let indexers: Vec<bool> = self
            .lineage_of(&type_name, &args.unwrap_or_default())
            .iter()
            .filter_map(|(key, _)| self.own.of_key(key))
            .flat_map(|t| t.indexers.iter().map(|i| i.settable))
            .collect();
        if !indexers.is_empty() && !indexers.contains(&true) {
            self.report.error(
                script_problem::ASSIGN_TO_CONSTANT,
                at,
                format!(
                    "`{type_name}`'s indexer has no `set`: its items are read here, never written"
                ),
            );
        }
    }

    /// The indexers of `on`'s type and of what it inherits, as a callee.
    fn indexer(&self, on: &Type) -> Option<Callee> {
        let Type::Own(type_name, args) = on.non_null() else {
            return None;
        };
        let getters: Vec<_> = self
            .lineage_of(type_name, args)
            .into_iter()
            .filter_map(|(key, args)| {
                Some((self.own.of_key(&key)?, self.bound_of(&key.name, &args)))
            })
            .flat_map(|(t, bound)| {
                t.indexers
                    .iter()
                    .map(move |i| (i.get.clone(), bound.clone()))
            })
            .collect();
        (!getters.is_empty()).then_some(Callee::Own(getters))
    }
}
