//! Statements: Java's own, a value computed before its statement (a
//! `switch`'s, a `try`'s) written on the lines before it.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Binding, Block, Expr, ExprKind, Stmt};

use yog_script_ids::clr::{ACCESSOR_SEP, INVOKE};
use yog_script_ids::jvm::{BUS_PRIORITY, REF};

use super::Gen;
use crate::modifier;
use yog_script_ids::java::{
    BREAK, CONTINUE, DO, ELSE, FINAL, FOR, IF, INT, NEW, NULL, RETURN, TRUE, VAR, WHILE,
};

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
            } => self.local(n, (written.as_ref(), value.as_ref()), *mutable),
            Stmt::Local {
                binding: Binding::Tuple(names),
                value,
                mutable,
                ..
            } => match value {
                Some(v) => {
                    let fin = if *mutable {
                        String::new()
                    } else {
                        modifier(FINAL)
                    };
                    self.taken_apart(names, v, &fin);
                }
                None => {
                    if let Some(n) = names.first() {
                        self.not_yet("a tuple taken apart without a value", n.span);
                    }
                }
            },
            Stmt::Assign { place, op, value } => self.assign(place, op.as_ref(), value),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                let c = self.expr(cond);
                self.branch(&format!("{IF} ({c})"), then);
                if let Some(o) = otherwise {
                    self.branch(ELSE, o);
                }
            }
            Stmt::While { cond, body } if crate::switch::hoists(cond) => {
                self.stepped_loop((Some(cond), true), None, body);
            }
            Stmt::While { cond, body } => {
                let c = self.expr(cond);
                self.loop_body(|g| g.branch(&format!("{WHILE} ({c})"), body));
            }
            Stmt::DoWhile { body, cond } if crate::switch::hoists(cond) => {
                self.stepped_loop((Some(cond), false), None, body);
            }
            Stmt::DoWhile { body, cond } => {
                self.code.open(DO);
                self.loop_body(|g| g.stmt(body));
                let c = self.expr(cond);
                self.code.close(&format!(" {WHILE} ({c});"));
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

    /// `let x = v;`, `i32 x = v;`, `i32 x;` (its value set after; one that
    /// doesn't change set once, Java's `final` too).
    fn local(
        &mut self,
        n: &yog_script_analyze::ast::Name,
        (written, value): (Option<&yog_script_analyze::ast::Type>, Option<&Expr>),
        mutable: bool,
    ) {
        let fin = if mutable {
            String::new()
        } else {
            modifier(FINAL)
        };
        let name = Self::ident(&n.text);
        if self.boxed.contains(&n.text) {
            return self.boxed_local(n, (written, value));
        }
        let Some(v) = value else {
            let ty = match written {
                Some(w) => self.written(w, false),
                None => return self.not_yet("a local of no type written and no value", n.span),
            };
            self.code.line(format!("{fin}{ty} {name};"));
            return;
        };
        // The type written, else a lambda's own (Java's `var` can't take
        // one), else `var`.
        let ty = match (written, self.types.of(v)) {
            (Some(w), _) => self.written(w, false),
            (None, Some(t @ Type::Fn(..))) => self.java(t, false),
            _ => VAR.to_string(),
        };
        let v = self.handed_expr(v);
        self.code.line(format!("{fin}{ty} {name} = {v};"));
    }

    /// A local a lambda captures and something changes: a cell, read and
    /// written as `x.value` from here on.
    fn boxed_local(
        &mut self,
        n: &yog_script_analyze::ast::Name,
        (written, value): (Option<&yog_script_analyze::ast::Type>, Option<&Expr>),
    ) {
        let ty = match (written, value.and_then(|v| self.types.of(v))) {
            (Some(w), _) => self.written(w, true),
            (None, Some(t)) => self.java(t, true),
            (None, None) => yog_script_ids::jvm::OBJECT.to_string(),
        };
        let v = value.map(|v| self.handed_expr(v)).unwrap_or_default();
        let name = Self::ident(&n.text);
        self.code
            .line(format!("{FINAL} {REF}<{ty}> {name} = {NEW} {REF}<>({v});"));
        self.cells.insert(n.text.clone());
    }

    /// `f` writing a body of `stmts`: its locals a lambda captures and
    /// something changes cells.
    pub(crate) fn with_boxed<R>(&mut self, stmts: &[Stmt], f: impl FnOnce(&mut Self) -> R) -> R {
        let outer = std::mem::replace(&mut self.boxed, crate::captures::boxed_locals(stmts));
        let cells = self.cells.clone();
        let out = f(self);
        self.cells = cells;
        self.boxed = outer;
        out
    }

    fn simple(&mut self, s: &Stmt) {
        match s {
            Stmt::Block(b) => {
                self.code.open("");
                self.block_stmts(b);
                self.code.close("");
            }
            Stmt::Break(_) => match self.breaks.last().cloned().flatten() {
                Some(jump) => {
                    self.code.line(jump);
                }
                None => {
                    self.code.line(format!("{BREAK};"));
                }
            },
            // In a loop written in steps: past its body, to its step.
            Stmt::Continue(_) => match self.continues.last().cloned().flatten() {
                Some(leave) => {
                    self.code.line(leave);
                }
                None => {
                    self.code.line(format!("{CONTINUE};"));
                }
            },
            // An `async` method's: its task finished.
            Stmt::Return(value, _) if self.machine.is_some() => {
                let v = value.as_ref().map(|v| self.handed_expr(v));
                self.async_return(v);
            }
            Stmt::Return(value, _) | Stmt::Skip(value, _) => match value {
                Some(v) => {
                    let v = self.handed_expr(v);
                    self.code.line(format!("{RETURN} {v};"));
                }
                None => {
                    self.code.line(format!("{RETURN};"));
                }
            },
            Stmt::Expr(e) | Stmt::Value(e) => self.expr_stmt(e),
            _ => {}
        }
    }

    /// An expression standing as a statement: a call as it is; any other
    /// value kept in a temporary (Java takes only calls and assignments).
    pub(crate) fn expr_stmt(&mut self, e: &Expr) {
        if let ExprKind::Unary { op, .. } = &e.kind
            && op.steps()
        {
            let v = match self.types.operator(e).cloned() {
                Some(used) => self.declared_step(e, &used, true),
                None => self.expr(e),
            };
            self.code.line(format!("{v};"));
            return;
        }
        let v = self.expr(e);
        if matches!(
            e.kind,
            ExprKind::Call { .. } | ExprKind::New { .. } | ExprKind::Macro { .. }
        ) && !v.is_empty()
        {
            self.code.line(format!("{v};"));
        } else if !v.is_empty() {
            let t = self.temp();
            self.code.line(format!("{VAR} {t} = {v};"));
        }
    }

    fn assign(&mut self, place: &Expr, op: Option<&BinOp>, value: &Expr) {
        if let Some(op @ (BinOp::Add | BinOp::Sub)) = op
            && let Some(ev) = self.event_of(place)
        {
            return self.subscribe(&ev, *op, value);
        }
        if let (Some(BinOp::Add), ExprKind::Member { expr, name, .. }) = (op, &place.kind)
            && let Some(Type::Yog(group)) = self.types.of(expr)
            && let Some(event) = yog_script_analyze::yog::event(group, &name.text)
        {
            return self.listen(&event, value);
        }
        let v = self.handed_expr(value);
        if let ExprKind::Index { expr, args } = &place.kind {
            let list = self.expr(expr);
            let at: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
            if !matches!(
                self.types.of(expr).map(Type::non_null),
                Some(Type::Array(..))
            ) && self.has_indexer(expr)
            {
                let at = at.join(", ");
                let v = match op {
                    Some(op) => self.op_value(place, *op, &Self::index_read(&list, &at), &v),
                    None => v,
                };
                self.code.line(Self::index_write(&list, &at, &v));
                return;
            }
            // An array's item.
            let slot = Self::array_slot(&list, &at);
            let set = self.set(place, op.copied(), &slot, &v);
            self.code.line(format!("{set};"));
            return;
        }
        if let Some(setter) = self.property_setter(place, op.copied(), &v) {
            self.code.line(format!("{setter};"));
            return;
        }
        let p = self.expr(place);
        let set = self.set(place, op.copied(), &p, &v);
        self.code.line(format!("{set};"));
    }

    /// `x.Name = v` of a property Java reads by its getter (a runtime
    /// type's): its setter, the CLR's `set_Name` (`x.set_Name(x.Name() op v)`).
    fn property_setter(&mut self, place: &Expr, op: Option<BinOp>, v: &str) -> Option<String> {
        let ExprKind::Member { expr, name, .. } = &place.kind else {
            return None;
        };
        let Some(Type::Own(of, args)) = self.types.of(expr).map(Type::non_null) else {
            return None;
        };
        let of = (of.clone(), args.len());
        if self.index.kind(&of) == Some(crate::index::Kind::Record)
            || !self.index.called(&of, &name.text)
        {
            return None;
        }
        let target = self.expr(expr);
        let n = Self::ident(&name.text);
        let value = match op {
            Some(op) => self.op_value(place, op, &format!("{target}.{n}()"), v),
            None => v.to_string(),
        };
        Some(format!("{target}.{}({value})", setter(&n)))
    }

    /// `p op= v` (`p` the place written): Java's, or — when the place's type
    /// declares the operator — `p = T.op_X(p, v)`.
    fn set(&self, place: &Expr, op: Option<BinOp>, p: &str, v: &str) -> String {
        match op {
            Some(op) if self.types.assigned(place).is_some() => {
                format!("{p} = {}", self.op_value(place, op, p, v))
            }
            Some(op) => format!("{p} {}= {v}", crate::exprs::op_text(op)),
            None => format!("{p} = {v}"),
        }
    }

    /// What `place op= v` puts in `place` (read as `read`): its type's
    /// operator's value, or Java's `read op v`.
    fn op_value(&self, place: &Expr, op: BinOp, read: &str, v: &str) -> String {
        match self.types.assigned(place) {
            Some(used) => self.applied(used, &[read.to_string(), v.to_string()]),
            None => format!("{read} {} {v}", crate::exprs::op_text(op)),
        }
    }

    fn for_loop(
        &mut self,
        init: Option<&Stmt>,
        cond: Option<&Expr>,
        step: Option<&Stmt>,
        body: &Stmt,
    ) {
        // In a block of its own, its first statement before it: Java's
        // `for` with the condition and the step in its head — or, either
        // computed in steps, a loop of them.
        self.code.open("");
        if let Some(i) = init {
            self.stmt(i);
        }
        let simple_step = match step {
            Some(Stmt::Assign { value, .. }) => !crate::switch::hoists(value),
            Some(Stmt::Expr(e)) => !crate::switch::hoists(e),
            Some(_) => false,
            None => true,
        };
        if cond.is_some_and(crate::switch::hoists) || !simple_step {
            self.stepped_loop((cond, true), step, body);
            self.code.close("");
            return;
        }
        let c = cond.map(|c| self.expr(c)).unwrap_or_default();
        let step = match step {
            Some(Stmt::Assign { place, op, value }) => {
                let (p, v) = (self.expr(place), self.expr(value));
                self.set(place, *op, &p, &v)
            }
            Some(Stmt::Expr(e)) => self.expr(e),
            _ => String::new(),
        };
        self.loop_body(|g| g.branch(&format!("{FOR} (; {c}; {step})"), body));
        self.code.close("");
    }

    /// A loop whose condition or step is computed in steps: `while (true)`,
    /// the condition checked first or (a `do`'s) last, the body in a
    /// labeled block a `continue` leaves, then the step.
    fn stepped_loop(
        &mut self,
        (cond, first): (Option<&Expr>, bool),
        step: Option<&Stmt>,
        body: &Stmt,
    ) {
        self.code.open(format!("{WHILE} ({TRUE})"));
        let check = |g: &mut Self| {
            if let Some(c) = cond {
                let c = g.expr(c);
                g.code.line(format!("{IF} (!({c})) {BREAK};"));
            }
        };
        if first {
            check(self);
        }
        let label = self.temp();
        self.code.open(format!("{label}:"));
        self.continues.push(Some(format!("{BREAK} {label};")));
        self.breaks.push(None);
        match body {
            Stmt::Block(b) => self.block_stmts(b),
            other => self.stmt(other),
        }
        self.continues.pop();
        self.breaks.pop();
        self.code.close("");
        if let Some(s) = step {
            self.stmt(s);
        }
        if !first {
            check(self);
        }
        self.code.close("");
    }

    /// A loop's body written by `f`, its `continue` Java's own.
    fn loop_body(&mut self, f: impl FnOnce(&mut Self)) {
        self.continues.push(None);
        self.breaks.push(None);
        f(self);
        self.breaks.pop();
        self.continues.pop();
    }

    fn foreach(&mut self, binding: &Binding, iter: &Expr, body: &Stmt) {
        let Binding::One(n) = binding else {
            if let Binding::Tuple(names) = binding {
                self.foreach_taken_apart(names, iter, body);
            }
            return;
        };
        let name = Self::ident(&n.text);
        if let ExprKind::Range {
            from,
            to,
            inclusive,
        } = &iter.kind
        {
            let (a, b) = (self.expr(from), self.expr(to));
            let cmp = crate::exprs::op_text(if *inclusive { BinOp::LtEq } else { BinOp::Lt });
            let head = format!("{FOR} ({INT} {name} = {a}; {name} {cmp} {b}; {name}++)");
            return self.loop_body(|g| g.branch(&head, body));
        }
        let (items, rows) = self.gone_through(iter);
        self.loop_body(|g| g.branch(&format!("{FOR} ({VAR} {name} : {items})"), body));
        for _ in 0..rows {
            self.code.close("");
        }
    }

    /// What `foreach` goes through in Java (a string's `char`s, an array's
    /// items row by row: the loops opened for the rows, how many).
    fn gone_through(&mut self, iter: &Expr) -> (String, usize) {
        let items = self.expr(iter);
        match self.types.of(iter).map(Type::non_null) {
            Some(Type::String) => (
                format!("{items}.{}()", yog_script_ids::jvm::TO_CHAR_ARRAY),
                0,
            ),
            Some(Type::Array(_, rank)) => self.array_loops(&items, *rank),
            _ => (items, 0),
        }
    }

    /// `foreach (let (a, b) in items)`: each item taken apart — a tuple
    /// by its items, a record by its constructor's parameters.
    fn foreach_taken_apart(
        &mut self,
        names: &[yog_script_analyze::ast::Name],
        iter: &Expr,
        body: &Stmt,
    ) {
        let item = self.types.item(iter).cloned();
        let (items, rows) = self.gone_through(iter);
        let each = self.temp();
        self.code.open(format!("{FOR} ({VAR} {each} : {items})"));
        for (i, n) in names.iter().enumerate() {
            let Some(part) = self.part_of(item.as_ref(), &each, i) else {
                return self.not_yet("this taken apart", n.span);
            };
            self.code.line(format!(
                "{} {VAR} {} = {part};",
                FINAL,
                Self::ident(&n.text)
            ));
        }
        self.loop_body(|g| match body {
            Stmt::Block(b) => g.block_stmts(b),
            other => g.stmt(other),
        });
        self.code.close("");
        for _ in 0..rows {
            self.code.close("");
        }
    }
}

