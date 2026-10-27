//! Indexing (`x[i]`): a call of the indexer the indexed type declares or
//! inherits (`this[i32 i]`), its parameters matched as a method's are.

use yog_ids::script_problem;

use super::Checker;
use super::members::Callee;
use super::model::Type;
use crate::ast::{Arg, Expr, ExprKind};
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// `on[args]`: an array's item, or what the indexer gives.
    pub(super) fn indexed(&mut self, on: Type, index: &[Expr], at: Span) -> Type {
        let outer = std::mem::replace(&mut self.indexing, true);
        let t = self.indexed_in(on, index, at);
        self.indexing = outer;
        t
    }

    fn indexed_in(&mut self, on: Type, index: &[Expr], at: Span) -> Type {
        // `xs[a..b]`: a slice, of `xs`'s own kind.
        if let [
            range @ Expr {
                kind: ExprKind::Range { .. },
                ..
            },
        ] = index
        {
            return self.sliced(on, range, at);
        }
        if let Type::Array(item, rank) = on {
            self.array_index(index, rank, at);
            return *item;
        }
        let on = self.hosted(&on).unwrap_or(on);
        let Some(callee) = self.indexer(&on) else {
            index.iter().for_each(|i| {
                self.expr(i);
            });
            return Type::Unknown;
        };
        let given: Vec<Type> = index.iter().map(|i| self.expr(i)).collect();
        let args: Vec<Arg> = index
            .iter()
            .map(|i| Arg {
                name: None,
                pass: None,
                value: i.clone(),
            })
            .collect();
        self.called(callee, &given, &args, at).0
    }

    /// `on[range]`: an array's, a string's or a list's items in it, of its kind.
    fn sliced(&mut self, on: Type, range: &Expr, at: Span) -> Type {
        let ExprKind::Range { from, to, .. } = &range.kind else {
            return Type::Unknown;
        };
        for end in [from, to] {
            let t = self.expr(end);
            self.want(&t, &Type::I32, end.span);
        }
        let sliceable = match on.non_null() {
            Type::Array(_, 1) | Type::String => true,
            Type::Own(name, args) => name == yog_script_ids::system::LIST && args.len() == 1,
            _ => !on.known(),
        };
        if !sliceable {
            self.report.error(
                script_problem::TYPE_MISMATCH,
                at,
                format!("a `{on}` isn't sliced: an array, a string or a `List<T>` is"),
            );
        }
        on.non_null().clone()
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
                Some((self.own.of_key(&key)?, self.context_of(&key.name, &args)))
            })
            .flat_map(|(t, context)| {
                t.indexers
                    .iter()
                    .map(move |i| (i.get.clone(), context.clone()))
            })
            .collect();
        (!getters.is_empty()).then_some(Callee::Own(getters))
    }
}
