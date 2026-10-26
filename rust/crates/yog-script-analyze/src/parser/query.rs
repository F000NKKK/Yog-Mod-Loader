//! A query (`from x in xs where … select …`), read into the calls C#
//! makes of it (§12.20.3 of its spec): each clause a LINQ call on the
//! sequence so far, its expressions lambdas over the range variables. More
//! than one is carried as a tuple, each lambda taking it apart first
//! (`row => { let (a, b) = row; return …; }`) — C#'s transparent
//! identifiers.

use yog_ids::script_problem;
use yog_script_ids::members::{
    CAST, GROUP_BY, GROUP_JOIN, JOIN, ORDER_BY, ORDER_BY_DESCENDING, SELECT, SELECT_MANY, THEN_BY,
    THEN_BY_DESCENDING, WHERE,
};
use yog_script_ids::system::TUPLE_ITEMS;

use super::{PResult, Parser};
use crate::ast::{Arg, Binding, Expr, ExprKind, LambdaBody, LambdaParam, Name, Stmt, Type};
use crate::macros::HYGIENE_MARK;
use crate::source::token::{Punct, Span};
use crate::words::{
    ascending, by, descending, equals, from, group, r#in, into, join, r#let, on, orderby, select,
    r#where,
};

/// What a lambda over several range variables calls their tuple: a name
/// no source can write (a macro's hygiene mark in it).
const ROW: &str = "row";

/// The sequence so far and the range variables each of its items holds.
struct Query {
    items: Expr,
    vars: Vec<Name>,
}

