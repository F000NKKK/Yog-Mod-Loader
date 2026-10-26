//! Statements: Java's own, a value computed before its statement (a
//! `switch`'s, a `try`'s) written on the lines before it.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{BinOp, Binding, Block, Expr, ExprKind, Stmt};

use yog_script_ids::clr::{ACCESSOR_SEP, TUPLE_ITEM};

use super::Gen;
use crate::modifier;
use yog_script_ids::java::{
    BREAK, CONTINUE, DO, ELSE, FINAL, FOR, IF, INT, NULL, RETURN, VAR, WHILE,
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
            Stmt::While { cond, body } => {
                let c = self.expr(cond);
                self.branch(&format!("{WHILE} ({c})"), body);
            }
            Stmt::DoWhile { body, cond } => {
                self.code.open(DO);
                self.stmt(body);
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

    fn simple(&mut self, s: &Stmt) {
        match s {
            Stmt::Block(b) => {
                self.code.open("");
                self.block_stmts(b);
                self.code.close("");
            }
            Stmt::Break(_) => {
                self.code.line(format!("{BREAK};"));
            }
            Stmt::Continue(_) => {
                self.code.line(format!("{CONTINUE};"));
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
        let setter = format!(
            "{}{ACCESSOR_SEP}{n}",
            yog_script_analyze::words::set::WORD.text
        );
        Some(format!("{target}.{setter}({value})"))
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
                self.set(place, *op, &p, &v)
            }
            Some(Stmt::Expr(e)) if !crate::switch::hoists(e) => self.expr(e),
            Some(other) => return self.not_yet("this `for` step", step_span(other)),
            None => String::new(),
        };
        self.branch(&format!("{FOR} (; {c}; {step})"), body);
        self.code.close("");
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
            let cmp = if *inclusive { "<=" } else { "<" };
            return self.branch(
                &format!("{FOR} ({INT} {name} = {a}; {name} {cmp} {b}; {name}++)"),
                body,
            );
        }
        let (items, rows) = self.gone_through(iter);
        self.branch(&format!("{FOR} ({VAR} {name} : {items})"), body);
        for _ in 0..rows {
            self.code.close("");
        }
    }

    /// What `foreach` goes through in Java (a string's `char`s, an array's
    /// items row by row: the loops opened for the rows, how many).
    fn gone_through(&mut self, iter: &Expr) -> (String, usize) {
        let items = self.expr(iter);
        match self.types.of(iter).map(Type::non_null) {
            Some(Type::String) => (format!("{items}.toCharArray()"), 0),
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
            let part = match item.as_ref().map(Type::non_null) {
                Some(Type::Own(record, args)) => match self
                    .index
                    .record_params(&(record.clone(), args.len()))
                    .get(i)
                {
                    Some(p) => format!("{each}.{}()", Self::ident(p)),
                    None => return self.not_yet("this taken apart", n.span),
                },
                _ => format!("{each}.{TUPLE_ITEM}{}", i + 1),
            };
            self.code.line(format!(
                "{} {VAR} {} = {part};",
                FINAL,
                Self::ident(&n.text)
            ));
        }
        match body {
            Stmt::Block(b) => self.block_stmts(b),
            other => self.stmt(other),
        }
        self.code.close("");
        for _ in 0..rows {
            self.code.close("");
        }
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
        self.code.line(format!("{RETURN} {NULL};"));
        self.code.close(");");
    }
}

fn step_span(s: &Stmt) -> yog_script_analyze::token::Span {
    match s {
        Stmt::Expr(e) | Stmt::Value(e) => e.span,
        _ => yog_script_analyze::token::Span::default(),
    }
}
