//! What the borrow checker knows at a point of the code: each local's and
//! parameter's state, innermost scope last; whether the point is reached.

use std::collections::HashMap;

use crate::ast::Pass;
use crate::source::token::Span;

/// A local or a parameter, as the code has left it.
#[derive(Clone, Default)]
pub(super) struct Var {
    /// How a parameter was lent (`in`, `ref`): not the method's to move.
    pub lent: Option<Pass>,
    /// Where `move` handed it on, on some path to here.
    pub moved: Option<Span>,
}

#[derive(Clone)]
pub(super) struct State {
    scopes: Vec<HashMap<String, Var>>,
    /// Every path here has returned: nothing after it runs.
    pub dead: bool,
}

impl State {
    pub fn new() -> Self {
        State {
            scopes: vec![HashMap::new()],
            dead: false,
        }
    }

    pub fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        self.scopes.pop();
    }

    pub fn declare(&mut self, name: &str, var: Var) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), var);
        }
    }

    pub fn get(&self, name: &str) -> Option<&Var> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Var> {
        self.scopes.iter_mut().rev().find_map(|s| s.get_mut(name))
    }

    /// Where two paths join: a value moved on either may be moved; a path
    /// that returned brings nothing.
    pub fn join(self, other: State) -> State {
        if self.dead {
            return other;
        }
        if other.dead {
            return self;
        }
        let scopes = self
            .scopes
            .into_iter()
            .zip(other.scopes)
            .map(|(mut mine, theirs)| {
                for (name, var) in theirs {
                    if let Some(m) = mine.get_mut(&name) {
                        m.moved = m.moved.or(var.moved);
                    }
                }
                mine
            })
            .collect();
        State {
            scopes,
            dead: false,
        }
    }
}
