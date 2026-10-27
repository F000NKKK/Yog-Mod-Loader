//! `await using` (C#'s, for `IAsyncDisposable`): `await using let x = e;`
//! to its block's end, `await using (let x = e) body` to the body's. Its
//! value is disposed (`await x.DisposeAsync()`) where what follows ends:
//! at its end, and before each `return`, `yield break`, and `break` or
//! `continue` leaving it.

use yog_script_ids::members::{DISPOSE_ASYNC, RETURNED};

use super::{PResult, Parser};
use crate::ast::{Arg, BinOp, Binding, Block, Expr, ExprKind, Name, Stmt, UnOp};
use crate::source::token::{Punct, Span};
use crate::words::{r#await, r#let, using};

impl Parser<'_, '_, '_> {
    /// Whether `await using` starts here.
    pub(super) fn await_using_ahead(&self) -> bool {
        self.at_word(&r#await::WORD) && self.nth_word(1, &using::WORD)
    }

    /// `await using (let x = e) body`: a block of the local and the body,
    /// `x` disposed where it ends.
    pub(super) fn await_using_stmt(&mut self) -> PResult<Stmt> {
        let start = self.span();
        self.bump();
        self.bump();
        self.need(Punct::LParen)?;
        let local = r#let::local(self)?;
        self.need(Punct::RParen)?;
        let body = self.stmt()?;
        let span = start.to(self.prev());
        let name = local_name(&local);
        let mut stmts = vec![local];
        stmts.extend(disposed_after(&name, vec![body], span));
        Ok(Stmt::Block(Block { stmts, span }))
    }

    /// `await using let x = e;` and the rest of its block (with its `}`),
    /// `x` disposed where it ends.
    pub(super) fn await_using_rest(&mut self, value: bool) -> PResult<Vec<Stmt>> {
        let start = self.span();
        self.bump();
        self.bump();
        let local = r#let::local(self)?;
        self.end()?;
        let rest = self.stmts(value)?;
        let name = local_name(&local);
        let mut stmts = vec![local];
        stmts.extend(disposed_after(&name, rest, start.to(self.prev())));
        Ok(stmts)
    }
}

/// The name a local declares (the first, of a tuple).
fn local_name(local: &Stmt) -> Name {
    match local {
        Stmt::Local {
            binding: Binding::One(n),
            ..
        } => n.clone(),
        Stmt::Local {
            binding: Binding::Tuple(ns),
            ..
        } if !ns.is_empty() => ns[0].clone(),
        _ => Name {
            text: String::new(),
            span: Span::default(),
        },
    }
}

/// Where a `break` or `continue` goes: loops and switches around it.
#[derive(Clone, Copy, Default)]
struct Around {
    loops: bool,
    switches: bool,
}

/// `stmts`, `x` disposed where they end.
fn disposed_after(x: &Name, stmts: Vec<Stmt>, span: Span) -> Vec<Stmt> {
    let leaves = stmts.last().is_some_and(crate::typeck::leaves);
    let mut out: Vec<Stmt> = stmts
        .into_iter()
        .map(|s| rewrite(x, s, Around::default()))
        .collect();
    if !leaves {
        out.push(dispose(x, span));
    }
    out
}

/// `s` with `x` disposed before each way out of what `x` lives in.
fn rewrite(x: &Name, s: Stmt, around: Around) -> Stmt {
    match leaving(x, s, around) {
        Ok(done) => done,
        Err(s) => nested(x, *s, around),
    }
}

/// A way out (`return`, `yield break`, a `break`/`continue` leaving):
/// `x` disposed first; else `s` back.
fn leaving(x: &Name, s: Stmt, around: Around) -> Result<Stmt, Box<Stmt>> {
    let block = |stmts: Vec<Stmt>, span| Stmt::Block(Block { stmts, span });
    Ok(match s {
        Stmt::Return(Some(v), at) => {
            let kept = Name {
                text: format!("{RETURNED}{}", x.text),
                span: at,
            };
            let held = Expr {
                kind: ExprKind::Ident(kept.text.clone()),
                span: at,
            };
            let local = Stmt::Local {
                ty: None,
                binding: Binding::One(kept),
                value: Some(v),
                mutable: false,
            };
            block(
                vec![local, dispose(x, at), Stmt::Return(Some(held), at)],
                at,
            )
        }
        Stmt::Return(None, at) => block(vec![dispose(x, at), Stmt::Return(None, at)], at),
        Stmt::Yield(None, at) => block(vec![dispose(x, at), Stmt::Yield(None, at)], at),
        Stmt::Break(at) if !around.loops && !around.switches => {
            block(vec![dispose(x, at), Stmt::Break(at)], at)
        }
        Stmt::Continue(at) if !around.loops => block(vec![dispose(x, at), Stmt::Continue(at)], at),
        other => return Err(Box::new(other)),
    })
}

/// The statements inside `s`, rewritten (a loop's or a switch's `break`
/// its own).
fn nested(x: &Name, s: Stmt, around: Around) -> Stmt {
    let inside = |s: Stmt, around| Box::new(rewrite(x, s, around));
    let looping = Around {
        loops: true,
        ..around
    };
    match s {
        Stmt::If {
            cond,
            then,
            otherwise,
        } => Stmt::If {
            cond,
            then: inside(*then, around),
            otherwise: otherwise.map(|o| inside(*o, around)),
        },
        Stmt::Block(b) => Stmt::Block(rewrite_block(x, b, around)),
        Stmt::Unsafe(b, at) => Stmt::Unsafe(rewrite_block(x, b, around), at),
        Stmt::Checked(b, checked) => Stmt::Checked(rewrite_block(x, b, around), checked),
        Stmt::Lock {
            target,
            value,
            body,
            at,
        } => Stmt::Lock {
            target,
            value,
            body: inside(*body, around),
            at,
        },
        Stmt::Switch { expr, mut sections } => {
            let switching = Around {
                switches: true,
                ..around
            };
            for s in &mut sections {
                s.body = std::mem::take(&mut s.body)
                    .into_iter()
                    .map(|b| rewrite(x, b, switching))
                    .collect();
            }
            Stmt::Switch { expr, sections }
        }
        Stmt::Foreach {
            ty,
            binding,
            iter,
            body,
            mutable,
            awaits,
        } => Stmt::Foreach {
            ty,
            binding,
            iter,
            body: inside(*body, looping),
            mutable,
            awaits,
        },
        Stmt::For {
            init,
            cond,
            step,
            body,
        } => Stmt::For {
            init,
            cond,
            step,
            body: inside(*body, looping),
        },
        Stmt::While { cond, body } => Stmt::While {
            cond,
            body: inside(*body, looping),
        },
        Stmt::DoWhile { body, cond } => Stmt::DoWhile {
            body: inside(*body, looping),
            cond,
        },
        other => other,
    }
}

fn rewrite_block(x: &Name, b: Block, around: Around) -> Block {
    Block {
        stmts: b.stmts.into_iter().map(|s| rewrite(x, s, around)).collect(),
        span: b.span,
    }
}

/// `if (x != null) await x.DisposeAsync();`.
fn dispose(x: &Name, at: Span) -> Stmt {
    let name = || Expr {
        kind: ExprKind::Ident(x.text.clone()),
        span: at,
    };
    let null = Expr {
        kind: ExprKind::Null,
        span: at,
    };
    let cond = Expr {
        kind: ExprKind::Binary {
            op: BinOp::NotEq,
            left: Box::new(name()),
            right: Box::new(null),
        },
        span: at,
    };
    let method = Name {
        text: DISPOSE_ASYNC.to_string(),
        span: at,
    };
    let callee = Expr {
        kind: ExprKind::Member {
            expr: Box::new(name()),
            name: method,
            safe: false,
        },
        span: at,
    };
    let call = Expr {
        kind: ExprKind::Call {
            callee: Box::new(callee),
            args: Vec::<Arg>::new(),
        },
        span: at,
    };
    let awaited = Expr {
        kind: ExprKind::Unary {
            op: UnOp::Await,
            expr: Box::new(call),
        },
        span: at,
    };
    Stmt::If {
        cond,
        then: Box::new(Stmt::Expr(awaited)),
        otherwise: None,
    }
}
