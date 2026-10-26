//! Statements: locals, assignments, branches and loops.

use yog_script_analyze::ast::{Binding, Block, Expr, ExprKind, Stmt};

use super::{Interp, Run, Stop};
use crate::value::Value;

impl<'p> Interp<'p> {
    pub(crate) fn block(&mut self, b: &Block) -> Run<()> {
        self.scoped(|i| b.stmts.iter().try_for_each(|s| i.stmt(s)))
    }

    pub(crate) fn stmt(&mut self, s: &Stmt) -> Run<()> {
        match s {
            Stmt::Local { binding, value, .. } => {
                let v = match value {
                    Some(e) => self.eval(e)?.copied(),
                    None => Value::Null,
                };
                self.bind_binding(binding, v);
            }
            Stmt::Assign { place, op, value } => {
                let mut v = self.eval(value)?.copied();
                if let Some(op) = op {
                    let now = self.eval(place)?;
                    v = self.binary_values(*op, now, v, place.span)?;
                }
                self.assign(place, v)?;
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                if self.eval(cond)?.truthy() {
                    self.scoped(|i| i.stmt(then))?;
                } else if let Some(o) = otherwise {
                    self.scoped(|i| i.stmt(o))?;
                }
            }
            Stmt::Foreach {
                binding,
                iter,
                body,
                ..
            } => {
                let items = self.items(iter)?;
                for item in items {
                    let turn = self.scoped(|i| {
                        i.bind_binding(binding, item);
                        i.stmt(body)
                    });
                    if !keep_looping(turn)? {
                        break;
                    }
                }
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => {
                self.scoped(|i| i.for_loop(init.as_deref(), cond.as_ref(), step.as_deref(), body))?
            }
            Stmt::While { cond, body } => {
                while self.eval(cond)?.truthy() {
                    if !keep_looping(self.scoped(|i| i.stmt(body)))? {
                        break;
                    }
                }
            }
            Stmt::DoWhile { body, cond } => loop {
                if !keep_looping(self.scoped(|i| i.stmt(body)))? || !self.eval(cond)?.truthy() {
                    break;
                }
            },
            _ => self.leaf(s)?,
        }
        Ok(())
    }

    fn leaf(&mut self, s: &Stmt) -> Run<()> {
        match s {
            Stmt::Block(b) | Stmt::Unsafe(b, _) => self.block(b),
            Stmt::Break(_) => Err(Stop::Break),
            Stmt::Continue(_) => Err(Stop::Continue),
            Stmt::Return(value, _) | Stmt::Skip(value, _) => {
                let v = match value {
                    Some(e) => self.eval(e)?,
                    None => Value::Void,
                };
                Err(Stop::Return(v))
            }
            Stmt::Yield(Some(e), at) => {
                let v = self.eval(e)?;
                self.yield_item(v, *at)
            }
            Stmt::Yield(None, _) => Err(Stop::Return(Value::Void)),
            // Threads are the JVM's and the native's: the interpreter has none.
            Stmt::Lock { at, .. } => self.panic(
                "a `lock` isn't run by the interpreter: it has no threads",
                *at,
            ),
            Stmt::Expr(e) => self.eval(e).map(drop),
            // A block lambda's last value: what it gives.
            Stmt::Value(e) => Err(Stop::Return(self.eval(e)?)),
            _ => Ok(()),
        }
    }

    fn for_loop(
        &mut self,
        init: Option<&Stmt>,
        cond: Option<&Expr>,
        step: Option<&Stmt>,
        body: &Stmt,
    ) -> Run<()> {
        if let Some(i) = init {
            self.stmt(i)?;
        }
        while match cond {
            Some(c) => self.eval(c)?.truthy(),
            None => true,
        } {
            if !keep_looping(self.scoped(|i| i.stmt(body)))? {
                break;
            }
            if let Some(s) = step {
                self.stmt(s)?;
            }
        }
        Ok(())
    }

    /// What a `foreach` walks: a list's items, a range's numbers, a
    /// string's characters.
    fn items(&mut self, iter: &Expr) -> Run<Vec<Value>> {
        Ok(match self.eval(iter)? {
            Value::List(items) => items.borrow().clone(),
            Value::Range(a, b, inclusive) => {
                let end = if inclusive { b } else { b - 1 };
                (a..=end).map(Value::Int).collect()
            }
            Value::Str(s) => s.chars().map(Value::Char).collect(),
            other => return self.panic(format!("`{other}` can't be walked"), iter.span),
        })
    }

    fn bind_binding(&mut self, b: &Binding, v: Value) {
        match b {
            Binding::One(n) => self.bind(&n.text, v),
            Binding::Tuple(names) => {
                let items = match &v {
                    Value::List(items) => items.borrow().clone(),
                    _ => Vec::new(),
                };
                for (k, n) in names.iter().enumerate() {
                    self.bind(&n.text, items.get(k).cloned().unwrap_or(Value::Null));
                }
            }
        }
    }

    /// `place = v`: a local, a field of `this` by its name, an object's
    /// field, a list's element.
    pub(crate) fn assign(&mut self, place: &Expr, v: Value) -> Run<()> {
        match &place.kind {
            ExprKind::Ident(name) => {
                if !self.set_local(name, v.clone()) {
                    let this = self.this.clone();
                    self.set_field(this.as_ref(), name, v, place.span)?;
                }
                Ok(())
            }
            ExprKind::Member { expr, name, .. } => {
                let on = self.eval(expr)?;
                self.set_field(Some(&on), &name.text, v, place.span)
            }
            ExprKind::Index { expr, args } => {
                let Some((last, rows)) = args.split_last() else {
                    return self.panic("an index is wanted", place.span);
                };
                let mut on = self.eval(expr)?;
                for row in rows {
                    on = self.index(on, row, expr)?;
                }
                let at = self.eval(last)?;
                match (on, at) {
                    (Value::List(items), Value::Int(k)) => {
                        let mut items = items.borrow_mut();
                        match usize::try_from(k).ok().and_then(|k| items.get_mut(k)) {
                            Some(slot) => *slot = v,
                            None => {
                                return self.panic(format!("index {k} out of range"), place.span);
                            }
                        }
                        Ok(())
                    }
                    _ => self.panic("only a list's element is set by index", place.span),
                }
            }
            _ => self.panic("this can't be assigned", place.span),
        }
    }
}

/// After a loop's turn: whether to go on (`break` ends it; `continue` and
/// the end of the body go on; a return or a panic goes up).
fn keep_looping(turn: Run<()>) -> Run<bool> {
    match turn {
        Ok(()) | Err(Stop::Continue) => Ok(true),
        Err(Stop::Break) => Ok(false),
        Err(other) => Err(other),
    }
}
