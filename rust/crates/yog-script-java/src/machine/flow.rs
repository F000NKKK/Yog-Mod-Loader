//! The machine's code: what no `await` splits written as any, what one
//! does in states — an `if` jumping past its branch, a loop jumping back
//! to its top, its `break` and `continue` jumps too. A local of the code
//! split is a field (`x$v1`): it lives across a wait.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Binding, Block, Expr, ExprKind, Stmt};
use yog_script_ids::java::{CHAR, IF, INT, RANK};
use yog_script_ids::jvm::tasks::LOCAL_FIELD;
use yog_script_ids::jvm::{HAS_NEXT, ITERATOR, ITERATOR_OF, LENGTH, NEXT, OBJECT, TO_CHAR_ARRAY};

use super::{awaits, stmt_awaits};
use crate::Gen;

/// Where a loop split by an `await` goes: its top (or its step), past it.
struct Loop {
    again: usize,
    past: usize,
}

impl Gen<'_> {
    /// A block of the machine's code, its locals fields.
    pub(crate) fn machine_block(&mut self, b: &Block) {
        self.machine_stmts(&b.stmts, false);
    }

    /// Statements of the machine's code, its locals fields; the last
    /// expression without `;` (a lambda's) what it finishes with when it
    /// gives a value (`gives_value`).
    pub(crate) fn machine_stmts(&mut self, stmts: &[Stmt], gives_value: bool) {
        let depth = self.renamed.len();
        for (i, s) in stmts.iter().enumerate() {
            match s {
                Stmt::Value(v) if gives_value && i + 1 == stmts.len() => {
                    if !self.await_unwritable(v) {
                        let v = self.handed_expr(v);
                        self.async_return(Some(v));
                    }
                }
                s => self.machine_stmt(s),
            }
        }
        self.renamed.truncate(depth);
    }

    /// A statement of the machine's code.
    fn machine_stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local {
                ty,
                binding: Binding::One(n),
                value,
                ..
            } => {
                if value.as_ref().is_some_and(|v| self.await_unwritable(v)) {
                    return;
                }
                let t = match (ty, value) {
                    (Some(w), _) => self.written(w, false),
                    (None, Some(v)) => self
                        .types
                        .of(v)
                        .map_or_else(|| OBJECT.to_string(), |t| self.java(t, false)),
                    (None, None) => OBJECT.to_string(),
                };
                let v = value.as_ref().map(|v| self.handed_expr(v));
                let field = format!("{}{LOCAL_FIELD}{}", Self::ident(&n.text), self.new_state());
                self.declare_field(&t, &field);
                self.renamed.push((n.text.clone(), field.clone()));
                if let Some(v) = v {
                    self.code.line(format!("{field} = {v};"));
                }
            }
            Stmt::Local {
                binding: Binding::Tuple(names),
                value: Some(v),
                ..
            } => self.machine_taken_apart(names, v),
            Stmt::Local {
                binding: Binding::Tuple(names),
                ..
            } => {
                if let Some(n) = names.first() {
                    self.not_yet("a tuple taken apart without a value", n.span);
                }
            }
            s if !stmt_awaits(s) => self.stmt(s),
            Stmt::Block(b) => self.machine_block(b),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => self.split_if(cond, then, otherwise.as_deref()),
            Stmt::While { cond, body } => self.split_while(cond, body),
            Stmt::DoWhile { body, cond } => self.split_do(body, cond),
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => self.split_for(init.as_deref(), cond.as_ref(), step.as_deref(), body),
            Stmt::Foreach {
                binding: Binding::One(n),
                iter,
                body,
                ..
            } => self.split_foreach(n, iter, body),
            Stmt::Assign { place, value, .. }
                if self.await_unwritable(place) || self.await_unwritable(value) => {}
            Stmt::Expr(e) | Stmt::Value(e) | Stmt::Return(Some(e), _)
                if self.await_unwritable(e) => {}
            other => self.stmt(other),
        }
    }

    /// `let (a, b) = v;` in the machine: `v` kept in a field, each name a
    /// field of its part.
    fn machine_taken_apart(&mut self, names: &[yog_script_analyze::ast::Name], v: &Expr) {
        if self.await_unwritable(v) {
            return;
        }
        let of = self.types.of(v).cloned();
        let whole_t = of
            .as_ref()
            .map_or_else(|| OBJECT.to_string(), |t| self.java(t, false));
        let whole = self.handed_expr(v);
        let kept = self.temp();
        self.declare_field(&whole_t, &kept);
        self.code.line(format!("{kept} = {whole};"));
        for (i, n) in names.iter().enumerate() {
            let Some(part) = self.part_of(of.as_ref(), &kept, i) else {
                return self.not_yet("this taken apart", n.span);
            };
            let t = self
                .types
                .binding(n)
                .map_or_else(|| OBJECT.to_string(), |t| self.java(t, false));
            let field = format!("{}{LOCAL_FIELD}{}", Self::ident(&n.text), self.new_state());
            self.declare_field(&t, &field);
            self.renamed.push((n.text.clone(), field.clone()));
            self.code.line(format!("{field} = {part};"));
        }
    }

    /// A branch of a split statement: a block's statements, or the one.
    fn machine_branch(&mut self, s: &Stmt) {
        match s {
            Stmt::Block(b) => self.machine_block(b),
            other => {
                let depth = self.renamed.len();
                self.machine_stmt(other);
                self.renamed.truncate(depth);
            }
        }
    }

    /// The condition `cond` as Java's, its `await`s states; jumping to
    /// `past` when it doesn't hold.
    fn unless_jump(&mut self, cond: &Expr, past: usize) -> bool {
        if self.await_unwritable(cond) {
            return false;
        }
        let c = self.expr(cond);
        self.code
            .line(format!("{IF} (!({c})) {}", Self::jump(past)));
        true
    }

    fn split_if(&mut self, cond: &Expr, then: &Stmt, otherwise: Option<&Stmt>) {
        let (other, past) = (self.new_state(), self.new_state());
        if !self.unless_jump(cond, other) {
            return;
        }
        self.machine_branch(then);
        self.code.line(Self::jump(past));
        self.state_starts(other);
        if let Some(o) = otherwise {
            self.machine_branch(o);
        }
        self.state_starts(past);
    }

    fn split_while(&mut self, cond: &Expr, body: &Stmt) {
        let l = Loop {
            again: self.new_state(),
            past: self.new_state(),
        };
        self.state_starts(l.again);
        if !self.unless_jump(cond, l.past) {
            return;
        }
        self.split_body(&l, body);
        self.code.line(Self::jump(l.again));
        self.state_starts(l.past);
    }

    fn split_do(&mut self, body: &Stmt, cond: &Expr) {
        let (top, l) = (
            self.new_state(),
            Loop {
                again: self.new_state(),
                past: self.new_state(),
            },
        );
        self.state_starts(top);
        self.split_body(&l, body);
        self.state_starts(l.again);
        if self.await_unwritable(cond) {
            return;
        }
        let c = self.expr(cond);
        self.code.line(format!("{IF} ({c}) {}", Self::jump(top)));
        self.state_starts(l.past);
    }

    fn split_for(
        &mut self,
        init: Option<&Stmt>,
        cond: Option<&Expr>,
        step: Option<&Stmt>,
        body: &Stmt,
    ) {
        let depth = self.renamed.len();
        if let Some(i) = init {
            self.machine_stmt(i);
        }
        let (top, l) = (
            self.new_state(),
            Loop {
                again: self.new_state(),
                past: self.new_state(),
            },
        );
        self.state_starts(top);
        if let Some(c) = cond
            && !self.unless_jump(c, l.past)
        {
            return;
        }
        self.split_body(&l, body);
        self.state_starts(l.again);
        if let Some(s) = step {
            self.machine_stmt(s);
        }
        self.code.line(Self::jump(top));
        self.state_starts(l.past);
        self.renamed.truncate(depth);
    }

    /// `foreach (let x in items)` split: its items' iterator (an array's
    /// index) a field, the item a field.
    fn split_foreach(&mut self, n: &yog_script_analyze::ast::Name, iter: &Expr, body: &Stmt) {
        if awaits(iter) && self.await_unwritable(iter) {
            return;
        }
        if let ExprKind::Range {
            from,
            to,
            inclusive,
        } = &iter.kind
        {
            return self.split_range(n, (from, to, *inclusive), body);
        }
        let item_t = self.types.item(iter).cloned().unwrap_or(Type::Unknown);
        let item = format!("{}{LOCAL_FIELD}{}", Self::ident(&n.text), self.new_state());
        self.declare_field(&self.java(&item_t, false), &item);
        let Some((has_next, next)) = self.foreach_source(iter, &item_t) else {
            return;
        };
        let depth = self.renamed.len();
        self.renamed.push((n.text.clone(), item.clone()));
        let l = Loop {
            again: self.new_state(),
            past: self.new_state(),
        };
        self.state_starts(l.again);
        self.code
            .line(format!("{IF} (!({has_next})) {}", Self::jump(l.past)));
        self.code.line(format!("{item} = {next};"));
        self.split_body(&l, body);
        self.code.line(Self::jump(l.again));
        self.state_starts(l.past);
        self.renamed.truncate(depth);
    }

    /// `foreach (let i in from..to)` split: `i` and the end fields, `i`
    /// stepped where a `continue` goes.
    fn split_range(
        &mut self,
        n: &yog_script_analyze::ast::Name,
        (from, to, inclusive): (&Expr, &Expr, bool),
        body: &Stmt,
    ) {
        let t = self
            .types
            .of(from)
            .map_or_else(|| INT.to_string(), |t| self.java(t, false));
        let at = format!("{}{LOCAL_FIELD}{}", Self::ident(&n.text), self.new_state());
        let end = self.temp();
        self.declare_field(&t, &at);
        self.declare_field(&t, &end);
        let a = self.expr(from);
        let a = self.kept_before(a, from, awaits(to));
        let b = self.expr(to);
        self.code.line(format!("{at} = {a};"));
        self.code.line(format!("{end} = {b};"));
        let depth = self.renamed.len();
        self.renamed.push((n.text.clone(), at.clone()));
        let (top, l) = (
            self.new_state(),
            Loop {
                again: self.new_state(),
                past: self.new_state(),
            },
        );
        let within = crate::exprs::op_text(if inclusive { BinOp::LtEq } else { BinOp::Lt });
        self.state_starts(top);
        self.code.line(format!(
            "{IF} (!({at} {within} {end})) {}",
            Self::jump(l.past)
        ));
        self.split_body(&l, body);
        self.state_starts(l.again);
        self.code.line(format!("{at}++;"));
        self.code.line(Self::jump(top));
        self.state_starts(l.past);
        self.renamed.truncate(depth);
    }

    /// What a split `foreach` reads its items with: whether there's one
    /// more, the next. An array (a string's chars) by an index, else an
    /// iterator, each a field.
    fn foreach_source(&mut self, iter: &Expr, item_t: &Type) -> Option<(String, String)> {
        let items = self.expr(iter);
        let over = match self.types.of(iter).map(Type::non_null) {
            Some(Type::String) => Some(format!("{items}.{TO_CHAR_ARRAY}()")),
            Some(Type::Array(_, 1)) => Some(items.clone()),
            Some(Type::Array(..)) => {
                self.not_yet(
                    "an array of more than one rank gone through in an `async` method",
                    iter.span,
                );
                return None;
            }
            _ => None,
        };
        let field = self.temp();
        Some(match over {
            Some(array) => {
                let t = match self.types.of(iter).map(Type::non_null) {
                    Some(Type::String) => format!("{CHAR}{RANK}"),
                    Some(t) => self.java(t, false),
                    None => OBJECT.to_string(),
                };
                let at = self.temp();
                self.declare_field(&t, &field);
                self.declare_field(INT, &at);
                self.code.line(format!("{field} = {array};"));
                self.code.line(format!("{at} = 0;"));
                (
                    format!("{at} < {field}.{LENGTH}"),
                    format!("{field}[{at}++]"),
                )
            }
            None => {
                let t = format!("{ITERATOR}<{}>", self.java(item_t, true));
                self.declare_field(&t, &field);
                self.code
                    .line(format!("{field} = {items}.{ITERATOR_OF}();"));
                (format!("{field}.{HAS_NEXT}()"), format!("{field}.{NEXT}()"))
            }
        })
    }

    /// A split loop's body: its `break` and `continue` jumps.
    fn split_body(&mut self, l: &Loop, body: &Stmt) {
        self.breaks.push(Some(Self::jump(l.past)));
        self.continues.push(Some(Self::jump(l.again)));
        self.machine_branch(body);
        self.continues.pop();
        self.breaks.pop();
    }
}
