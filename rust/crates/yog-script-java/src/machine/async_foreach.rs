//! `await foreach (let x in items) body` in the machine, as C#'s: the
//! sequence's walk made, each `MoveNextAsync` awaited, its `Current` the
//! item, the walk let go of (`DisposeAsync`, awaited) once it ends.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, Name, Stmt};
use yog_script_ids::java::{IF, RETURN};
use yog_script_ids::jvm::tasks::{CHECK, LOCAL_FIELD, RESULT, STATE, SUSPEND};
use yog_script_ids::members::{CURRENT, DISPOSE_ASYNC, GET_ASYNC_ENUMERATOR, MOVE_NEXT_ASYNC};
use yog_script_ids::system::{ASYNC_ENUMERATOR, TASK};

use super::flow::Loop;
use crate::Gen;

impl Gen<'_> {
    pub(super) fn split_await_foreach(&mut self, n: &Name, iter: &Expr, body: &Stmt) {
        if self.await_unwritable(iter) {
            return;
        }
        let item_t = self.types.item(iter).cloned().unwrap_or(Type::Unknown);
        let item = format!("{}{LOCAL_FIELD}{}", Self::ident(&n.text), self.new_state());
        self.declare_field(&self.java(&item_t, false), &item);
        let walk_t = Type::Own(ASYNC_ENUMERATOR.to_string(), vec![item_t]);
        let seq = self.expr(iter);
        let walk = self.field_of_type(Some(&walk_t), &format!("{seq}.{GET_ASYNC_ENUMERATOR}()"));
        let depth = self.renamed.len();
        self.renamed.push((n.text.clone(), item.clone()));
        let l = Loop {
            again: self.new_state(),
            past: self.new_state(),
        };
        self.state_starts(l.again);
        let moved_t = Type::Own(TASK.to_string(), vec![Type::Bool]);
        let moved = self.awaited_field(&moved_t, &format!("{walk}.{MOVE_NEXT_ASYNC}()"));
        self.code
            .line(format!("{IF} (!{moved}.{RESULT}()) {}", Self::jump(l.past)));
        self.code.line(format!("{item} = {walk}.{CURRENT}();"));
        self.split_body(&l, body);
        self.code.line(Self::jump(l.again));
        self.state_starts(l.past);
        self.renamed.truncate(depth);
        let done_t = Type::Own(TASK.to_string(), Vec::new());
        let done = self.awaited_field(&done_t, &format!("{walk}.{DISPOSE_ASYNC}()"));
        self.code.line(format!("{done}.{CHECK}();"));
    }

    /// Task `task` (of `t`) kept in a field and awaited: the machine leaving
    /// until it's done, then going on in a state of its own.
    fn awaited_field(&mut self, t: &Type, task: &str) -> String {
        let field = self.field_of_type(Some(t), task);
        let state = self.new_state();
        self.code.line(format!("{STATE} = {state};"));
        self.code
            .line(format!("{IF} ({SUSPEND}({field})) {RETURN};"));
        self.state_starts(state);
        field
    }
}
