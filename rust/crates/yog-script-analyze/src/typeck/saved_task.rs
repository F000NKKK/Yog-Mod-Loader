//! `saved async` methods (docs/design/storage.md S9): a method whose wait
//! is saved with its owner's state, picked up again when the world loads.
//! What it waits for has to be waited for again after a restart: only a
//! delay (`Task.Delay`, `GameTime.Delay`, `GameTime.NextTick`), which goes
//! on with the time it had left.

use yog_ids::script_problem;
use yog_script_ids::members::{DELAY, NEXT_TICK};
use yog_script_ids::system::{GAME_TIME, TASK};

use super::Checker;
use crate::ast::{Decl, Expr, ExprKind, Method, MethodBody, Modifier, Stmt, UnOp};
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// Method `m` (`d`), when it's `saved`: `async`, of an owner of state,
    /// waiting only for delays.
    pub(super) fn saved_task(&mut self, d: &Decl, m: &Method) {
        if !d.has(Modifier::Saved) {
            return;
        }
        if !d.has(Modifier::Async) {
            self.report.error(
                script_problem::SAVED_TASK,
                m.name.span,
                format!("`{}` is saved but not `async`: what a save keeps of a method is where it waits", m.name.text),
            );
        }
        if !self.state_owner {
            self.report.error(
                script_problem::STATE_OWNER,
                m.name.span,
                format!("`{}` keeps a wait with no owner: it goes on a game's object (`extend Player`), content, a component or the mod", m.name.text),
            );
        }
        let mut others = Vec::new();
        match &m.body {
            Some(MethodBody::Block(block)) => block
                .stmts
                .iter()
                .for_each(|s| awaits_in_stmt(s, &mut others)),
            Some(MethodBody::Expr(e)) => awaits_in(e, &mut others),
            None => {}
        }
        for span in others {
            self.report.error(
                script_problem::SAVED_TASK,
                span,
                "a saved method waits only for a delay (`Task.Delay`, `GameTime.Delay`, `GameTime.NextTick`): after a restart nothing else can be waited for again",
            );
        }
    }
}

/// The spans of each `await` in `s` of something that isn't a delay.
fn awaits_in_stmt(s: &Stmt, out: &mut Vec<Span>) {
    s.each_expr(&mut |e| awaits_in(e, out));
}

/// The spans of each `await` in `e` (and what it holds) of something that isn't a delay.
fn awaits_in(e: &Expr, out: &mut Vec<Span>) {
    if let ExprKind::Unary {
        op: UnOp::Await,
        expr,
    } = &e.kind
        && !is_delay(expr)
    {
        out.push(e.span);
    }
    e.each_child(|c| awaits_in(c, out));
}

/// Whether `e` is a delay a save can wait for again: `Task.Delay(…)`,
/// `GameTime.Delay(…)`, `GameTime.NextTick()`.
fn is_delay(e: &Expr) -> bool {
    let ExprKind::Call { callee, .. } = &e.kind else {
        return false;
    };
    let ExprKind::Member { expr, name, .. } = &callee.kind else {
        return false;
    };
    let ExprKind::Ident(owner) = &expr.kind else {
        return false;
    };
    match owner.as_str() {
        TASK => name.text == DELAY,
        GAME_TIME => name.text == DELAY || name.text == NEXT_TICK,
        _ => false,
    }
}