impl Gen<'_> {
    /// `Group.Event += handler`: listened to on the bus, the handler
    /// changing nothing (its listener gives `null`). A method or a
    /// delegate's value made once, then called with each event's.
    fn listen(&mut self, event: &str, handler: &Expr) {
        let ExprKind::Lambda {
            params,
            body,
            asynchronous: false,
        } = &handler.kind
        else {
            let Some(t @ Type::Fn(..)) = self.types.of(handler).cloned() else {
                return self.not_yet("this handler", handler.span);
            };
            let (made, each) = (self.temp(), self.temp());
            let delegate = self.expr(handler);
            self.code.line(format!(
                "{FINAL} {} {made} = {delegate};",
                self.java(&t, false)
            ));
            self.code
                .open(format!("{event}.on({BUS_PRIORITY}, ({each}) ->"));
            self.code.line(format!("{made}.{INVOKE}({each});"));
            self.code.line(format!("{RETURN} {NULL};"));
            self.code.close(");");
            return;
        };
        let name = params.first().map_or_else(
            || yog_script_analyze::ast::IT.to_string(),
            |p| Self::ident(&p.name.text),
        );
        self.code
            .open(format!("{event}.on({BUS_PRIORITY}, ({name}) ->"));
        match body {
            yog_script_analyze::ast::LambdaBody::Expr(e) => self.expr_stmt(e),
            yog_script_analyze::ast::LambdaBody::Block(stmts) => {
                stmts.iter().for_each(|s| self.stmt(s))
            }
        }
        self.code.line(format!("{RETURN} {NULL};"));
        self.code.close(");");
    }
}

/// Property `n`'s setter in Java: the CLR's `set_Name`.
pub(crate) fn setter(n: &str) -> String {
    format!(
        "{}{ACCESSOR_SEP}{n}",
        yog_script_analyze::words::set::WORD.text
    )
}
