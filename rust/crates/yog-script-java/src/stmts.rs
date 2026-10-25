//! Statements: Java's own, a value computed before its statement (a
//! `switch`'s, a `try`'s) written on the lines before it.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Binding, Block, Expr, ExprKind, Stmt};

use super::Gen;

impl Gen<'_> {
    pub(crate) fn block_stmts(&mut self, b: &Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }

    fn branch(&mut self, head: &str, s: &Stmt) {
        self.code.open(head);
        match s {
            Stmt::Block(b) => self.block_stmts(b),
            other => self.stmt(other),
        }
        self.code.close("");
    }

    pub(crate) fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local {
                ty: written,
                binding: Binding::One(n),
                value,
                mutable,
            } => {
                let fin = if *mutable { "" } else { "final " };
                let name = Self::ident(&n.text);
                match value {
                    Some(v) => {
                        // The type written, else a lambda's own (Java's `var`
                        // can't take one), else `var`.
                        let ty = match (written, self.types.of(v)) {
                            (Some(w), _) => self.written(w, false),
                            (None, Some(t @ Type::Fn(..))) => self.java(t, false),
                            _ => "var".to_string(),
                        };
                        let v = self.handed_expr(v);
                        self.code.line(format!("{fin}{ty} {name} = {v};"));
                    }
                    None => self.not_yet("a local without a value", n.span),
                }
            }
            Stmt::Local {
                binding: Binding::Tuple(names),
                ..
            } => {
                if let Some(n) = names.first() {
                    self.not_yet("a tuple taken apart", n.span);
                }
            }
            Stmt::Assign { place, op, value } => self.assign(place, op.as_ref(), value),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                let c = self.expr(cond);
                self.branch(&format!("if ({c})"), then);
                if let Some(o) = otherwise {
                    self.branch("else", o);
                }
            }
            Stmt::While { cond, body } => {
                let c = self.expr(cond);
                self.branch(&format!("while ({c})"), body);
            }
            Stmt::DoWhile { body, cond } => {
                self.code.open("do");
                self.stmt(body);
                let c = self.expr(cond);
                self.code.close(&format!(" while ({c});"));
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => self.for_loop(init.as_deref(), cond.as_ref(), step.as_deref(), body),
            Stmt::Foreach {
                binding,
                iter,
                body,
                ..
            } => self.foreach(binding, iter, body),
            _ => self.simple(s),
        }
    }

    fn simple(&mut self, s: &Stmt) {
        match s {
            Stmt::Block(b) => {
                self.code.open("");
                self.block_stmts(b);
                self.code.close("");
            }
            Stmt::Break(_) => {
                self.code.line("break;");
            }
            Stmt::Continue(_) => {
                self.code.line("continue;");
            }
            Stmt::Return(value, _) | Stmt::Skip(value, _) => match value {
                Some(v) => {
                    let v = self.handed_expr(v);
                    self.code.line(format!("return {v};"));
                }
                None => {
                    self.code.line("return;");
                }
            },
            Stmt::Expr(e) | Stmt::Value(e) => self.expr_stmt(e),
            _ => {}
        }
    }

    /// An expression standing as a statement: a call as it is; any other
    /// value kept in a temporary (Java takes only calls and assignments).
    fn expr_stmt(&mut self, e: &Expr) {
        let v = self.expr(e);
        if matches!(
            e.kind,
            ExprKind::Call { .. } | ExprKind::New { .. } | ExprKind::Macro { .. }
        ) && !v.is_empty()
        {
            self.code.line(format!("{v};"));
        } else if !v.is_empty() {
            let t = self.temp();
            self.code.line(format!("var {t} = {v};"));
        }
    }

    fn assign(&mut self, place: &Expr, op: Option<&yog_script_analyze::ast::BinOp>, value: &Expr) {
        if let (Some(yog_script_analyze::ast::BinOp::Add), ExprKind::Member { expr, name, .. }) =
            (op, &place.kind)
            && let Some(Type::Yog(group)) = self.types.of(expr)
            && let Some(event) = yog_script_analyze::yog::event(group, &name.text)
        {
            return self.listen(&event, value);
        }
        let v = self.handed_expr(value);
        if let ExprKind::Index { expr, index } = &place.kind {
            let (list, at) = (self.expr(expr), self.expr(index));
            if self.has_indexer(expr) {
                let v = match op {
                    Some(op) => format!(
                        "{} {} {v}",
                        Self::index_read(&list, &at),
                        crate::exprs::op_text(*op)
                    ),
                    None => v,
                };
                self.code.line(Self::index_write(&list, &at, &v));
                return;
            }
            let v = match op {
                Some(op) => format!("{list}.get({at}) {} {v}", crate::exprs::op_text(*op)),
                None => v,
            };
            self.code.line(format!("{list}.set({at}, {v});"));
            return;
        }
        let p = self.expr(place);
        match op {
            Some(op) => self
                .code
                .line(format!("{p} {}= {v};", crate::exprs::op_text(*op))),
            None => self.code.line(format!("{p} = {v};")),
        };
    }

    fn for_loop(
        &mut self,
        init: Option<&Stmt>,
        cond: Option<&Expr>,
        step: Option<&Stmt>,
        body: &Stmt,
    ) {
        // In a block of its own, its first statement before it: Java's
        // `for` with the condition and the step in its head.
        self.code.open("");
        if let Some(i) = init {
            self.stmt(i);
        }
        let c = match cond {
            Some(c) if crate::switch::hoists(c) => {
                return self.not_yet("a `for`'s condition computed in steps", c.span);
            }
            Some(c) => self.expr(c),
            None => String::new(),
        };
        let step = match step {
            Some(Stmt::Assign { place, op, value }) if !crate::switch::hoists(value) => {
                let (p, v) = (self.expr(place), self.expr(value));
                let op = op.map(crate::exprs::op_text).unwrap_or_default();
                format!("{p} {op}= {v}")
            }
            Some(Stmt::Expr(e)) if !crate::switch::hoists(e) => self.expr(e),
            Some(other) => return self.not_yet("this `for` step", step_span(other)),
            None => String::new(),
        };
        self.branch(&format!("for (; {c}; {step})"), body);
        self.code.close("");
    }

    fn foreach(&mut self, binding: &Binding, iter: &Expr, body: &Stmt) {
        let Binding::One(n) = binding else {
            return self.not_yet("a tuple taken apart", iter.span);
        };
        let name = Self::ident(&n.text);
        if let ExprKind::Range {
            from,
            to,
            inclusive,
        } = &iter.kind
        {
            let (a, b) = (self.expr(from), self.expr(to));
            let cmp = if *inclusive { "<=" } else { "<" };
            return self.branch(
                &format!("for (int {name} = {a}; {name} {cmp} {b}; {name}++)"),
                body,
            );
        }
        let items = self.expr(iter);
        let items = match self.types.of(iter) {
            Some(Type::String) => format!("{items}.toCharArray()"),
            _ => items,
        };
        self.branch(&format!("for (var {name} : {items})"), body);
    }
}

impl Gen<'_> {
    /// `Group.Event += handler`: listened to on the bus, the handler
    /// changing nothing (its listener gives `null`).
    fn listen(&mut self, event: &str, handler: &Expr) {
        let ExprKind::Lambda { params, body } = &handler.kind else {
            return self.not_yet("a handler that isn't a lambda", handler.span);
        };
        let name = params.first().map_or_else(
            || yog_script_analyze::ast::IT.to_string(),
            |p| Self::ident(&p.name.text),
        );
        self.code.open(format!("{event}.on(0, ({name}) ->"));
        match body {
            yog_script_analyze::ast::LambdaBody::Expr(e) => self.expr_stmt(e),
            yog_script_analyze::ast::LambdaBody::Block(stmts) => {
                stmts.iter().for_each(|s| self.stmt(s))
            }
        }
        self.code.line("return null;");
        self.code.close(");");
    }
}

fn step_span(s: &Stmt) -> yog_script_analyze::token::Span {
    match s {
        Stmt::Expr(e) | Stmt::Value(e) => e.span,
        _ => yog_script_analyze::token::Span::default(),
    }
}