impl Parser<'_, '_, '_> {
    /// `from T? x in xs` and the clauses after it.
    pub(crate) fn query(&mut self) -> PResult<Expr> {
        self.need_word(&from::WORD)?;
        let (items, var) = self.range()?;
        self.query_body(Query {
            items,
            vars: vec![var],
        })
    }

    /// `T? x in xs` (after `from` or `join`): the sequence (cast to `T`
    /// when it's written) and the name.
    fn range(&mut self) -> PResult<(Expr, Name)> {
        let typed = !self.nth_word(1, &r#in::WORD);
        let ty = if typed { Some(self.ty()?) } else { None };
        let name = self.name("a range variable")?;
        self.need_word(&r#in::WORD)?;
        let items = self.expr()?;
        Ok(match ty {
            Some(ty) => (cast(items, ty), name),
            None => (items, name),
        })
    }

    /// The clauses after a `from`, to its `select` or `group`, and an
    /// `into` going on from that.
    fn query_body(&mut self, mut q: Query) -> PResult<Expr> {
        loop {
            q = if self.eat_word(&from::WORD) {
                self.more_from(q)?
            } else if self.eat_word(&r#let::WORD) {
                self.let_clause(q)?
            } else if self.eat_word(&r#where::WORD) {
                let cond = self.expr()?;
                let items = call(q.items, WHERE, vec![row_lambda(&q.vars, cond, None)]);
                Query { items, ..q }
            } else if self.eat_word(&join::WORD) {
                self.join_clause(q)?
            } else if self.eat_word(&orderby::WORD) {
                self.orderings(q)?
            } else {
                break;
            };
        }
        let result = if self.eat_word(&select::WORD) {
            let picked = self.expr()?;
            call(q.items, SELECT, vec![row_lambda(&q.vars, picked, None)])
        } else {
            self.need_word(&group::WORD)?;
            let element = self.expr()?;
            self.need_word(&by::WORD)?;
            let key = self.expr()?;
            let (key, element) = (
                row_lambda(&q.vars, key, None),
                row_lambda(&q.vars, element, None),
            );
            call(q.items, GROUP_BY, vec![key, element])
        };
        if self.eat_word(&into::WORD) {
            let name = self.name("the continuation's name")?;
            return self.query_body(Query {
                items: result,
                vars: vec![name],
            });
        }
        Ok(result)
    }

    /// `from y in ys` after the first: every pairing (`SelectMany`).
    fn more_from(&mut self, q: Query) -> PResult<Query> {
        let (inner, var) = self.range()?;
        let row = row_tuple(&q.vars, var_expr(&var), &var);
        let many = row_lambda(&q.vars, inner, None);
        let paired = row_lambda(&q.vars, row, Some(var.clone()));
        self.grown(q, SELECT_MANY, vec![many, paired], var)
    }

    /// `let y = e`: each item with `y` beside it.
    fn let_clause(&mut self, q: Query) -> PResult<Query> {
        let var = self.name("a range variable")?;
        self.need(Punct::Eq)?;
        let value = self.expr()?;
        let row = row_tuple(&q.vars, value, &var);
        let lambda = row_lambda(&q.vars, row, None);
        self.grown(q, SELECT, vec![lambda], var)
    }

    /// `join y in ys on k1 equals k2 (into g)?`.
    fn join_clause(&mut self, q: Query) -> PResult<Query> {
        let (inner, var) = self.range()?;
        self.need_word(&on::WORD)?;
        let outer_key = self.expr()?;
        self.need_word(&equals::WORD)?;
        let inner_key = self.expr()?;
        let (method, joined) = if self.eat_word(&into::WORD) {
            (GROUP_JOIN, self.name("the group's name")?)
        } else {
            (JOIN, var.clone())
        };
        let args = vec![
            inner,
            row_lambda(&q.vars, outer_key, None),
            lambda(vec![var], inner_key),
            row_lambda(
                &q.vars,
                row_tuple(&q.vars, var_expr(&joined), &joined),
                Some(joined.clone()),
            ),
        ];
        self.grown(q, method, args, joined)
    }

    /// `orderby k1 descending, k2, …`.
    fn orderings(&mut self, mut q: Query) -> PResult<Query> {
        let mut first = true;
        loop {
            let key = self.expr()?;
            let down = self.eat_word(&descending::WORD);
            if !down {
                self.eat_word(&ascending::WORD);
            }
            let method = match (first, down) {
                (true, false) => ORDER_BY,
                (true, true) => ORDER_BY_DESCENDING,
                (false, false) => THEN_BY,
                (false, true) => THEN_BY_DESCENDING,
            };
            q.items = call(q.items, method, vec![row_lambda(&q.vars, key, None)]);
            first = false;
            if !self.eat(Punct::Comma) {
                return Ok(q);
            }
        }
    }

    /// `q`'s sequence through `method(args)`, its items holding `var` too.
    fn grown(&mut self, q: Query, method: &str, args: Vec<Expr>, var: Name) -> PResult<Query> {
        if q.vars.len() >= TUPLE_ITEMS {
            return Err(self.error(
                script_problem::EXPECTED,
                var.span,
                format!("a query keeps {TUPLE_ITEMS} range variables at most"),
            ));
        }
        let mut vars = q.vars;
        vars.push(var);
        Ok(Query {
            items: call(q.items, method, args),
            vars,
        })
    }
}

/// `on.method(args)`.
fn call(on: Expr, method: &str, args: Vec<Expr>) -> Expr {
    let span = args.last().map_or(on.span, |a| on.span.to(a.span));
    let name = Name {
        text: method.to_string(),
        span,
    };
    let callee = Expr {
        kind: ExprKind::Member {
            expr: Box::new(on),
            name,
            safe: false,
        },
        span,
    };
    let args = args
        .into_iter()
        .map(|value| Arg {
            name: None,
            pass: None,
            value,
        })
        .collect();
    Expr {
        kind: ExprKind::Call {
            callee: Box::new(callee),
            args,
        },
        span,
    }
}

/// `xs.Cast<T>()`: `from T x in xs`'s.
fn cast(items: Expr, ty: Type) -> Expr {
    let span = items.span;
    let name = Name {
        text: CAST.to_string(),
        span,
    };
    let member = Expr {
        kind: ExprKind::Member {
            expr: Box::new(items),
            name,
            safe: false,
        },
        span,
    };
    let callee = Expr {
        kind: ExprKind::TypeArgs {
            expr: Box::new(member),
            args: vec![ty],
        },
        span,
    };
    Expr {
        kind: ExprKind::Call {
            callee: Box::new(callee),
            args: Vec::new(),
        },
        span,
    }
}

/// `(params) => body`.
fn lambda(params: Vec<Name>, body: Expr) -> Expr {
    let span = body.span;
    let params = params
        .into_iter()
        .map(|name| LambdaParam { ty: None, name })
        .collect();
    Expr {
        kind: ExprKind::Lambda {
            params,
            body: LambdaBody::Expr(Box::new(body)),
        },
        span,
    }
}

/// A lambda over an item holding `vars` (and `extra` after it): `x => body`
/// for one, `(row, extra) => { let (a, b) = row; return body; }` for more.
fn row_lambda(vars: &[Name], body: Expr, extra: Option<Name>) -> Expr {
    let span = body.span;
    let [one] = vars else {
        let row = Name {
            text: format!("{ROW}{HYGIENE_MARK}"),
            span,
        };
        // The row is the lambda's own: taken apart, it's handed on.
        let moved = Expr {
            kind: ExprKind::Move(Box::new(var_expr(&row))),
            span,
        };
        let apart = Stmt::Local {
            ty: None,
            binding: Binding::Tuple(vars.to_vec()),
            value: Some(moved),
            mutable: false,
        };
        let params = std::iter::once(row)
            .chain(extra)
            .map(|name| LambdaParam { ty: None, name })
            .collect();
        return Expr {
            kind: ExprKind::Lambda {
                params,
                body: LambdaBody::Block(vec![apart, Stmt::Return(Some(body), span)]),
            },
            span,
        };
    };
    lambda(std::iter::once(one.clone()).chain(extra).collect(), body)
}

/// `(a, b, value)`: an item's `vars` and `value` named `var` beside them.
fn row_tuple(vars: &[Name], value: Expr, var: &Name) -> Expr {
    let span: Span = value.span;
    let items = vars
        .iter()
        .map(var_expr)
        .chain(std::iter::once(value))
        .collect();
    let names = vars
        .iter()
        .map(|v| Some(v.clone()))
        .chain(std::iter::once(Some(var.clone())))
        .collect();
    Expr {
        kind: ExprKind::Tuple { items, names },
        span,
    }
}

/// `x`, a range variable read.
fn var_expr(var: &Name) -> Expr {
    Expr {
        kind: ExprKind::Ident(var.text.clone()),
        span: var.span,
    }
}
